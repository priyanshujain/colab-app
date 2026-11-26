use crate::engine::Core;
use crate::ipc::events;
use anyhow::{Context, Result, bail};
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures::{SinkExt, StreamExt};
use kernel::KernelStatus;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Notify, broadcast, mpsc};

pub type Events = broadcast::Sender<(Option<u64>, String, Value)>;

const ORDERED: [&str; 9] = ["edit_cell", "rename_notebook", "format_cell", "format_notebook", "organize_imports", "apply_code_action", "rename_symbol", "run_cell", "run_all"];
const LOCAL_ONLY: [&str; 7] = ["shutdown", "can_restart", "tailscale_status", "tailscale_enable", "tailscale_disable", "tailscale_logout", "tailscale_proxy"];

tokio::task_local! {
    pub static ORIGIN: u64;
}

#[derive(Clone)]
pub struct Rpc {
    pub core: Arc<Core>,
    pub events: Events,
    pub shutdown: Arc<Notify>,
    pub tailscale: Option<Arc<crate::tailscale::Tailscale>>,
}

pub fn router(rpc: Rpc) -> axum::Router {
    axum::Router::new().route("/rpc", axum::routing::get(upgrade)).with_state(rpc)
}

async fn upgrade(ws: WebSocketUpgrade, State(rpc): State<Rpc>) -> axum::response::Response {
    ws.max_message_size(1 << 30).on_upgrade(move |socket| client(socket, rpc))
}

