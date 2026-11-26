use crate::engine::Core;
use crate::ipc::KernelType;
use axum::{Extension, Json};
use axum::extract::ws::{Message as AxumFrame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures::{SinkExt, StreamExt};
use kernel::{Kernel, KernelStatus};
use kernel_local::LocalKernel;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message as Frame;

const RING_BYTES: usize = 50 << 20;
const REAP_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
const CLIENT_SILENCE: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct Client(pub String);

pub static TYPES: std::sync::LazyLock<Vec<KernelType>> = std::sync::LazyLock::new(detect);

fn detect() -> Vec<KernelType> {
    let first_line = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|l| l.trim().to_string())).filter(|l| !l.is_empty());
    let cpu = || std::fs::read_to_string("/proc/cpuinfo").ok()?.lines().find_map(|l| Some(l.strip_prefix("model name")?.split_once(':')?.1.trim().to_string()));
    let (id, label) = if cfg!(target_os = "macos") {
        (if cfg!(target_arch = "aarch64") { "MPS" } else { "CPU" }, first_line("sysctl", &["-n", "machdep.cpu.brand_string"]))
    } else if let Some(gpu) = first_line("nvidia-smi", &["--query-gpu=name", "--format=csv,noheader"]) {
        ("CUDA", Some(gpu))
    } else {
        ("CPU", cpu())
    };
    vec![KernelType { id: id.into(), label: label.unwrap_or_default() }]
}

#[cfg(target_os = "macos")]
mod power {
    use std::ffi::{CString, c_char, c_void};

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(alloc: *const c_void, text: *const c_char, encoding: u32) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(kind: *const c_void, level: u32, name: *const c_void, id: *mut u32) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    fn cf(text: &str) -> Option<*const c_void> {
        let text = CString::new(text).ok()?;
        Some(unsafe { CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), 0x0800_0100) }).filter(|s| !s.is_null())
    }

    pub struct Awake(u32);

    impl Awake {
        pub fn hold(name: &str) -> Option<Self> {
            let name = cf(name)?;
            let kind = cf("PreventUserIdleSystemSleep")?;
            let mut id = 0;
            let code = unsafe { IOPMAssertionCreateWithName(kind, 255, name, &mut id) };
            unsafe { CFRelease(kind); CFRelease(name) };
            if code != 0 {
                log::warn!("IOPMAssertionCreateWithName failed with {code:#x}");
            }
            (code == 0).then_some(Self(id))
        }
    }

    impl Drop for Awake {
        fn drop(&mut self) {
            unsafe { IOPMAssertionRelease(self.0) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod power {
    pub struct Awake;

    impl Awake {
        pub fn hold(_: &str) -> Option<Self> {
            None
        }
    }
}

pub use power::Awake;

pub fn stay_awake(awake: &mut Option<Awake>, busy: bool, name: impl FnOnce() -> String) {
    if !busy {
        *awake = None;
    } else if awake.is_none() {
        *awake = Awake::hold(&name());
    }
}

struct Attached {
    tx: UnboundedSender<Frame>,
    _close: oneshot::Sender<()>,
}

#[derive(Default)]
struct Session {
    client: Option<Attached>,
    ring: VecDeque<Frame>,
    bytes: usize,
}

impl Session {
    fn push(&mut self, frame: Frame) {
        self.bytes += frame.len();
        self.ring.push_back(frame);
        while (self.bytes > RING_BYTES || self.ring.len() > colab::kernel::RECENT_IDS) && let Some(old) = self.ring.pop_front() {
            self.bytes -= old.len();
        }
    }
}

struct Hosted {
    kernel: LocalKernel,
    sessions: Mutex<HashMap<String, Session>>,
    client: String,
    awake: Mutex<Option<Awake>>,
    used: Mutex<Instant>,
}

impl Hosted {
    fn stay_awake(&self) {
        stay_awake(&mut self.awake.lock().unwrap(), self.kernel.status() == KernelStatus::Busy, || format!("Colab Notebook kernel running for {}", self.client));
    }

    fn abandoned(&self) -> bool {
        let mut used = self.used.lock().unwrap();
        if self.kernel.status() == KernelStatus::Busy || self.sessions.lock().unwrap().values().any(|s| s.client.is_some()) {
            *used = Instant::now();
        }
        used.elapsed() > REAP_AFTER
    }

    fn deliver(&self, frame: Frame) {
        for session in self.sessions.lock().unwrap().values_mut() {
            if session.client.as_ref().is_some_and(|c| c.tx.send(frame.clone()).is_err()) {
                session.client = None;
            }
            session.push(frame.clone());
        }
    }

    fn attach(&self, id: &str, client: Attached) -> VecDeque<Frame> {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|k, s| k == id || s.client.is_some());
        let session = sessions.entry(id.into()).or_default();
        session.client = Some(client);
        session.ring.clone()
    }

    fn detach(&self, id: &str, tx: &UnboundedSender<Frame>) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(id) && s.client.as_ref().is_some_and(|c| c.tx.same_channel(tx)) {
            s.client = None;
        }
    }

    fn model(&self, id: &str) -> Value {
        let state = match self.kernel.status() {
            KernelStatus::Busy => "busy",
            KernelStatus::Dead => "dead",
            KernelStatus::Connecting => "starting",
            _ => "idle",
        };
        json!({"id": id, "name": "python3", "execution_state": state})
    }
}

pub struct KernelHost {
    core: Arc<Core>,
    root: PathBuf,
    kernels: Arc<Mutex<HashMap<String, Arc<Hosted>>>>,
}

type Reply = Result<Response, (StatusCode, String)>;

fn not_found(id: &str) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("no kernel {id}"))
}

