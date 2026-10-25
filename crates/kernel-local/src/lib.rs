use anyhow::{Context, bail};
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt};
use hmac::{Hmac, Mac};
use kernel::{Channel, Content, ExecutionState, Execution, Kernel, KernelStatus, Message};
use serde_json::{Value, json};
use sha2::Sha256;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSendHalf, DealerSocket, ReqSocket, Socket, SocketOptions, SocketRecv, SocketSend, SubSocket, ZmqMessage};

const DELIM: &[u8] = b"<IDS|MSG>";

pub fn project_root(notebook: &Path) -> PathBuf {
    let dir = notebook.parent().unwrap_or(Path::new(".")).to_path_buf();
    dir.ancestors().find(|d| d.join("pyproject.toml").is_file()).map(Path::to_path_buf).unwrap_or(dir)
}

pub async fn ensure_env(uv: &Path, root: &Path) -> anyhow::Result<PathBuf> {
    let python = root.join(".venv/bin/python");
    if !python.exists() {
        run(Command::new(uv).arg("venv").arg(root.join(".venv")).current_dir(root)).await?;
    }
    let has_ipykernel = Command::new(&python).args(["-c", "import ipykernel"]).stdout(Stdio::null()).stderr(Stdio::null()).status().await?.success();
    if !has_ipykernel && root.join("pyproject.toml").is_file() {
        run(Command::new(uv).args(["add", "--dev", "ipykernel"]).current_dir(root)).await?;
    } else if !has_ipykernel {
        run(Command::new(uv).args(["pip", "install", "--python"]).arg(&python).arg("ipykernel").current_dir(root)).await?;
    }
    Ok(python)
}