async fn client(socket: WebSocket, rpc: Rpc) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let me = NEXT.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();
    let mut events = rpc.events.subscribe();
    let local = rpc.tailscale.is_some();
    let (replies, mut outbox) = mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                reply = outbox.recv() => match reply { Some(r) => r, None => break },
                event = events.recv() => match event {
                    Ok((Some(origin), event, _)) if origin == me && event == events::NOTEBOOK_CHANGED => continue,
                    Ok((_, event, _)) if !local && event == events::TAILSCALE_STATUS => continue,
                    Ok((_, event, payload)) => json!({"event": event, "payload": payload}),
                    Err(broadcast::error::RecvError::Lagged(n)) => { log::warn!("rpc client lagged by {n} events, closing it"); break }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            if sink.send(Message::Text(frame.to_string().into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let (ordered, mut queue) = mpsc::unbounded_channel::<Value>();
    let (serial, serial_replies) = (rpc.clone(), replies.clone());
    tokio::spawn(async move {
        while let Some(request) = queue.recv().await {
            let run = request["method"].as_str().is_some_and(|m| m.starts_with("run_"));
            let task = answer(serial.clone(), me, request, serial_replies.clone());
            if run { drop(tokio::spawn(task)) } else { task.await }
        }
    });
    while let Some(Ok(message)) = stream.next().await {
        let Message::Text(text) = message else { continue };
        let Ok(request) = serde_json::from_str::<Value>(&text) else { continue };
        if request["method"].as_str().is_some_and(|m| ORDERED.contains(&m)) {
            let _ = ordered.send(request);
        } else {
            tokio::spawn(answer(rpc.clone(), me, request, replies.clone()));
        }
        if writer.is_finished() {
            break;
        }
    }
    writer.abort();
}

async fn answer(rpc: Rpc, me: u64, request: Value, replies: mpsc::UnboundedSender<Value>) {
    let method = request["method"].as_str().unwrap_or_default();
    let origin = if method == "edit_cell" { me } else { 0 };
    let reply = match ORIGIN.scope(origin, dispatch(&rpc, method, request.get("params").cloned().unwrap_or(Value::Null))).await {
        Ok(result) => json!({"id": request["id"], "result": result}),
        Err(e) => json!({"id": request["id"], "error": format!("{e:#}")}),
    };
    let _ = replies.send(reply);
}

async fn can_restart(rpc: &Rpc, hosted: &crate::kernels::KernelHost) -> bool {
    for path in rpc.core.open_paths() {
        if rpc.core.kernel_state(&path).await.is_ok_and(|s| !matches!(s.status, KernelStatus::Dead | KernelStatus::Disconnected)) {
            return false;
        }
    }
    hosted.alive() == 0
}

fn arg<T: DeserializeOwned>(params: &Value, name: &str) -> Result<T> {
    serde_json::from_value(params.get(name).cloned().unwrap_or(Value::Null)).with_context(|| format!("argument {name}"))
}

async fn dispatch(rpc: &Rpc, method: &str, p: Value) -> Result<Value> {
    let core = &rpc.core;
    let path = || arg::<String>(&p, "path");
    let to = |v: Result<_>| v.map(|()| Value::Null);
    let tailscale = || rpc.tailscale.as_ref().context("Tailscale is not available");
    if rpc.tailscale.is_none() && LOCAL_ONLY.contains(&method) {
        bail!("{method} is not available over Tailscale");
    }
    Ok(match method {
        "open_notebook" => json!(core.open_notebook(&path()?).await?),
        "open_notebooks" => json!(core.open_paths()),
        "rename_notebook" => json!(core.rename_notebook(&path()?, &arg::<String>(&p, "new_name")?).await?),
        "edit_cell" => to(core.edit_cell(&path()?, arg(&p, "edit")?).await)?,
        "run_cell" => to(core.run_cell(&path()?, &arg::<String>(&p, "cell_id")?).await)?,
        "run_all" => to(core.run_all(&path()?).await)?,
        "interrupt" => to(core.interrupt(&path()?).await)?,
        "restart" => to(core.restart(&path()?).await)?,
        "select_kernel" => to(core.select_kernel(&path()?, arg(&p, "choice")?).await)?,
        "kernel_state" => json!(core.kernel_state(&path()?).await?),
        "runtime_started" => json!(core.runtime_started(&path()?).await?),
        "diagnostics" => json!(core.diagnostics(&path()?)?),
        "send_comm" => to(core.send_comm(arg(&p, "message")?).await)?,
        "begin_sign_in" => json!(core.begin_sign_in(arg(&p, "client_id")?, arg(&p, "client_secret")?, &arg::<String>(&p, "redirect_uri")?)?),
        "complete_sign_in" => json!(core.complete_sign_in(&arg::<String>(&p, "code")?, &arg::<String>(&p, "redirect_uri")?).await?),
        "version" => json!(env!("CARGO_PKG_VERSION")),
        "machine" => json!(core.machine()),
        "can_restart" => json!(can_restart(rpc, &tailscale()?.kernels).await),
        "tailscale_status" => json!(tailscale()?.status()),
        "tailscale_enable" => json!(tailscale()?.enable(arg(&p, "hostname")?).await?),
        "tailscale_disable" => json!(tailscale()?.disable().await?),
        "tailscale_logout" => json!(tailscale()?.logout().await?),
        "tailscale_proxy" => json!(tailscale()?.proxy()),
        "host_status" => json!(core.host_status().await?),
        "list_dir" => json!(list_dir(&path()?)?),
        "sign_out" => to(core.sign_out().await)?,
        "auth_state" => json!(core.auth_state().await?),
        "colab_info" => json!(core.colab_info().await?),
        "list_runtimes" => json!(core.list_runtimes().await?),
        "detach" => to(core.detach(&path()?).await)?,
        "release" => to(core.release(&path()?).await)?,
        "clone_link" => json!(core.clone_link(&arg::<String>(&p, "url")?, &arg::<String>(&p, "dest")?).await?),
        "completion" => core.completion(arg(&p, "pos")?).await?,
        "hover" => core.hover(arg(&p, "pos")?).await?,
        "definition" => core.definition(arg(&p, "pos")?).await?,
        "prepare_rename" => core.prepare_rename(arg(&p, "pos")?).await?,
        "rename_symbol" => to(core.rename_symbol(arg(&p, "pos")?, &arg::<String>(&p, "new_name")?).await)?,
        "format_cell" => to(core.format(&path()?, Some(&arg::<String>(&p, "cell_id")?)).await)?,
        "format_notebook" => to(core.format(&path()?, None).await)?,
        "organize_imports" => to(core.organize_imports(&path()?, arg::<Option<String>>(&p, "cell_id")?.as_deref()).await)?,
        "code_actions" => json!(core.code_actions(arg(&p, "pos")?).await?),
        "apply_code_action" => to(core.apply_code_action(&path()?, arg(&p, "id")?).await)?,
        "read_file" => json!(core.read_file(&path()?)?),
        "send_input" => to(core.send_input(&path()?, arg(&p, "value")?).await)?,
        "list_secrets" => json!(core.list_secrets().await?),
        "set_secret" => to(core.set_secret(&arg::<String>(&p, "name")?, &arg::<String>(&p, "value")?).await)?,
        "delete_secret" => to(core.delete_secret(&arg::<String>(&p, "name")?).await)?,
        "shutdown" => {
            rpc.shutdown.notify_one();
            Value::Null
        }
        _ => bail!("unknown method {method}"),
    })
}

#[derive(serde::Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(serde::Serialize)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
}

pub fn list_dir(path: &str) -> Result<Listing> {
    let dir = if path.is_empty() { dirs::home_dir().context("no home directory")? } else { std::fs::canonicalize(path).with_context(|| format!("{path} does not exist"))? };
    let mut entries: Vec<Entry> = std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?.flatten()
        .map(|e| Entry { name: e.file_name().to_string_lossy().into_owned(), dir: e.path().is_dir() })
        .filter(|e| e.dir || e.name.ends_with(".ipynb"))
        .collect();
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing { path: dir.to_string_lossy().into_owned(), entries })
}