fn failed(e: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
}

impl KernelHost {
    pub fn new(core: Arc<Core>, root: PathBuf) -> Arc<Self> {
        Arc::new(Self { core, root, kernels: Arc::default() })
    }

    pub fn alive(&self) -> usize {
        self.kernels.lock().unwrap().values().filter(|k| !matches!(k.kernel.status(), KernelStatus::Dead | KernelStatus::Disconnected)).count()
    }

    fn get(&self, id: &str) -> Result<Arc<Hosted>, (StatusCode, String)> {
        self.kernels.lock().unwrap().get(id).cloned().ok_or_else(|| not_found(id))
    }

    async fn create(&self, client: String, body: Value) -> Reply {
        let name = body["path"].as_str().unwrap_or_default();
        if name.is_empty() || name.contains('/') || name.starts_with('.') {
            return Err((StatusCode::BAD_REQUEST, format!("invalid project name {name:?}")));
        }
        let home = self.root.join(name);
        let dir = if home.is_dir() { home } else { self.core.data_dir().join("kernels").join(name) };
        std::fs::create_dir_all(&dir).map_err(|e| failed(e.into()))?;
        let uv = self.core.uv().await.map_err(failed)?;
        let python = kernel_local::ensure_env(&uv, &dir).await.map_err(failed)?;
        let kernel = LocalKernel::launch(python, dir.clone(), self.core.shim().map_err(failed)?).await.map_err(failed)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut messages = kernel.subscribe();
        let hosted = Arc::new(Hosted { kernel, sessions: Mutex::default(), client, awake: Mutex::default(), used: Mutex::new(Instant::now()) });
        let (weak, kernels, kernel_id) = (Arc::downgrade(&hosted), self.kernels.clone(), id.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(10));
            loop {
                let msg = tokio::select! {
                    msg = messages.next() => match msg { Some(msg) => Some(msg), None => break },
                    _ = tick.tick() => None,
                };
                let Some(hosted) = weak.upgrade() else { break };
                hosted.stay_awake();
                match msg.map(colab::kernel::encode) {
                    Some(Ok(frame)) => hosted.deliver(frame),
                    None if hosted.abandoned() => {
                        log::info!("shutting down kernel {kernel_id}, idle with no client for {REAP_AFTER:?}");
                        kernels.lock().unwrap().remove(&kernel_id);
                        let _ = hosted.kernel.shutdown().await;
                        break;
                    }
                    _ => {}
                }
            }
        });
        log::info!("hosting kernel {id} in {}", dir.display());
        let model = hosted.model(&id);
        self.kernels.lock().unwrap().insert(id, hosted);
        Ok((StatusCode::CREATED, Json(model)).into_response())
    }
}

