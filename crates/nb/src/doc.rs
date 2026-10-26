use crate::Notebook;
use notify::{RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(1000);

pub fn merge(disk: &Notebook, app: &Notebook, active: Option<&str>, running: &HashSet<String>) -> Notebook {
    let mut merged = disk.clone();
    for cell in merged.cells.iter_mut().filter(|c| running.contains(&c.id)) {
        if let Some(mine) = app.cell(&cell.id) {
            (cell.outputs, cell.execution_count) = (mine.outputs.clone(), mine.execution_count);
        }
    }
    let Some((index, cell)) = active.and_then(|id| app.cells.iter().enumerate().find(|(_, c)| c.id == id)) else {
        return merged;
    };
    match merged.cells.iter_mut().find(|c| c.id == cell.id) {
        Some(slot) => *slot = cell.clone(),
        None => {
            let after = index.checked_sub(1).and_then(|i| merged.cells.iter().position(|c| c.id == app.cells[i].id));
            merged.cells.insert(after.map_or(0, |i| i + 1), cell.clone());
        }
    }
    merged
}

struct State {
    notebook: Notebook,
    active: Option<String>,
    running: HashSet<String>,
    on_disk: String,
}

enum Msg {
    Dirty,
    Fs,
    Flush(Sender<anyhow::Result<()>>),
    Stop,
}

type OnDirty = Arc<Mutex<Option<Box<dyn Fn(bool) + Send>>>>;

pub struct NotebookDoc {
    path: PathBuf,
    state: Arc<Mutex<State>>,
    on_dirty: OnDirty,
    tx: Sender<Msg>,
    _watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

impl NotebookDoc {
    pub fn open(path: impl AsRef<Path>, on_external_change: impl Fn(Notebook) + Send + 'static) -> anyhow::Result<Self> {
        let path = path.as_ref().canonicalize()?;
        let on_disk = std::fs::read_to_string(&path)?;
        let notebook = Notebook::parse(&on_disk)?;
        let state = Arc::new(Mutex::new(State { notebook, active: None, running: HashSet::new(), on_disk }));
        let (tx, rx) = channel();
        let watcher = Arc::new(Mutex::new(None));
        let (slot, watch_tx, watch_path) = (Arc::downgrade(&watcher), tx.clone(), path.clone());
        std::thread::spawn(move || {
            let (event_tx, event_path) = (watch_tx.clone(), watch_path.clone());
            let started = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|e| e.paths.iter().any(|p| p == &event_path)) {
                    let _ = event_tx.send(Msg::Fs);
                }
            }).and_then(|mut w| w.watch(watch_path.parent().unwrap(), RecursiveMode::NonRecursive).map(|_| w));
            match (started, slot.upgrade()) {
                (Ok(w), Some(slot)) => *slot.lock().unwrap() = Some(w),
                (Err(e), _) => return log::error!("watching {}: {e}", watch_path.display()),
                _ => return,
            }
            let _ = watch_tx.send(Msg::Fs);
        });
        let (worker_path, worker_state, on_dirty) = (path.clone(), state.clone(), OnDirty::default());
        let worker_dirty = on_dirty.clone();
        std::thread::spawn(move || run(worker_path, worker_state, rx, on_external_change, worker_dirty));
        Ok(Self { path, state, on_dirty, tx, _watcher: watcher })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn notebook(&self) -> Notebook {
        self.state.lock().unwrap().notebook.clone()
    }

    pub fn edit<R>(&self, f: impl FnOnce(&mut Notebook) -> R) -> R {
        let result = f(&mut self.state.lock().unwrap().notebook);
        let _ = self.tx.send(Msg::Dirty);
        result
    }

    pub fn set_active_cell(&self, id: Option<String>) {
        self.state.lock().unwrap().active = id;
    }

    pub fn on_dirty(&self, f: impl Fn(bool) + Send + 'static) {
        *self.on_dirty.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_running(&self, id: &str, running: bool) {
        let set = &mut self.state.lock().unwrap().running;
        if running { set.insert(id.into()) } else { set.remove(id) };
    }

    pub fn flush(&self) -> anyhow::Result<()> {
        let (tx, rx) = channel();
        self.tx.send(Msg::Flush(tx))?;
        rx.recv()?
    }
}

impl Drop for NotebookDoc {
    fn drop(&mut self) {
        let _ = self.flush();
        let _ = self.tx.send(Msg::Stop);
    }
}

fn run(path: PathBuf, state: Arc<Mutex<State>>, rx: Receiver<Msg>, on_change: impl Fn(Notebook), on_dirty: OnDirty) {
    let mut deadline: Option<Instant> = None;
    let mut dirty = false;
    let mut set_dirty = |value: bool| {
        if dirty != value {
            dirty = value;
            if let Some(f) = on_dirty.lock().unwrap().as_ref() {
                f(value);
            }
        }
    };
    loop {
        let msg = match deadline {
            Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())).ok(),
            None => rx.recv().ok().or(Some(Msg::Stop)),
        };
        match msg {
            Some(Msg::Dirty) => {
                deadline.get_or_insert_with(|| Instant::now() + DEBOUNCE);
                set_dirty(true);
            }
            Some(Msg::Fs) => {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let mut s = state.lock().unwrap();
                if text == s.on_disk {
                    continue;
                }
                let Ok(disk) = Notebook::parse(&text) else { continue };
                s.on_disk = text;
                s.notebook = merge(&disk, &s.notebook, s.active.as_deref(), &s.running);
                if s.notebook != disk {
                    deadline = Some(Instant::now() + DEBOUNCE);
                    set_dirty(true);
                }
                let notebook = s.notebook.clone();
                drop(s);
                on_change(notebook);
            }
            Some(Msg::Flush(reply)) => {
                deadline = None;
                let result = save(&path, &state);
                if result.is_ok() {
                    set_dirty(false);
                }
                let _ = reply.send(result);
            }
            Some(Msg::Stop) => return,
            None => {
                deadline = None;
                if save(&path, &state).is_ok() {
                    set_dirty(false);
                }
            }
        }
    }
}

fn save(path: &Path, state: &Mutex<State>) -> anyhow::Result<()> {
    let notebook = state.lock().unwrap().notebook.clone();
    let text = notebook.to_json()?;
    if text == state.lock().unwrap().on_disk {
        return Ok(());
    }
    let tmp = path.with_file_name(format!(".{}.tmp", path.file_name().unwrap().to_string_lossy()));
    std::fs::write(&tmp, &text)?;
    std::fs::File::open(&tmp)?.sync_all()?;
    std::fs::rename(&tmp, path)?;
    state.lock().unwrap().on_disk = text;
    Ok(())
}
