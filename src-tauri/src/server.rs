use crate::windows::{self, LOCAL};
use colab_server::ipc::events;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, EventTarget, Manager};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio_tungstenite::{WebSocketStream, tungstenite::{Message, client::IntoClientRequest}};

type Reply = Result<Value, String>;

pub const OUTDATED: &str = "server-outdated";

#[derive(Default)]
pub struct Servers(Mutex<HashMap<String, Arc<Server>>>);

impl Servers {
    pub fn get(&self, app: &AppHandle, host: &str) -> Arc<Server> {
        let mut servers = self.0.lock().unwrap();
        servers.entry(host.into()).or_insert_with(|| {
            let server = Arc::new(Server::new(host));
            tauri::async_runtime::spawn(server.clone().run(app.clone()));
            server
        }).clone()
    }
}

pub fn of(app: &AppHandle, host: &str) -> Arc<Server> {
    app.state::<Servers>().get(app, host)
}

pub fn closed(app: &AppHandle, label: &str) {
    let host = windows::host_of(label);
    if host == LOCAL || app.webview_windows().keys().any(|l| l != label && windows::host_of(l) == host) {
        return;
    }
    if let Some(server) = app.state::<Servers>().0.lock().unwrap().remove(&host) {
        server.quit.notify_one();
    }
}

pub fn older(running: &str, included: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(running) < parts(included)
}

pub struct Server {
    host: String,
    conn: watch::Sender<Option<mpsc::UnboundedSender<String>>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    next: AtomicU64,
    choices: Mutex<HashMap<String, Value>>,
    stopped: AtomicBool,
    wake: Notify,
    quit: Notify,
}

impl Server {
    fn new(host: &str) -> Self {
        Self { host: host.into(), conn: watch::channel(None).0, pending: Default::default(), next: AtomicU64::new(1), choices: Default::default(), stopped: AtomicBool::new(false), wake: Notify::new(), quit: Notify::new() }
    }

    pub fn connection(&self) -> watch::Receiver<Option<mpsc::UnboundedSender<String>>> {
        self.conn.subscribe()
    }