fn to_axum(frame: Frame) -> AxumFrame {
    match frame {
        Frame::Text(t) => AxumFrame::Text(t.as_str().into()),
        other => AxumFrame::Binary(other.into_data()),
    }
}

fn to_tungstenite(frame: AxumFrame) -> Option<Frame> {
    match frame {
        AxumFrame::Text(t) => Some(Frame::text(t.as_str())),
        AxumFrame::Binary(b) => Some(Frame::binary(b)),
        _ => None,
    }
}

async fn list(State(host): State<Arc<KernelHost>>) -> Json<Value> {
    Json(host.kernels.lock().unwrap().iter().map(|(id, k)| k.model(id)).collect())
}

async fn create(State(host): State<Arc<KernelHost>>, client: Option<Extension<Client>>, body: Option<Json<Value>>) -> Reply {
    host.create(client.map_or("another machine".into(), |c| c.0.0), body.map_or(Value::Null, |b| b.0)).await
}

async fn show(State(host): State<Arc<KernelHost>>, Path(id): Path<String>) -> Reply {
    Ok(Json(host.get(&id)?.model(&id)).into_response())
}

async fn delete(State(host): State<Arc<KernelHost>>, Path(id): Path<String>) -> Reply {
    let hosted = host.kernels.lock().unwrap().remove(&id).ok_or_else(|| not_found(&id))?;
    hosted.kernel.shutdown().await.map_err(failed)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn interrupt(State(host): State<Arc<KernelHost>>, Path(id): Path<String>) -> Reply {
    host.get(&id)?.kernel.interrupt().await.map_err(failed)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn restart(State(host): State<Arc<KernelHost>>, Path(id): Path<String>) -> Reply {
    let hosted = host.get(&id)?;
    hosted.kernel.restart().await.map_err(failed)?;
    Ok(Json(hosted.model(&id)).into_response())
}

async fn channels(ws: WebSocketUpgrade, State(host): State<Arc<KernelHost>>, Path(id): Path<String>, Query(query): Query<HashMap<String, String>>) -> Reply {
    let hosted = host.get(&id)?;
    let session = query.get("session_id").cloned().unwrap_or_default();
    Ok(ws.max_message_size(1 << 30).on_upgrade(move |socket| client(hosted, session, socket)))
}

async fn client(hosted: Arc<Hosted>, session: String, socket: WebSocket) {
    let (tx, mut rx) = unbounded_channel();
    let (close, closed) = oneshot::channel();
    let backlog = hosted.attach(&session, Attached { tx: tx.clone(), _close: close });
    let serve = {
        let hosted = hosted.clone();
        async move {
            let (mut sink, mut stream) = socket.split();
            for frame in backlog {
                if sink.send(to_axum(frame)).await.is_err() {
                    return;
                }
            }
            let mut heard = Instant::now();
            loop {
                tokio::select! {
                    frame = rx.recv() => if let Some(frame) = frame && sink.send(to_axum(frame)).await.is_err() {
                        return;
                    },
                    incoming = stream.next() => {
                        heard = Instant::now();
                        match incoming {
                            Some(Ok(frame @ (AxumFrame::Text(_) | AxumFrame::Binary(_)))) => {
                                if let Some(Ok(msg)) = to_tungstenite(frame).map(|f| colab::kernel::decode(&f)) && let Err(e) = hosted.kernel.send(msg).await {
                                    log::warn!("sending to hosted kernel: {e:#}");
                                }
                            }
                            Some(Ok(_)) => {}
                            _ => return,
                        }
                    }
                    _ = tokio::time::sleep_until((heard + CLIENT_SILENCE).into()) => return,
                }
            }
        }
    };
    tokio::select! {
        _ = serve => {}
        _ = closed => {}
    }
    hosted.detach(&session, &tx);
}

pub fn router(host: Arc<KernelHost>) -> axum::Router {
    axum::Router::new()
        .route("/api/kernels", get(list).post(create))
        .route("/api/kernels/{id}", get(show).delete(delete))
        .route("/api/kernels/{id}/interrupt", post(interrupt))
        .route("/api/kernels/{id}/restart", post(restart))
        .route("/api/kernels/{id}/channels", get(channels))
        .with_state(host)
}