async fn run(cmd: &mut Command) -> anyhow::Result<()> {
    let out = cmd.env_remove("VIRTUAL_ENV").output().await.with_context(|| format!("running {cmd:?}"))?;
    if !out.status.success() {
        bail!("{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

fn sign(key: &[u8], parts: &[&[u8]]) -> String {
    if key.is_empty() {
        return String::new();
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key");
    parts.iter().for_each(|p| mac.update(p));
    hex::encode(mac.finalize().into_bytes())
}

pub fn encode(msg: &Message, key: &[u8]) -> anyhow::Result<Vec<Bytes>> {
    let header = serde_json::to_vec(&msg.header)?;
    let parent = msg.parent_header.as_ref().map_or(Ok(b"{}".to_vec()), serde_json::to_vec)?;
    let metadata = serde_json::to_vec(&msg.metadata)?;
    let content = serde_json::to_vec(&msg.content)?;
    let sig = sign(key, &[&header, &parent, &metadata, &content]);
    let mut frames: Vec<Bytes> = vec![DELIM.into(), sig.into(), header.into(), parent.into(), metadata.into(), content.into()];
    frames.extend(msg.buffers.iter().cloned().map(Bytes::from));
    Ok(frames)
}

pub fn decode(frames: &[Bytes], key: &[u8], channel: Channel) -> anyhow::Result<Message> {
    let i = frames.iter().position(|f| f.as_ref() == DELIM).context("missing delimiter")?;
    let [sig, header, parent, metadata, content, ..] = frames.get(i + 1..).filter(|f| f.len() >= 5).context("short message")? else { unreachable!() };
    if sign(key, &[header, parent, metadata, content]).as_bytes() != sig.as_ref() {
        bail!("bad signature");
    }
    let mut msg: Message = serde_json::from_value(json!({
        "header": serde_json::from_slice::<Value>(header)?,
        "parent_header": serde_json::from_slice::<Value>(parent)?,
        "metadata": serde_json::from_slice::<Value>(metadata)?,
        "content": serde_json::from_slice::<Value>(content)?,
    }))?;
    msg.channel = Some(channel);
    msg.buffers = frames[i + 6..].iter().map(|b| b.to_vec()).collect();
    Ok(msg)
}

struct Conn {
    child: Child,
    key: Vec<u8>,
    shell: tokio::sync::Mutex<DealerSendHalf>,
    stdin: tokio::sync::Mutex<DealerSendHalf>,
    control: tokio::sync::Mutex<DealerSendHalf>,
    tasks: Vec<JoinHandle<()>>,
    file: PathBuf,
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.tasks.iter().for_each(JoinHandle::abort);
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.file);
    }
}

struct Pending(String, Arc<Mutex<HashSet<String>>>);

impl Drop for Pending {
    fn drop(&mut self) {
        self.1.lock().unwrap().remove(&self.0);
    }
}

pub struct LocalKernel {
    python: PathBuf,
    cwd: PathBuf,
    session: String,
    conn: tokio::sync::RwLock<Option<Conn>>,
    tx: broadcast::Sender<Message>,
    status: Arc<Mutex<KernelStatus>>,
    executions: Arc<Mutex<HashSet<String>>>,
}

impl LocalKernel {
    pub async fn start(notebook: &Path, uv: Option<&Path>) -> anyhow::Result<Self> {
        let root = project_root(notebook);
        let python = ensure_env(uv.unwrap_or(Path::new("uv")), &root).await?;
        Self::launch(python, root).await
    }

    pub async fn launch(python: PathBuf, cwd: PathBuf) -> anyhow::Result<Self> {
        let kernel = Self {
            python,
            cwd,
            session: uuid::Uuid::new_v4().to_string(),
            conn: Default::default(),
            tx: broadcast::channel(4096).0,
            status: Arc::new(Mutex::new(KernelStatus::Connecting)),
            executions: Default::default(),
        };
        kernel.connect().await?;
        Ok(kernel)
    }

    async fn connect(&self) -> anyhow::Result<()> {
        *self.status.lock().unwrap() = KernelStatus::Connecting;
        let ports: Vec<u16> = (0..5).map(|_| std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port())).collect::<Result<_, _>>()?;
        let key = uuid::Uuid::new_v4().to_string();
        let file = std::env::temp_dir().join(format!("colab-app-kernel-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&file, serde_json::to_vec(&json!({
            "transport": "tcp", "ip": "127.0.0.1", "signature_scheme": "hmac-sha256", "key": key,
            "shell_port": ports[0], "iopub_port": ports[1], "stdin_port": ports[2], "control_port": ports[3], "hb_port": ports[4],
        }))?)?;
        let child = Command::new(&self.python).args(["-m", "ipykernel_launcher", "-f"]).arg(&file).env("JPY_PARENT_PID", std::process::id().to_string())
            .current_dir(&self.cwd).stdin(Stdio::null()).stdout(Stdio::null()).kill_on_drop(true).spawn().context("starting ipykernel")?;
        let url = |p: u16| format!("tcp://127.0.0.1:{p}");
        let key = key.into_bytes();
        let mut tasks = Vec::new();

        let mut iopub = SubSocket::new();
        iopub.connect(&url(ports[1])).await?;
        iopub.subscribe("").await?;
        tasks.push(self.reader(iopub, key.clone(), Channel::Iopub));

        let identity = PeerIdentity::try_from(uuid::Uuid::new_v4().as_bytes().as_slice())?;
        let shell = self.dealer(&url(ports[0]), Some(&identity), &key, Channel::Shell, &mut tasks).await?;
        let stdin = self.dealer(&url(ports[2]), Some(&identity), &key, Channel::Stdin, &mut tasks).await?;
        let control = self.dealer(&url(ports[3]), None, &key, Channel::Control, &mut tasks).await?;

        let mut hb = ReqSocket::new();
        hb.connect(&url(ports[4])).await?;
        let status = self.status.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let ok = hb.send("ping".into()).await.is_ok() && matches!(timeout(Duration::from_secs(10), hb.recv()).await, Ok(Ok(_)));
                if !ok {
                    *status.lock().unwrap() = KernelStatus::Dead;
                    break;
                }
            }
        }));

        *self.conn.write().await = Some(Conn { child, key, shell, stdin, control, tasks, file });
        self.wait_ready().await
    }

    async fn dealer(&self, url: &str, identity: Option<&PeerIdentity>, key: &[u8], channel: Channel, tasks: &mut Vec<JoinHandle<()>>) -> anyhow::Result<tokio::sync::Mutex<DealerSendHalf>> {
        let mut opts = SocketOptions::default();
        if let Some(id) = identity {
            opts.peer_identity(id.clone());
        }
        let mut socket = DealerSocket::with_options(opts);
        socket.connect(url).await?;
        let (send, recv) = socket.split();
        tasks.push(self.reader(recv, key.to_vec(), channel));
        Ok(tokio::sync::Mutex::new(send))
    }

    async fn wait_ready(&self) -> anyhow::Result<()> {
        let mut rx = self.tx.subscribe();
        let ready = async {
            loop {
                let msg = Message::new("kernel_info_request", &self.session, json!({}));
                let id = msg.header.msg_id.clone();
                self.send_on(Channel::Shell, msg).await?;
                let got = timeout(Duration::from_millis(500), async {
                    loop {
                        if let Ok(m) = rx.recv().await && m.channel == Some(Channel::Iopub) && m.parent_id() == Some(&id) {
                            return;
                        }
                    }
                }).await;
                if got.is_ok() {
                    return anyhow::Ok(());
                }
                if let Some(conn) = self.conn.write().await.as_mut() && let Ok(Some(code)) = conn.child.try_wait() {
                    bail!("ipykernel exited with {code}");
                }
            }
        };
        timeout(Duration::from_secs(60), ready).await.context("kernel did not start in 60s")??;
        *self.status.lock().unwrap() = KernelStatus::Idle;
        Ok(())
    }

    fn reader<S: SocketRecv + Send + 'static>(&self, mut socket: S, key: Vec<u8>, channel: Channel) -> JoinHandle<()> {
        let (tx, status) = (self.tx.clone(), self.status.clone());
        tokio::spawn(async move {
            while let Ok(frames) = socket.recv().await {
                let Ok(msg) = decode(&frames.into_vec(), &key, channel) else { continue };
                if let Content::Status { execution_state } = msg.typed() {
                    let mut s = status.lock().unwrap();
                    *s = match execution_state {
                        ExecutionState::Busy => KernelStatus::Busy,
                        ExecutionState::Idle => KernelStatus::Idle,
                        ExecutionState::Dead => KernelStatus::Dead,
                        _ => *s,
                    };
                }
                let _ = tx.send(msg);
            }
        })
    }

    async fn send_on(&self, channel: Channel, mut msg: Message) -> anyhow::Result<()> {
        msg.header.session = self.session.clone();
        let guard = self.conn.read().await;
        let conn = guard.as_ref().context("kernel is not running")?;
        let socket = match channel {
            Channel::Shell => &conn.shell,
            Channel::Stdin => &conn.stdin,
            Channel::Control => &conn.control,
            Channel::Iopub => bail!("cannot send on iopub"),
        };
        let frames = ZmqMessage::try_from(encode(&msg, &conn.key)?).map_err(|e| anyhow::anyhow!("{e}"))?;
        socket.lock().await.send(frames).await?;
        Ok(())
    }

    fn pid(&self, conn: &Conn) -> Option<i32> {
        conn.child.id().map(|p| p as i32)
    }

    async fn stop(&self) {
        if self.conn.read().await.is_some() {
            let _ = self.send_on(Channel::Control, Message::new("shutdown_request", &self.session, json!({"restart": false}))).await;
        }
        if let Some(mut conn) = self.conn.write().await.take() {
            let _ = timeout(Duration::from_secs(3), conn.child.wait()).await;
        }
    }
}

