use crate::client::{Colab, Runtime, Variant};
use crate::store;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{SinkExt, StreamExt};
use kernel::{Content, ExecutionState, Execution, Kernel, KernelStatus, Message};
use reqwest::Method;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

const PING_INTERVAL: Duration = Duration::from_secs(10);
const RECONNECT_ATTEMPTS: u32 = 60;
const MISSED_PINGS: u32 = 3;
const LOG_HOOK: &str = include_str!("log_hook.py");

enum Command {
    Send(tungstenite::Message),
    Reconnect,
    Close,
}

struct Shared {
    status: Mutex<KernelStatus>,
    executions: Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>,
    events: broadcast::Sender<Message>,
    seen: Mutex<std::collections::VecDeque<String>>,
    logged: Mutex<Option<u64>>,
}

impl Shared {
    fn set(&self, status: KernelStatus) {
        *self.status.lock().unwrap() = status;
    }
}

pub struct ColabKernel {
    pub runtime: Arc<Runtime>,
    pub kernel_id: String,
    pub session: String,
    notebook: PathBuf,
    shared: Arc<Shared>,
    commands: mpsc::UnboundedSender<Command>,
}

impl Colab {
    pub async fn start(&self, notebook: &Path, variant: Variant, accelerator: Option<&str>, high_ram: bool) -> Result<ColabKernel> {
        let a = self.assign(variant, accelerator, high_ram).await?;
        let runtime = Arc::new(Runtime::new(self.clone(), a.endpoint.clone(), &a.runtime_proxy_info));
        match runtime.jupyter(Method::POST, "/api/kernels").await {
            Ok(k) if k["id"].is_string() => {
                let kernel_id = k["id"].as_str().unwrap().to_string();
                store::save(&store::default_path(), notebook, &store::Saved { endpoint: a.endpoint.clone(), kernel_id: kernel_id.clone() })?;
                Ok(ColabKernel::connect(runtime, kernel_id, notebook))
            }
            other => {
                self.unassign(&a.endpoint).await?;
                bail!("creating a kernel failed: {other:?}")
            }
        }
    }

    pub async fn reattach(&self, notebook: &Path) -> Result<Option<ColabKernel>> {
        let path = store::default_path();
        let Some(saved) = store::get(&path, notebook)? else { return Ok(None) };
        let assignment = self.assignments().await?.into_iter().find(|a| a.endpoint == saved.endpoint);
        let Some(a) = assignment else {
            store::remove(&path, notebook)?;
            return Ok(None);
        };
        let runtime = Arc::new(Runtime::new(self.clone(), a.endpoint, &a.runtime_proxy_info));
        if runtime.jupyter(Method::GET, &format!("/api/kernels/{}", saved.kernel_id)).await.is_err() {
            store::remove(&path, notebook)?;
            return Ok(None);
        }
        Ok(Some(ColabKernel::connect(runtime, saved.kernel_id, notebook)))
    }
}

impl ColabKernel {
    pub fn connect(runtime: Arc<Runtime>, kernel_id: String, notebook: &Path) -> Self {
        let shared = Arc::new(Shared { status: Mutex::new(KernelStatus::Connecting), executions: Mutex::default(), events: broadcast::channel(1024).0, seen: Mutex::default(), logged: Mutex::default() });
        let (commands, rx) = mpsc::unbounded_channel();
        let session = uuid::Uuid::new_v4().to_string();
        tokio::spawn(run(runtime.clone(), kernel_id.clone(), session.clone(), shared.clone(), rx));
        Self { runtime, kernel_id, session, notebook: notebook.into(), shared, commands }
    }

    pub fn set_notebook(&mut self, notebook: &Path) {
        self.notebook = notebook.into();
    }

    pub fn force_reconnect(&self) {
        let _ = self.commands.send(Command::Reconnect);
    }

    pub fn detach(self) -> Result<()> {
        let _ = self.commands.send(Command::Close);
        store::save(&store::default_path(), &self.notebook, &store::Saved { endpoint: self.runtime.endpoint.clone(), kernel_id: self.kernel_id.clone() })
    }

    pub async fn release(self) -> Result<()> {
        let _ = self.commands.send(Command::Close);
        store::remove(&store::default_path(), &self.notebook)?;
        self.runtime.colab.unassign(&self.runtime.endpoint).await
    }

    fn kernel_path(&self, action: &str) -> String {
        format!("/api/kernels/{}{action}", self.kernel_id)
    }
}

#[async_trait]
impl Kernel for ColabKernel {
    async fn execute(&self, code: &str) -> Result<Execution> {
        let msg = Message::execute_request(&self.session, code);
        let (tx, rx) = mpsc::unbounded_channel();
        self.shared.executions.lock().unwrap().insert(msg.header.msg_id.clone(), tx);
        self.send(msg).await?;
        Ok(tokio_stream(rx))
    }

    async fn send(&self, mut msg: Message) -> Result<()> {
        msg.header.session = self.session.clone();
        self.commands.send(Command::Send(encode(msg)?)).map_err(|_| anyhow::anyhow!("kernel connection closed"))
    }

