use crate::ipc::{self, *};
use crate::outputs;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use anyhow::{Context, Result, anyhow, bail};
use colab::{Auth, AuthError, Colab, ColabKernel, OAuthClient, store};
use futures::StreamExt;
use chrono::{DateTime, SecondsFormat, Utc};
use kernel::{Channel, Content, ExecutionState, Kernel, KernelStatus, Message};
use kernel_local::LocalKernel;
use nb::{Cell, CellType, NotebookDoc};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, Weak};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell, RwLock, watch};
use tooling::{CodeAction, Project, Tooling, cell_uri, text_edits};

const FRAME: Duration = Duration::from_millis(33);

pub type Emit = Arc<dyn Fn(&str, Value) + Send + Sync>;

enum Active {
    Local(LocalKernel),
    Colab(ColabKernel),
}

impl Active {
    fn k(&self) -> &dyn Kernel {
        match self {
            Self::Local(k) => k,
            Self::Colab(k) => k,
        }
    }
}

struct Tasks(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Tasks {
    fn drop(&mut self) {
        self.0.iter().for_each(|t| t.abort());
    }
}

struct Session {
    active: Active,
    choice: KernelChoice,
    _tasks: Tasks,
}

struct Open {
    path: PathBuf,
    key: String,
    doc: NotebookDoc,
    kernel: RwLock<Option<Session>>,
    choice: StdMutex<Option<KernelChoice>>,
    run: Mutex<()>,
    cancel: watch::Sender<u64>,
    displays: StdMutex<HashMap<String, Vec<(String, usize)>>>,
    project: OnceCell<Arc<Project>>,
    sync: Mutex<()>,
    actions: StdMutex<(Vec<CodeAction>, Synced)>,
}

pub struct Core {
    emit: Emit,
    data_dir: PathBuf,
    notebooks: StdMutex<HashMap<String, Arc<Open>>>,
    tooling: OnceCell<Arc<Tooling>>,
    auth: Mutex<Option<Arc<Auth>>>,
    auth_state: StdMutex<Option<String>>,
}

fn colab_variant(v: ipc::Variant) -> colab::Variant {
    match v {
        ipc::Variant::Default => colab::Variant::Default,
        ipc::Variant::Gpu => colab::Variant::Gpu,
        ipc::Variant::Tpu => colab::Variant::Tpu,
    }
}

fn ipc_variant(v: &Value) -> ipc::Variant {
    match (v.as_str(), v.as_u64()) {
        (Some("GPU"), _) | (_, Some(1)) => ipc::Variant::Gpu,
        (Some("TPU"), _) | (_, Some(2)) => ipc::Variant::Tpu,
        _ => ipc::Variant::Default,
    }
}

impl Core {
    pub fn new(emit: Emit, data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self { emit, data_dir, notebooks: Default::default(), tooling: OnceCell::new(), auth: Mutex::new(None), auth_state: Default::default() })
    }

    pub async fn install_tools(&self) {
        if let Err(e) = self.tooling().await {
            log::error!("installing tools: {e:#}");
        }
    }

    fn send<T: Serialize>(&self, event: &str, payload: T) {
        (self.emit)(event, serde_json::to_value(payload).unwrap_or_default());
    }

    async fn tooling(&self) -> Result<Arc<Tooling>> {
        let dir = self.data_dir.join("tools");
        self.tooling.get_or_try_init(|| async move { Tooling::new(&dir).await.map(Arc::new) }).await.cloned()
    }

    fn get(&self, path: &str) -> Result<Arc<Open>> {
        let key = Path::new(path).canonicalize().map_or(path.to_string(), |p| p.to_string_lossy().into());
        self.notebooks.lock().unwrap().get(&key).cloned().with_context(|| format!("notebook is not open: {path}"))
    }

    pub async fn open_notebook(self: &Arc<Self>, path: &str) -> Result<OpenedNotebook> {
        let canonical = Path::new(path).canonicalize().with_context(|| format!("opening {path}"))?;
        let key = canonical.to_string_lossy().to_string();
        if let Some(open) = self.notebooks.lock().unwrap().get(&key) {
            return Ok(OpenedNotebook { path: key, notebook: open.doc.notebook() });
        }
        let (weak, handle, k) = (Arc::downgrade(self), tokio::runtime::Handle::current(), key.clone());
        let doc = NotebookDoc::open(&canonical, move |notebook| {
            let Some(core) = weak.upgrade() else { return };
            core.send(events::NOTEBOOK_CHANGED, NotebookChanged { path: k.clone(), notebook });
            if let Ok(open) = core.get(&k) {
                handle.spawn(async move { open.sync_tooling().await });
            }
        })?;
        let (weak, k) = (Arc::downgrade(self), key.clone());
        doc.on_dirty(move |dirty| {
            if let Some(core) = weak.upgrade() {
                core.send(events::SAVE_STATE, SaveState { path: k.clone(), dirty });
            }
        });
        let open = Arc::new(Open {
            path: canonical.clone(),
            key: key.clone(),
            doc,
            kernel: RwLock::new(None),
            choice: Default::default(),
            run: Mutex::new(()),
            cancel: watch::channel(0).0,
            displays: Default::default(),
            project: OnceCell::new(),
            sync: Mutex::new(()),
            actions: Default::default(),
        });
        self.notebooks.lock().unwrap().insert(key.clone(), open.clone());
        let notebook = open.doc.notebook();
        let (core, o) = (self.clone(), open.clone());
        tokio::spawn(async move { core.start_project(o).await.map_err(|e| log::error!("tooling: {e:#}")) });
        if store::get(&store::default_path(), &canonical).ok().flatten().is_some() {
            let (core, o) = (self.clone(), open.clone());
            tokio::spawn(async move { core.reattach(o).await.map_err(|e| log::error!("reattach: {e:#}")) });
        }
        Ok(OpenedNotebook { path: key, notebook })
    }

    pub async fn rename_notebook(self: &Arc<Self>, path: &str, new_name: &str) -> Result<String> {
        let old = self.get(path)?;
        let name = new_name.trim();
        let name = if name.ends_with(".ipynb") { name.to_string() } else { format!("{name}.ipynb") };
        if name.contains('/') || name.starts_with('.') {
            bail!("invalid notebook name: {new_name}");
        }
        let target = old.path.with_file_name(&name);
        if target.exists() {
            bail!("{} already exists", target.display());
        }
        let _run = old.run.lock().await;
        old.doc.flush()?;
        std::fs::rename(&old.path, &target)?;
        self.notebooks.lock().unwrap().remove(&old.key);
        let session = old.kernel.write().await.take();
        let opened = self.open_notebook(&target.to_string_lossy()).await?;
        let new = self.get(&opened.path)?;
        *new.choice.lock().unwrap() = old.choice.lock().unwrap().clone();
        let store_path = store::default_path();
        if let Some(saved) = store::get(&store_path, &old.path)? {
            store::save(&store_path, &new.path, &saved)?;
            store::remove(&store_path, &old.path)?;
        }
        if let Some(Session { active, choice, .. }) = session {
            let active = match active {
                Active::Colab(mut k) => {
                    k.set_notebook(&new.path);
                    Active::Colab(k)
                }
                local => local,
            };
            self.install(&new, active, choice).await;
        }
        Ok(opened.path)
    }

    async fn start_project(self: Arc<Self>, open: Arc<Open>) -> Result<()> {
        let project = self.project(&open).await?;
        open.sync_tooling().await;
        let mut rx = project.subscribe();
        let prefix = cell_uri(&open.path, "");
        let weak = Arc::downgrade(&open);
        drop(open);
        loop {
            match rx.recv().await {
                Ok(d) => {
                    let Some(open) = weak.upgrade() else { return Ok(()) };
                    if let Some(cell_id) = d.uri.strip_prefix(&prefix) {
                        self.send(lsp_events::DIAGNOSTICS, Diagnostics { path: open.key.clone(), cell_id: cell_id.into(), diagnostics: d.diagnostics });
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => return Ok(()),
            }
        }
    }

    async fn project(&self, open: &Open) -> Result<Arc<Project>> {
        let tooling = self.tooling().await?;
        let root = kernel_local::project_root(&open.path);
        open.project.get_or_try_init(|| async move { tooling.project(&root).await }).await.cloned()
    }

    pub async fn edit_cell(&self, path: &str, edit: CellEdit) -> Result<()> {
        let open = self.get(path)?;
        if let CellEdit::Focus { cell_id } = edit {
            open.doc.set_active_cell(cell_id);
            return Ok(());
        }
        open.doc.edit(|nb| {
            let find = |nb: &nb::Notebook, id: &str| nb.cells.iter().position(|c| c.id == id).with_context(|| format!("no cell {id}"));
            match edit {
                CellEdit::Source { cell_id, source } => {
                    let i = find(nb, &cell_id)?;
                    nb.cells[i].source = source;
                }
                CellEdit::Insert { cell_id, index, cell_type } => {
                    let cell = Cell { id: cell_id, ..Cell::new(cell_type, "") };
                    nb.cells.insert((index as usize).min(nb.cells.len()), cell);
                }
                CellEdit::Delete { cell_id } => {
                    let i = find(nb, &cell_id)?;
                    nb.cells.remove(i);
                }
                CellEdit::Move { cell_id, index } => {
                    let cell = nb.cells.remove(find(nb, &cell_id)?);
                    nb.cells.insert((index as usize).min(nb.cells.len()), cell);
                }
                CellEdit::ChangeType { cell_id, cell_type } => {
                    let i = find(nb, &cell_id)?;
                    let cell = &mut nb.cells[i];
                    let code = cell_type == CellType::Code;
                    cell.cell_type = cell_type;
                    cell.outputs = code.then(|| cell.outputs.take().unwrap_or_default());
                    cell.execution_count = code.then(|| cell.execution_count.flatten());
                }
                CellEdit::Focus { .. } => {}
            }
            anyhow::Ok(())
        })?;
        open.sync_tooling().await;
        Ok(())
    }

    pub fn open_paths(&self) -> Vec<String> {
        self.notebooks.lock().unwrap().keys().cloned().collect()
    }

    pub async fn agent_open(self: &Arc<Self>, path: &str) -> Result<String> {
        let file = Path::new(path);
        if !file.is_absolute() || file.extension().is_none_or(|x| x != "ipynb") {
            bail!("path must be an absolute .ipynb path: {path}");
        }
        if !file.exists() {
            std::fs::write(file, nb::Notebook::parse(r#"{"nbformat": 4, "nbformat_minor": 5, "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3"}}, "cells": []}"#)?.to_json()?)?;
        }
        let opened = self.open_notebook(path).await?;
        self.send(events::AGENT_OPEN, AgentOpen { path: opened.path.clone() });
        Ok(opened.path)
    }

    pub async fn kernel_state(&self, path: &str) -> Result<KernelStatusEvent> {
        let open = self.get(path)?;
        let guard = open.kernel.read().await;
        let choice = guard.as_ref().map(|s| s.choice.clone()).or_else(|| open.choice.lock().unwrap().clone());
        Ok(KernelStatusEvent { path: open.key.clone(), status: guard.as_ref().map_or(KernelStatus::Disconnected, |s| s.active.k().status()), choice })
    }

    pub async fn agent_connect(self: &Arc<Self>, path: &str, choice: KernelChoice) -> Result<bool> {
        let state = self.kernel_state(path).await?;
        let live = !matches!(state.status, KernelStatus::Dead | KernelStatus::Disconnected);
        if live && serde_json::to_value(&state.choice)? == serde_json::to_value(Some(&choice))? {
            return Ok(false);
        }
        self.select_kernel(path, choice.clone()).await?;
        if let KernelChoice::Local = choice {
            self.ensure_kernel(&self.get(path)?).await?;
        }
        Ok(true)
    }

    pub fn notebook(&self, path: &str) -> Result<nb::Notebook> {
        Ok(self.get(path)?.doc.notebook())
    }

    fn follow(&self, open: &Open, cell_id: &str) {
        self.send(events::AGENT_FOCUS, AgentFocus { path: open.key.clone(), cell_id: cell_id.into() });
    }

    pub async fn agent_edit(&self, path: &str, edit: CellEdit) -> Result<()> {
        let open = self.get(path)?;
        let focus = match &edit {
            CellEdit::Source { cell_id, .. } | CellEdit::Insert { cell_id, .. } => Some(cell_id.clone()),
            _ => None,
        };
        self.edit_cell(path, edit).await?;
        self.send(events::NOTEBOOK_CHANGED, NotebookChanged { path: open.key.clone(), notebook: open.doc.notebook() });
        focus.inspect(|id| self.follow(&open, id));
        Ok(())
    }

    pub async fn agent_run(self: &Arc<Self>, path: &str, cell_id: &str) -> Result<Cell> {
        let open = self.get(path)?;
        open.doc.notebook().cell(cell_id).with_context(|| format!("no cell {cell_id}"))?;
        self.follow(&open, cell_id);
        self.run_cell(path, cell_id).await?;
        open.doc.notebook().cell(cell_id).cloned().with_context(|| format!("cell {cell_id} was deleted while running"))
    }

    fn emit_cell(&self, open: &Open, cell_id: &str, running: bool) {
        let snapshot = open.doc.edit(|nb| nb.cell(cell_id).map(|c| (c.outputs.clone().unwrap_or_default(), c.execution_count.flatten(), c.metadata.get("execution").cloned())));
        if let Some((outputs, execution_count, execution)) = snapshot {
            self.send(events::CELL_OUTPUTS, CellOutputs { path: open.key.clone(), cell_id: cell_id.into(), outputs, execution_count, execution, running });
        }
    }

    pub async fn run_cell(self: &Arc<Self>, path: &str, cell_id: &str) -> Result<()> {
        let open = self.get(path)?;
        let _run = open.run.lock().await;
        self.ensure_kernel(&open).await?;
        self.execute(&open, cell_id).await.map(drop)
    }

    pub async fn run_all(self: &Arc<Self>, path: &str) -> Result<()> {
        let open = self.get(path)?;
        let _run = open.run.lock().await;
        self.ensure_kernel(&open).await?;
        let ids: Vec<String> = open.doc.notebook().cells.into_iter().filter(|c| c.cell_type == CellType::Code).map(|c| c.id).collect();
        for id in ids {
            if !self.execute(&open, &id).await? {
                break;
            }
        }
        Ok(())
    }

    async fn execute(&self, open: &Open, cell_id: &str) -> Result<bool> {
        let Some(source) = open.doc.notebook().cell(cell_id).filter(|c| c.cell_type == CellType::Code).map(|c| c.source.clone()) else {
            return Ok(true);
        };
        if source.trim().is_empty() {
            return Ok(true);
        }
        let mut cancel = open.cancel.subscribe();
        let mut exec = match open.kernel.read().await.as_ref() {
            Some(s) => s.active.k().execute(&source).await?,
            None => bail!("no kernel selected"),
        };
        open.doc.edit(|nb| {
            if let Some(c) = nb.cells.iter_mut().find(|c| c.id == cell_id) {
                c.outputs = Some(Vec::new());
                c.execution_count = Some(None);
                c.metadata.insert("execution".into(), json!({"iopub.status.busy": stamp(Utc::now())}));
            }
        });
        open.doc.set_running(cell_id, true);
        open.displays.lock().unwrap().values_mut().for_each(|v| v.retain(|(c, _)| c != cell_id));
        self.emit_cell(open, cell_id, true);
        let (mut clear_wait, mut ok, mut cancelled, mut dirty, mut last) = (false, true, false, false, Instant::now());
        loop {
            let next = tokio::select! {
                _ = cancel.changed() => { (ok, cancelled) = (false, true); break }
                _ = tokio::time::sleep_until((last + FRAME).into()), if dirty => None,
                m = exec.next() => match m { Some(m) => Some(m), None => break },
            };
            if let Some(msg) = next {
                ok &= msg.header.msg_type != "error";
                let key = match msg.typed() {
                    Content::Status { execution_state: ExecutionState::Busy } => Some("iopub.status.busy"),
                    Content::ExecuteInput { .. } => Some("iopub.execute_input"),
                    Content::Status { execution_state: ExecutionState::Idle } => Some("iopub.status.idle"),
                    _ => None,
                };
                if let Some(key) = key {
                    timing(open, cell_id, key, msg.header.date, true);
                }
                dirty |= self.handle(open, cell_id, &mut clear_wait, msg);
            }
            if dirty && last.elapsed() >= FRAME {
                self.emit_cell(open, cell_id, true);
                (dirty, last) = (false, Instant::now());
            }
        }
        let now = Utc::now();
        if !cancelled {
            timing(open, cell_id, "shell.execute_reply", now, false);
        }
        timing(open, cell_id, "iopub.status.idle", now, false);
        open.doc.set_running(cell_id, false);
        self.emit_cell(open, cell_id, false);
        Ok(ok)
    }

    fn handle(&self, open: &Open, cell_id: &str, clear_wait: &mut bool, msg: Message) -> bool {
        let content = msg.typed();
        match content {
            Content::ExecuteInput { execution_count, .. } => {
                open.doc.edit(|nb| nb.cells.iter_mut().filter(|c| c.id == cell_id).for_each(|c| c.execution_count = Some(Some(execution_count))));
                true
            }
            Content::InputRequest { prompt, password } => {
                self.send(lsp_events::INPUT_REQUEST, InputRequest { path: open.key.clone(), cell_id: cell_id.into(), prompt, password });
                false
            }
            Content::CommOpen { .. } | Content::CommMsg { .. } | Content::CommClose { .. } => {
                self.emit_comm(open, &msg);
                false
            }
            Content::UpdateDisplayData { data, metadata, transient } => {
                for cell in self.update_display(open, &transient, data, metadata) {
                    if cell != cell_id {
                        self.emit_cell(open, &cell, false);
                    }
                }
                true
            }
            content => {
                let display_id = match &content {
                    Content::DisplayData { transient, .. } => transient["display_id"].as_str().map(String::from),
                    _ => None,
                };
                let index = open.doc.edit(|nb| {
                    let cell = nb.cells.iter_mut().find(|c| c.id == cell_id)?;
                    let outputs = cell.outputs.get_or_insert_default();
                    let cleared = outputs::apply(outputs, clear_wait, content);
                    Some((cleared, outputs.len().checked_sub(1)))
                });
                let mut displays = open.displays.lock().unwrap();
                if let Some((cleared, last)) = index {
                    if cleared {
                        displays.values_mut().for_each(|v| v.retain(|(c, _)| c != cell_id));
                    }
                    if let (Some(id), Some(i)) = (display_id, last) {
                        displays.entry(id).or_default().push((cell_id.into(), i));
                    }
                }
                true
            }
        }
    }

    fn update_display(&self, open: &Open, transient: &Value, data: Value, metadata: Value) -> Vec<String> {
        let Some(id) = transient["display_id"].as_str() else { return Vec::new() };
        let targets = open.displays.lock().unwrap().get(id).cloned().unwrap_or_default();
        open.doc.edit(|nb| {
            for (cell, i) in &targets {
                if let Some(o) = nb.cells.iter_mut().find(|c| &c.id == cell).and_then(|c| c.outputs.as_mut()).and_then(|o| o.get_mut(*i)) {
                    outputs::update(o, data.clone(), metadata.clone());
                }
            }
        });
        targets.into_iter().map(|(c, _)| c).collect()
    }

    fn emit_comm(&self, open: &Open, msg: &Message) {
        self.send(events::COMM, CommMessage {
            path: open.key.clone(),
            comm_id: msg.content["comm_id"].as_str().unwrap_or_default().into(),
            msg_type: msg.header.msg_type.clone(),
            target_name: msg.content["target_name"].as_str().map(String::from),
            buffers: msg.buffers.iter().map(|b| BASE64.encode(b)).collect(),
            data: msg.content.get("data").cloned().unwrap_or(Value::Null),
            metadata: Value::Object(msg.metadata.clone()),
        });
    }

    pub async fn send_comm(&self, message: CommMessage) -> Result<()> {
        let open = self.get(&message.path)?;
        let mut content = json!({"comm_id": message.comm_id, "data": message.data});
        if let Some(target) = message.target_name.filter(|_| message.msg_type == "comm_open") {
            content["target_name"] = target.into();
        }
        let mut msg = Message::new(&message.msg_type, "", content);
        msg.metadata = message.metadata.as_object().cloned().unwrap_or_default();
        msg.buffers = message.buffers.iter().map(|b| BASE64.decode(b)).collect::<Result<_, _>>()?;
        msg.channel = Some(Channel::Shell);
        open.kernel_send(msg).await
    }

    pub async fn send_input(&self, path: &str, value: String) -> Result<()> {
        let mut msg = Message::new("input_reply", "", json!({"value": value, "status": "ok"}));
        msg.channel = Some(Channel::Stdin);
        self.get(path)?.kernel_send(msg).await
    }

    pub async fn interrupt(&self, path: &str) -> Result<()> {
        let open = self.get(path)?;
        let guard = open.kernel.read().await;
        guard.as_ref().context("no kernel selected")?.active.k().interrupt().await
    }

    pub async fn restart(&self, path: &str) -> Result<()> {
        let open = self.get(path)?;
        open.cancel.send_modify(|n| *n += 1);
        let guard = open.kernel.read().await;
        guard.as_ref().context("no kernel selected")?.active.k().restart().await
    }

    async fn drop_kernel(&self, open: &Open, release: bool) -> Result<()> {
        open.cancel.send_modify(|n| *n += 1);
        let Some(session) = open.kernel.write().await.take() else { return Ok(()) };
        let result = match session.active {
            Active::Local(k) => k.shutdown().await,
            Active::Colab(k) if release => k.release().await,
            Active::Colab(k) => k.detach(),
        };
        self.send(events::KERNEL_STATUS, KernelStatusEvent { path: open.key.clone(), status: KernelStatus::Disconnected, choice: None });
        result
    }

    pub async fn select_kernel(self: &Arc<Self>, path: &str, choice: KernelChoice) -> Result<()> {
        let open = self.get(path)?;
        open.cancel.send_modify(|n| *n += 1);
        let _run = open.run.lock().await;
        self.drop_kernel(&open, true).await?;
        *open.choice.lock().unwrap() = Some(choice.clone());
        if let KernelChoice::Local = choice {
            self.send(events::KERNEL_STATUS, KernelStatusEvent { path: open.key.clone(), status: KernelStatus::Disconnected, choice: Some(choice) });
            return Ok(());
        }
        if self.reuse_detached(&open, &choice).await? {
            return Ok(());
        }
        self.start_kernel(&open, choice).await
    }

    async fn reuse_detached(self: &Arc<Self>, open: &Arc<Open>, choice: &KernelChoice) -> Result<bool> {
        let KernelChoice::Colab { variant, accelerator, .. } = choice else { return Ok(false) };
        let colab = self.colab().await?;
        let Some(kernel) = colab.reattach(&open.path).await? else { return Ok(false) };
        let assigned = colab.assignments().await?.into_iter().find(|a| a.endpoint == kernel.runtime.endpoint);
        let same = assigned.is_some_and(|a| ipc_variant(&a.variant) == *variant && accelerator.as_ref().is_none_or(|acc| *acc == a.accelerator));
        if !same {
            log::info!("releasing detached runtime {} of {} for a different one", kernel.runtime.endpoint, open.key);
            kernel.release().await?;
            return Ok(false);
        }
        log::info!("reusing detached runtime {} for {}", kernel.runtime.endpoint, open.key);
        self.install(open, Active::Colab(kernel), choice.clone()).await;
        Ok(true)
    }

    async fn ensure_kernel(self: &Arc<Self>, open: &Arc<Open>) -> Result<()> {
        let local = matches!(*open.choice.lock().unwrap(), None | Some(KernelChoice::Local));
        if !local || open.kernel.read().await.is_some() {
            return Ok(());
        }
        *open.choice.lock().unwrap() = Some(KernelChoice::Local);
        self.start_kernel(open, KernelChoice::Local).await
    }

    async fn start_kernel(self: &Arc<Self>, open: &Arc<Open>, choice: KernelChoice) -> Result<()> {
        self.send(events::KERNEL_STATUS, KernelStatusEvent { path: open.key.clone(), status: KernelStatus::Connecting, choice: Some(choice.clone()) });
        log::info!("starting {choice:?} kernel for {}", open.key);
        let clock = std::time::Instant::now();
        let started = async {
            Ok(match &choice {
                KernelChoice::Local => {
                    let uv = self.tooling().await?.tools().uv.clone();
                    Active::Local(LocalKernel::start(&open.path, Some(&uv)).await?)
                }
                KernelChoice::Colab { variant, accelerator, high_ram } => {
                    Active::Colab(self.colab().await?.start(&open.path, colab_variant(*variant), accelerator.as_deref(), *high_ram).await?)
                }
            })
        };
        match started.await {
            Ok(active) => {
                log::info!("started {choice:?} kernel for {} in {:?}", open.key, clock.elapsed());
                self.install(open, active, choice).await;
                Ok(())
            }
            Err(e) => {
                log::error!("starting {choice:?} kernel for {} failed after {:?}: {e:#}", open.key, clock.elapsed());
                self.send(events::KERNEL_STATUS, KernelStatusEvent { path: open.key.clone(), status: KernelStatus::Disconnected, choice: None });
                Err(e)
            }
        }
    }

    async fn install(self: &Arc<Self>, open: &Arc<Open>, active: Active, choice: KernelChoice) {
        let mut messages = active.k().subscribe();
        let (core, weak) = (self.clone(), Arc::downgrade(open));
        let comms = tokio::spawn(async move {
            while let Some(msg) = messages.next().await {
                let Some(open) = weak.upgrade() else { return };
                match msg.typed() {
                    Content::CommOpen { .. } | Content::CommMsg { .. } | Content::CommClose { .. } => core.emit_comm(&open, &msg),
                    Content::UpdateDisplayData { data, metadata, transient } => {
                        core.update_display(&open, &transient, data, metadata).into_iter().for_each(|c| core.emit_cell(&open, &c, false))
                    }
                    _ => {}
                }
            }
        });
        let status = tokio::spawn(watch_status(self.clone(), Arc::downgrade(open)));
        *open.kernel.write().await = Some(Session { active, choice, _tasks: Tasks(vec![comms, status]) });
    }

    async fn reattach(self: Arc<Self>, open: Arc<Open>) -> Result<()> {
        let colab = self.colab().await?;
        let Some(kernel) = colab.reattach(&open.path).await? else { return Ok(()) };
        let a = colab.assignments().await?.into_iter().find(|a| a.endpoint == kernel.runtime.endpoint);
        let choice = KernelChoice::Colab {
            variant: a.as_ref().map_or(ipc::Variant::Default, |a| ipc_variant(&a.variant)),
            accelerator: a.map(|a| a.accelerator).filter(|a| !a.is_empty() && a != "NONE"),
            high_ram: false,
        };
        if open.kernel.read().await.is_some() || open.choice.lock().unwrap().is_some() {
            return kernel.detach();
        }
        *open.choice.lock().unwrap() = Some(choice.clone());
        self.install(&open, Active::Colab(kernel), choice).await;
        Ok(())
    }

    pub async fn detach(&self, path: &str) -> Result<()> {
        let open = self.get(path)?;
        self.drop_kernel(&open, false).await
    }

    pub async fn release(&self, path: &str) -> Result<()> {
        let open = self.get(path)?;
        let colab = matches!(open.kernel.read().await.as_ref().map(|s| &s.active), Some(Active::Colab(_)));
        if colab {
            return self.drop_kernel(&open, true).await;
        }
        let Some(saved) = store::get(&store::default_path(), &open.path)? else { return Ok(()) };
        self.colab().await?.unassign(&saved.endpoint).await?;
        store::remove(&store::default_path(), &open.path)
    }

    async fn auth(&self) -> Result<Option<Arc<Auth>>> {
        let mut auth = self.auth.lock().await;
        if auth.is_none() {
            let import_cli = !self.data_dir.join("signed-out").exists();
            *auth = tokio::task::spawn_blocking(move || Auth::load(import_cli)).await??.map(Arc::new);
        }
        Ok(auth.clone())
    }

    async fn colab(&self) -> Result<Colab> {
        Ok(Colab::new(self.auth().await?.context("not signed in to Google")?))
    }

    async fn state_of(&self, auth: Option<Arc<Auth>>) -> AuthState {
        let Some(auth) = auth else { return AuthState::SignedOut };
        match auth.email().await {
            Ok(email) => AuthState::SignedIn { email },
            Err(e) => match e.downcast_ref::<AuthError>() {
                Some(AuthError::ClientRejected(_)) => AuthState::ClientRejected { message: e.to_string() },
                _ => AuthState::SignedOut,
            },
        }
    }

    fn publish(&self, state: &AuthState) {
        let text = serde_json::to_string(state).unwrap_or_default();
        if self.auth_state.lock().unwrap().replace(text.clone()).as_ref() != Some(&text) {
            self.send(events::AUTH_STATE, state);
        }
    }

    pub async fn auth_state(&self) -> Result<AuthState> {
        let state = self.state_of(self.auth().await?).await;
        self.publish(&state);
        Ok(state)
    }

    pub async fn sign_in(&self, client_id: Option<String>, client_secret: Option<String>) -> Result<AuthState> {
        let client = match client_id.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            Some(id) => OAuthClient { client_id: id.into(), client_secret: client_secret.unwrap_or_default().trim().into() },
            None => OAuthClient::default(),
        };
        let state = match Auth::sign_in(client).await {
            Ok(auth) => {
                let auth = Arc::new(auth);
                let _ = std::fs::remove_file(self.data_dir.join("signed-out"));
                *self.auth.lock().await = Some(auth.clone());
                self.state_of(Some(auth)).await
            }
            Err(e) => match e.downcast_ref::<AuthError>() {
                Some(AuthError::ClientRejected(_)) => AuthState::ClientRejected { message: e.to_string() },
                _ => return Err(e),
            },
        };
        self.publish(&state);
        Ok(state)
    }

    pub async fn set_auth(&self, auth: Auth) {
        *self.auth.lock().await = Some(Arc::new(auth));
    }

    pub async fn force_reconnect(&self, path: &str) -> Result<()> {
        match self.get(path)?.kernel.read().await.as_ref().map(|s| &s.active) {
            Some(Active::Colab(k)) => {
                k.force_reconnect();
                Ok(())
            }
            _ => bail!("no Colab kernel"),
        }
    }

    pub async fn sign_out(&self) -> Result<()> {
        let mut auth = self.auth.lock().await;
        tokio::task::spawn_blocking(Auth::sign_out).await??;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::write(self.data_dir.join("signed-out"), "")?;
        *auth = None;
        drop(auth);
        self.publish(&AuthState::SignedOut);
        Ok(())
    }

    pub async fn colab_info(&self) -> Result<ColabInfo> {
        let i = self.colab().await?.ccu_info().await?;
        Ok(ColabInfo { eligible_gpus: i.eligible_gpus, eligible_tpus: i.eligible_tpus })
    }

    pub async fn list_runtimes(&self) -> Result<Vec<ipc::Runtime>> {
        let assignments = self.colab().await?.assignments().await?;
        Ok(assignments.into_iter().map(|a| ipc::Runtime { variant: ipc_variant(&a.variant), endpoint: a.endpoint, accelerator: a.accelerator }).collect())
    }

    pub async fn clone_link(&self, url: &str, dest: &str) -> Result<String> {
        Ok(colab::clone::clone(url, Path::new(dest)).await?.to_string_lossy().into())
    }

    async fn cell_request(&self, pos: &CellPosition) -> Result<(Arc<Project>, String)> {
        let open = self.get(&pos.path)?;
        let project = self.project(&open).await?;
        Ok((project, cell_uri(&open.path, &pos.cell_id)))
    }

    pub async fn completion(&self, pos: CellPosition) -> Result<Value> {
        let (p, uri) = self.cell_request(&pos).await?;
        p.completion(&uri, pos.line, pos.character).await
    }

    pub async fn hover(&self, pos: CellPosition) -> Result<Value> {
        let (p, uri) = self.cell_request(&pos).await?;
        p.hover(&uri, pos.line, pos.character).await
    }

    pub async fn definition(&self, pos: CellPosition) -> Result<Value> {
        let (p, uri) = self.cell_request(&pos).await?;
        p.definition(&uri, pos.line, pos.character).await
    }

    pub async fn prepare_rename(&self, pos: CellPosition) -> Result<Value> {
        let (p, uri) = self.cell_request(&pos).await?;
        self.get(&pos.path)?.sync_tooling().await;
        p.prepare_rename(&uri, pos.line, pos.character).await
    }

    pub async fn rename_symbol(&self, pos: CellPosition, new_name: &str) -> Result<()> {
        let (p, uri) = self.cell_request(&pos).await?;
        let open = self.get(&pos.path)?;
        let synced = open.sync_tooling().await;
        let edit = p.rename(&uri, pos.line, pos.character, new_name).await?;
        if edit.is_null() { bail!("nothing to rename here") }
        self.apply_edits(&open, text_edits(&edit), &synced).await
    }

    pub async fn format(&self, path: &str, cell_id: Option<&str>) -> Result<()> {
        self.organize_imports(path, cell_id).await?;
        let open = self.get(path)?;
        let project = self.project(&open).await?;
        let synced = open.sync_tooling().await;
        let mut edits = HashMap::new();
        for c in open.doc.notebook().cells.into_iter().filter(|c| c.cell_type == CellType::Code && cell_id.is_none_or(|id| id == c.id)) {
            let uri = cell_uri(&open.path, &c.id);
            edits.insert(uri.clone(), project.format(&uri).await?.as_array().cloned().unwrap_or_default());
        }
        self.apply_edits(&open, edits, &synced).await
    }

    pub async fn organize_imports(&self, path: &str, cell_id: Option<&str>) -> Result<()> {
        let open = self.get(path)?;
        let project = self.project(&open).await?;
        let synced = open.sync_tooling().await;
        let Some(first) = open.doc.notebook().cells.into_iter().find(|c| c.cell_type == CellType::Code && cell_id.is_none_or(|id| id == c.id)) else { return Ok(()) };
        let mut edits = text_edits(&project.organize_imports(&cell_uri(&open.path, &first.id)).await?);
        if let Some(id) = cell_id {
            edits.retain(|uri, _| *uri == cell_uri(&open.path, id));
        }
        self.apply_edits(&open, edits, &synced).await
    }

    pub async fn code_actions(&self, pos: CellPosition) -> Result<Vec<CodeActionItem>> {
        let (project, uri) = self.cell_request(&pos).await?;
        let synced = self.get(&pos.path)?.sync_tooling().await;
        let p = json!({"line": pos.line, "character": pos.character});
        let actions: Vec<CodeAction> = project.code_actions(&uri, json!({"start": p, "end": p}), Some(&["quickfix"])).await?;
        let items = actions.iter().enumerate().map(|(id, a)| CodeActionItem { id: id as u32, title: a.action["title"].as_str().unwrap_or_default().into() }).collect();
        *self.get(&pos.path)?.actions.lock().unwrap() = (actions, synced);
        Ok(items)
    }

    pub async fn apply_code_action(&self, path: &str, id: u32) -> Result<()> {
        let open = self.get(path)?;
        let (action, synced) = {
            let actions = open.actions.lock().unwrap();
            (actions.0.get(id as usize).cloned().context("code action is gone")?, actions.1.clone())
        };
        let edit = self.project(&open).await?.resolve(&action).await?;
        self.apply_edits(&open, text_edits(&edit), &synced).await
    }

    async fn apply_edits(&self, open: &Open, edits: HashMap<String, Vec<Value>>, synced: &Synced) -> Result<()> {
        let mut stale = false;
        let changed = open.doc.edit(|nb| {
            stale = nb.cells.iter().any(|c| { let uri = cell_uri(&open.path, &c.id); edits.contains_key(&uri) && synced.get(&uri) != Some(&c.source) });
            let mut changed = false;
            for c in nb.cells.iter_mut().filter(|_| !stale) {
                if let Some(e) = edits.get(&cell_uri(&open.path, &c.id)) {
                    let text = outputs::apply_text_edits(&c.source, &Value::from(e.clone()));
                    changed |= text != c.source;
                    c.source = text;
                }
            }
            changed
        });
        if changed {
            self.send(events::NOTEBOOK_CHANGED, NotebookChanged { path: open.key.clone(), notebook: open.doc.notebook() });
            open.sync_tooling().await;
        }
        if stale { bail!("the cell changed while the edit was computed, nothing was applied; try again") }
        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<String> {
        let file = match url::Url::parse(path) {
            Ok(u) if u.scheme() == "file" => u.to_file_path().map_err(|_| anyhow!("bad file url {path}"))?,
            _ => PathBuf::from(path),
        };
        std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))
    }
}

impl Open {
    async fn kernel_send(&self, msg: Message) -> Result<()> {
        let guard = self.kernel.read().await;
        guard.as_ref().context("no kernel selected")?.active.k().send(msg).await
    }

    async fn sync_tooling(&self) -> Synced {
        let Some(project) = self.project.get() else { return Synced::new() };
        let _lock = self.sync.lock().await;
        let cells: Vec<(String, String)> = self.doc.notebook().cells.into_iter().filter(|c| c.cell_type == CellType::Code).map(|c| (c.id, c.source)).collect();
        if let Err(e) = project.sync(&self.path, &cells).await {
            log::error!("tooling sync: {e:#}");
            return Synced::new();
        }
        cells.into_iter().map(|(id, source)| (cell_uri(&self.path, &id), source)).collect()
    }
}

type Synced = HashMap<String, String>;

fn stamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn timing(open: &Open, cell_id: &str, key: &str, at: DateTime<Utc>, replace: bool) {
    open.doc.edit(|nb| {
        if let Some(Value::Object(m)) = nb.cells.iter_mut().find(|c| c.id == cell_id).and_then(|c| c.metadata.get_mut("execution")) {
            if replace || !m.contains_key(key) {
                m.insert(key.into(), stamp(at).into());
            }
        }
    });
}

async fn watch_status(core: Arc<Core>, weak: Weak<Open>) {
    let mut last = None;
    loop {
        let Some(open) = weak.upgrade() else { return };
        if let Some(s) = open.kernel.read().await.as_ref() {
            let status = s.active.k().status();
            if last != Some(status) {
                if matches!(status, KernelStatus::Dead | KernelStatus::Disconnected) {
                    open.cancel.send_modify(|n| *n += 1);
                }
                core.send(events::KERNEL_STATUS, KernelStatusEvent { path: open.key.clone(), status, choice: Some(s.choice.clone()) });
                last = Some(status);
            }
        }
        drop(open);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