#[async_trait]
impl Kernel for LocalKernel {
    async fn execute(&self, code: &str) -> anyhow::Result<Execution> {
        let rx = self.tx.subscribe();
        let msg = Message::execute_request(&self.session, code);
        let id = msg.header.msg_id.clone();
        self.executions.lock().unwrap().insert(id.clone());
        self.send_on(Channel::Shell, msg).await?;
        let pending = Pending(id, self.executions.clone());
        Ok(stream::unfold(Some((rx, pending)), |state| async move {
            let (mut rx, pending) = state?;
            loop {
                match rx.recv().await {
                    Ok(m) if m.parent_id() == Some(&pending.0) && matches!(m.channel, Some(Channel::Iopub | Channel::Stdin)) => {
                        let done = m.channel == Some(Channel::Iopub) && matches!(m.typed(), Content::Status { execution_state: ExecutionState::Idle });
                        return Some((m, (!done).then_some((rx, pending))));
                    }
                    Err(broadcast::error::RecvError::Closed) => return None,
                    _ => {}
                }
            }
        }).boxed())
    }

    async fn send(&self, msg: Message) -> anyhow::Result<()> {
        self.send_on(msg.channel.unwrap_or(Channel::Shell), msg).await
    }

    async fn interrupt(&self) -> anyhow::Result<()> {
        let guard = self.conn.read().await;
        let pid = guard.as_ref().and_then(|c| self.pid(c)).context("kernel is not running")?;
        if unsafe { libc::kill(pid, libc::SIGINT) } != 0 {
            bail!("interrupt failed: {}", std::io::Error::last_os_error());
        }
        Ok(())
    }

    async fn restart(&self) -> anyhow::Result<()> {
        self.stop().await;
        self.executions.lock().unwrap().clear();
        self.connect().await
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        self.stop().await;
        *self.status.lock().unwrap() = KernelStatus::Disconnected;
        Ok(())
    }

    fn status(&self) -> KernelStatus {
        *self.status.lock().unwrap()
    }

    fn subscribe(&self) -> BoxStream<'static, Message> {
        let executions = self.executions.clone();
        stream::unfold(self.tx.subscribe(), move |mut rx| {
            let executions = executions.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(m) if !m.parent_id().is_some_and(|p| executions.lock().unwrap().contains(p)) => return Some((m, rx)),
                        Err(broadcast::error::RecvError::Closed) => return None,
                        _ => {}
                    }
                }
            }
        }).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_frames_round_trip_and_reject_tampering() {
        let mut msg = Message::execute_request("s1", "print('hi')");
        msg.buffers = vec![vec![1, 2, 3]];
        let mut frames = encode(&msg, b"secret").unwrap();
        frames.insert(0, Bytes::from_static(b"routing-id"));
        let back = decode(&frames, b"secret", Channel::Shell).unwrap();
        assert_eq!((back.header, back.content, back.buffers), (msg.header, msg.content, msg.buffers));
        assert!(decode(&frames, b"other", Channel::Shell).is_err());
        frames[6] = Bytes::from_static(br#"{"code":"import os"}"#);
        assert!(decode(&frames, b"secret", Channel::Shell).is_err());
    }
}