    pub fn status(&self) -> &'static str {
        if self.conn.borrow().is_some() { "Running" } else if colab_server::socket_path().exists() { "Reconnecting" } else { "Stopped" }
    }

    pub fn start(&self) {
        self.stopped.store(false, Ordering::Relaxed);
        self.wake.notify_one();
    }

    pub async fn stop(&self, restart: bool) {
        self.stopped.store(!restart, Ordering::Relaxed);
        if self.conn.borrow().is_some() {
            let _ = self.call("shutdown", Value::Null).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), self.conn.subscribe().wait_for(Option::is_none)).await;
        }
    }

    fn name(&self) -> String {
        if self.host == LOCAL { "the Colab Notebook server".into() } else { format!("colab-server on {}", self.host) }
    }

    pub async fn call(&self, method: &str, params: Value) -> Reply {
        let mut conn = self.conn.subscribe();
        let wait = Duration::from_secs(if self.host == LOCAL { 30 } else { 10 });
        let out = tokio::time::timeout(wait, conn.wait_for(Option::is_some)).await
            .map_err(|_| format!("cannot reach {}", self.name()))?
            .map_err(|e| e.to_string())?.clone().unwrap();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let lost = || format!("lost the connection to {}", self.name());
        if out.send(json!({"id": id, "method": method, "params": params}).to_string()).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err(lost());
        }
        rx.await.unwrap_or_else(|_| Err(lost()))
    }

    fn reply(&self, frame: &Value) {
        let Some(tx) = frame["id"].as_u64().and_then(|id| self.pending.lock().unwrap().remove(&id)) else { return };
        let _ = tx.send(match frame.get("error") {
            Some(e) => Err(e.as_str().unwrap_or_default().to_string()),
            None => Ok(frame["result"].clone()),
        });
    }

    fn emit(&self, app: &AppHandle, event: &str, payload: Value) {
        drop(app.emit_filter(event, payload, |t| matches!(t, EventTarget::WebviewWindow { label } if windows::host_of(label) == self.host)));
    }

    fn event(self: &Arc<Self>, app: &AppHandle, event: &str, payload: Value) {
        let path = payload["path"].as_str().unwrap_or_default().to_string();
        match event {
            events::AGENT_OPEN if !windows::opens_agent_notebooks(app) => {}
            events::AGENT_OPEN => windows::agent_open(app, &self.host, &path),
            events::AGENT_CLOSE => windows::agent_close(app, &self.host, &path),
            _ => {
                if event == events::KERNEL_STATUS {
                    self.choices.lock().unwrap().insert(path, payload["choice"].clone());
                }
                self.emit(app, event, payload);
            }
        }
    }

    async fn session<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(self: &Arc<Self>, app: &AppHandle, ws: WebSocketStream<S>) {
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let writer = tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });
        self.conn.send_replace(Some(tx));
        while let Some(Ok(message)) = stream.next().await {
            let Message::Text(text) = message else { continue };
            let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue };
            match frame["event"].as_str() {
                Some(event) => self.event(app, event, frame["payload"].clone()),
                None => self.reply(&frame),
            }
        }
        self.conn.send_replace(None);
        writer.abort();
        let lost = format!("lost the connection to {}", self.name());
        self.pending.lock().unwrap().drain().for_each(|(_, tx)| drop(tx.send(Err(lost.clone()))));
    }

    fn reconnecting(&self, app: &AppHandle) {
        for path in windows::paths(app, &self.host) {
            let choice = self.choices.lock().unwrap().get(&path).cloned().unwrap_or(Value::Null);
            self.emit(app, events::KERNEL_STATUS, json!({"path": path, "status": "reconnecting", "choice": choice}));
        }
    }

    async fn refetch(self: Arc<Self>, app: AppHandle) -> Reply {
        let open: Vec<String> = serde_json::from_value(self.call("open_notebooks", Value::Null).await?).map_err(|e| e.to_string())?;
        for path in windows::paths(&app, &self.host) {
            if !open.contains(&path) {
                let opened = self.call("open_notebook", json!({"path": path})).await?;
                self.emit(&app, events::NOTEBOOK_CHANGED, opened);
            }
            let state = self.call("kernel_state", json!({"path": path})).await?;
            self.event(&app, events::KERNEL_STATUS, state);
        }
        Ok(Value::Null)
    }

    async fn check_version(self: Arc<Self>, app: AppHandle) {
        let mut conn = self.conn.subscribe();
        conn.borrow_and_update();
        let Some(running) = self.call("version", Value::Null).await.ok().and_then(|v| v.as_str().map(String::from)) else { return };
        let included = env!("CARGO_PKG_VERSION");
        if !older(&running, included) {
            return self.emit(&app, OUTDATED, Value::Null);
        }
        let notice = format!("The Colab Notebook server still runs version {running}. It restarts with the new version when no kernel is running.");
        while self.call("can_restart", Value::Null).await != Ok(Value::Bool(true)) {
            self.emit(&app, OUTDATED, json!(notice));
            tokio::select! {
                _ = conn.changed() => return,
                () = tokio::time::sleep(Duration::from_secs(10)) => {}
            }
        }
        log::info!("restarting colab-server {running} to run {included}");
        let _ = self.call("shutdown", Value::Null).await.map_err(|e| log::error!("stopping outdated colab-server: {e}"));
    }

    async fn connect_and_serve(self: &Arc<Self>, app: &AppHandle) -> anyhow::Result<()> {
        if self.host == LOCAL {
            let ws = colab_server::connect(&colab_server::socket_path()).await?;
            self.attached(app);
            self.session(app, ws).await;
        } else {
            let host = colab_server::hosts::load(&colab_server::hosts::file())?.into_iter().find(|h| h.label == self.host).ok_or_else(|| anyhow::anyhow!("no host named {}", self.host))?;
            let socks: Option<colab::Socks> = of(app, LOCAL).call("tailscale_proxy", Value::Null).await.ok().and_then(|v| serde_json::from_value(v).ok());
            let request = format!("{}/rpc", colab::base_url(&host.address).replacen("http://", "ws://", 1)).into_client_request()?;
            let ws = tokio::time::timeout(Duration::from_secs(5), colab::websocket(request, socks.as_ref())).await??;
            self.attached(app);
            self.session(app, ws).await;
        }
        Ok(())
    }

    fn attached(self: &Arc<Self>, app: &AppHandle) {
        let (s, a) = (self.clone(), app.clone());
        tokio::spawn(async move { s.refetch(a).await.map_err(|e| log::error!("refetch after connect: {e}")) });
        if self.host == LOCAL {
            tokio::spawn(self.clone().check_version(app.clone()));
        }
    }

    async fn run(self: Arc<Self>, app: AppHandle) {
        tokio::select! {
            () = self.serve(&app) => {}
            () = self.quit.notified() => {}
        }
        self.conn.send_replace(None);
        self.pending.lock().unwrap().clear();
    }

    async fn serve(self: &Arc<Self>, app: &AppHandle) {
        let (mut delay, mut started) = (Duration::from_millis(100), None::<Instant>);
        loop {
            match self.connect_and_serve(app).await {
                Ok(()) => {
                    delay = Duration::from_millis(100);
                    log::warn!("lost the connection to {}, reconnecting", self.name());
                    self.reconnecting(app);
                }
                Err(e) => {
                    if self.host == LOCAL && !self.stopped.load(Ordering::Relaxed) && started.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
                        log::info!("colab-server is not answering ({e}), starting it");
                        let _ = colab_server::spawn(&["server", "run"]).map_err(|e| log::error!("starting colab-server: {e}"));
                        started = Some(Instant::now());
                    }
                    tokio::select! {
                        () = tokio::time::sleep(delay) => {}
                        () = self.wake.notified() => {}
                    }
                    delay = (delay * 2).min(Duration::from_secs(5));
                }
            }
        }
    }
}