    async fn interrupt(&self) -> Result<()> {
        self.runtime.jupyter(Method::POST, &self.kernel_path("/interrupt")).await.map(drop)
    }

    async fn restart(&self) -> Result<()> {
        self.runtime.jupyter(Method::POST, &self.kernel_path("/restart")).await.map(drop)
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = self.commands.send(Command::Close);
        self.runtime.jupyter(Method::DELETE, &self.kernel_path("")).await.map(drop)
    }

    fn status(&self) -> KernelStatus {
        *self.shared.status.lock().unwrap()
    }

    fn subscribe(&self) -> BoxStream<'static, Message> {
        futures::stream::unfold(self.shared.events.subscribe(), |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(m) => return Some((m, rx)),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return None,
                }
            }
        }).boxed()
    }
}

fn tokio_stream(rx: mpsc::UnboundedReceiver<Message>) -> Execution {
    futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|m| (m, rx)) }).boxed()
}

async fn run(runtime: Arc<Runtime>, kernel_id: String, session: String, shared: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<Command>) {
    let mut unsent: Option<tungstenite::Message> = None;
    let mut failures = 0;
    let mut next = None;
    loop {
        let opened = match next.take() {
            Some(ws) => Ok(ws),
            None => open(&runtime, &kernel_id, &session).await,
        };
        let ws = match opened {
            Ok(ws) => ws,
            Err(e) if failures < RECONNECT_ATTEMPTS => {
                log::warn!("kernel {kernel_id} websocket attempt {} failed: {e:#}", failures + 1);
                failures += 1;
                shared.set(KernelStatus::Reconnecting);
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            Err(e) => {
                log::error!("kernel {kernel_id} websocket gave up: {e:#}");
                break;
            }
        };
        log::info!("kernel {kernel_id} websocket connected");
        failures = 0;
        shared.set(remote_status(&runtime, &kernel_id).await.unwrap_or(KernelStatus::Idle));
        backfill(&runtime, &kernel_id, &shared).await;
        let (mut sink, mut stream) = ws.split();
        if let Some(frame) = unsent.take() && sink.send(frame.clone()).await.is_err() {
            unsent = Some(frame);
            continue;
        }
        if let Ok(hook) = encode(log_hook(&session, &kernel_id)) && sink.send(hook).await.is_err() {
            continue;
        }
        let mut ping = tokio::time::interval(PING_INTERVAL);
        let (mut heard, mut message) = (Instant::now(), Instant::now());
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(Command::Send(frame)) => if sink.send(frame.clone()).await.is_err() {
                        unsent = Some(frame);
                        break;
                    },
                    Some(Command::Reconnect) => {
                        next = open(&runtime, &kernel_id, &session).await.ok();
                        let drain = async {
                            while let Some(Ok(frame)) = stream.next().await {
                                if let Ok(msg) = decode(&frame) {
                                    route(&shared, msg);
                                }
                            }
                        };
                        let _ = tokio::time::timeout(Duration::from_secs(5), drain).await;
                        break;
                    }
                    Some(Command::Close) | None => {
                        let _ = sink.close().await;
                        shared.set(KernelStatus::Disconnected);
                        shared.executions.lock().unwrap().clear();
                        return;
                    }
                },
                frame = stream.next() => match frame {
                    Some(Ok(frame @ (tungstenite::Message::Text(_) | tungstenite::Message::Binary(_)))) => {
                        (heard, message) = (Instant::now(), Instant::now());
                        let Ok(msg) = decode(&frame) else { continue };
                        route(&shared, msg);
                    }
                    Some(Ok(tungstenite::Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => heard = Instant::now(),
                },
                _ = ping.tick() => {
                    if heard.elapsed() > PING_INTERVAL * MISSED_PINGS || sink.send(tungstenite::Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                    let waiting = !shared.executions.lock().unwrap().is_empty();
                    if waiting && message.elapsed() >= PING_INTERVAL && remote_status(&runtime, &kernel_id).await == Some(KernelStatus::Idle) {
                        backfill(&runtime, &kernel_id, &shared).await;
                        shared.set(KernelStatus::Idle);
                        shared.executions.lock().unwrap().clear();
                    }
                }
            }
        }
        shared.set(KernelStatus::Reconnecting);
    }
    shared.set(KernelStatus::Disconnected);
    shared.executions.lock().unwrap().clear();
}

fn log_hook(session: &str, kernel_id: &str) -> Message {
    let mut msg = Message::execute_request(session, &format!("{LOG_HOOK}\n_colab_app_log({:?})", log_path(kernel_id)));
    msg.content["silent"] = true.into();
    msg.content["store_history"] = false.into();
    msg
}

fn log_path(kernel_id: &str) -> String {
    format!("/tmp/colab-app/{kernel_id}.jsonl")
}

async fn backfill(runtime: &Runtime, kernel_id: &str, shared: &Shared) {
    let Some(offset) = *shared.logged.lock().unwrap() else { return };
    let Ok(Ok(text)) = tokio::time::timeout(PING_INTERVAL, runtime.file_from(&log_path(kernel_id), offset)).await else { return };
    text.lines().filter_map(|line| serde_json::from_str(line).ok()).for_each(|msg| route(shared, msg));
}

async fn remote_status(runtime: &Runtime, kernel_id: &str) -> Option<KernelStatus> {
    let kernel = tokio::time::timeout(PING_INTERVAL, runtime.jupyter(Method::GET, &format!("/api/kernels/{kernel_id}"))).await.ok()?.ok()?;
    Some(match kernel["execution_state"].as_str()? {
        "busy" => KernelStatus::Busy,
        "dead" => KernelStatus::Dead,
        _ => KernelStatus::Idle,
    })
}

async fn open(runtime: &Runtime, kernel_id: &str, session: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
    let (url, token) = runtime.url_and_token().await?;
    let ws_url = format!("{}/api/kernels/{kernel_id}/channels?session_id={session}", url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1));
    let mut request = ws_url.into_client_request()?;
    request.headers_mut().insert("X-Colab-Runtime-Proxy-Token", token.parse()?);
    Ok(tokio_tungstenite::connect_async(request).await?.0)
}

pub fn encode(mut msg: Message) -> Result<tungstenite::Message> {
    let buffers = std::mem::take(&mut msg.buffers);
    let json = serde_json::to_vec(&msg)?;
    if buffers.is_empty() {
        return Ok(tungstenite::Message::text(String::from_utf8(json)?));
    }
    let header = 4 * (buffers.len() + 2);
    let mut offsets = vec![header];
    for part in std::iter::once(&json).chain(&buffers).take(buffers.len()) {
        offsets.push(offsets.last().unwrap() + part.len());
    }
    let mut out = ((buffers.len() + 1) as u32).to_be_bytes().to_vec();
    offsets.iter().for_each(|o| out.extend((*o as u32).to_be_bytes()));
    out.extend(json);
    buffers.iter().for_each(|b| out.extend(b));
    Ok(tungstenite::Message::binary(out))
}

pub fn decode(frame: &tungstenite::Message) -> Result<Message> {
    let data = match frame {
        tungstenite::Message::Text(text) => return Ok(serde_json::from_str(text)?),
        tungstenite::Message::Binary(data) => data,
        _ => bail!("unexpected frame"),
    };
    let word = |i: usize| data.get(i * 4..i * 4 + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()) as usize).context("short binary frame");
    let n = word(0)?;
    let mut offsets = (1..=n).map(word).collect::<Result<Vec<_>>>()?;
    offsets.push(data.len());
    let part = |i: usize| data.get(offsets[i]..offsets[i + 1]).context("bad binary frame offsets");
    let mut msg: Message = serde_json::from_slice(part(0)?)?;
    msg.buffers = (1..n).map(|i| part(i).map(<[u8]>::to_vec)).collect::<Result<_>>()?;
    Ok(msg)
}

fn route(shared: &Shared, msg: Message) {
    if let Some(offset) = msg.metadata.get("colab_app_log").and_then(|o| o.as_u64()) {
        let mut logged = shared.logged.lock().unwrap();
        *logged = Some(logged.map_or(offset, |l| l.max(offset)));
    }
    {
        let mut seen = shared.seen.lock().unwrap();
        if seen.contains(&msg.header.msg_id) {
            return;
        }
        if seen.len() >= 4096 {
            seen.pop_front();
        }
        seen.push_back(msg.header.msg_id.clone());
    }
    if let Content::Status { execution_state } = msg.typed() {
        shared.set(match execution_state {
            ExecutionState::Busy => KernelStatus::Busy,
            ExecutionState::Dead => KernelStatus::Dead,
            _ => KernelStatus::Idle,
        });
    }
    let mut executions = shared.executions.lock().unwrap();
    let Some(tx) = msg.parent_id().and_then(|id| executions.get(id)) else {
        drop(executions);
        let _ = shared.events.send(msg);
        return;
    };
    let done = matches!(msg.typed(), Content::Status { execution_state: ExecutionState::Idle });
    let _ = tx.send(msg.clone());
    if done {
        executions.remove(msg.parent_id().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::Channel;
    use serde_json::json;

    #[test]
    fn binary_frames_round_trip_widget_buffers() {
        let mut msg = Message::new("comm_msg", "s", json!({"comm_id": "c1", "data": {"method": "update", "buffer_paths": [["value"]]}}));
        msg.channel = Some(Channel::Shell);
        msg.buffers = vec![vec![0, 159, 255, 7], Vec::new(), b"png bytes".to_vec()];
        let frame = encode(msg.clone()).unwrap();
        let tungstenite::Message::Binary(bytes) = &frame else { panic!("expected a binary frame") };
        assert_eq!(&bytes[..8], &[0, 0, 0, 4, 0, 0, 0, 20]);
        assert_eq!(decode(&frame).unwrap(), msg);
        msg.buffers.clear();
        assert!(matches!(encode(msg.clone()).unwrap(), tungstenite::Message::Text(_)));
        assert_eq!(decode(&encode(msg.clone()).unwrap()).unwrap(), msg);
        assert!(decode(&tungstenite::Message::binary(vec![0, 0, 0, 9, 0, 0])).is_err());
    }
}
