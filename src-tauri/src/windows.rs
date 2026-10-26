use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime, TitleBarStyle, WebviewUrl, WebviewWindowBuilder};

#[derive(Default)]
pub struct Registry {
    notebooks: HashMap<String, String>,
    startup: HashMap<String, String>,
    ready: HashSet<String>,
}

impl Registry {
    pub fn owner(&self, path: &str) -> Option<&str> {
        self.notebooks.iter().find(|(_, p)| *p == path).map(|(l, _)| l.as_str())
    }

    pub fn claim(&mut self, label: &str, path: &str) -> Option<String> {
        match self.owner(path) {
            Some(owner) if owner != label => Some(owner.into()),
            _ => (self.notebooks.insert(label.into(), path.into()), None).1,
        }
    }

    pub fn is_blank(&self, label: &str) -> bool {
        !self.notebooks.contains_key(label) && !self.startup.contains_key(label)
    }

    pub fn take_startup(&mut self, label: &str) -> Option<String> {
        self.ready.insert(label.into());
        self.startup.remove(label)
    }

    pub fn deliver(&mut self, label: &str, action: &str) -> bool {
        self.ready.contains(label) || (self.startup.insert(label.into(), action.into()), false).1
    }

    pub fn place(&mut self, path: &str, windows: &[String], action: &str) -> Place {
        if let Some(owner) = self.owner(path) {
            return Place::Focus(owner.into());
        }
        let label = windows.iter().find(|l| self.is_blank(l)).cloned().unwrap_or_else(|| {
            let label = next_label();
            self.startup.insert(label.clone(), action.into());
            label
        });
        self.notebooks.insert(label.clone(), path.into());
        Place::Load(label)
    }

    pub fn close(&mut self, label: &str) {
        self.notebooks.remove(label);
        self.startup.remove(label);
        self.ready.remove(label);
    }
}

#[derive(Debug, PartialEq)]
pub enum Place {
    Focus(String),
    Load(String),
}

pub type Windows = Mutex<Registry>;

fn registry<R: Runtime>(app: &AppHandle<R>) -> std::sync::MutexGuard<'_, Registry> {
    app.state::<Windows>().inner().lock().unwrap()
}

fn next_label() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    format!("window-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn build<R: Runtime>(app: &AppHandle<R>, label: &str) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::default()).title("").inner_size(1200.0, 850.0).min_inner_size(700.0, 500.0);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(9.0, 25.0));
    builder.build().map(drop)
}

pub fn create<R: Runtime>(app: &AppHandle<R>, startup: &str) -> tauri::Result<()> {
    let label = next_label();
    registry(app).startup.insert(label.clone(), startup.into());
    build(app, &label)
}

pub fn startup<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<String> {
    registry(app).take_startup(label)
}

fn focus<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        drop(window.unminimize());
        drop(window.set_focus());
    }
}

pub fn claim<R: Runtime>(app: &AppHandle<R>, label: &str, path: &str) -> bool {
    let owner = registry(app).claim(label, path);
    owner.map(|o| focus(app, &o)).is_none()
}

fn focus_owner<R: Runtime>(app: &AppHandle<R>, path: &str) -> bool {
    let owner = registry(app).owner(path).map(String::from);
    owner.map(|o| focus(app, &o)).is_some()
}

fn send<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>, action: &str) -> tauri::Result<()> {
    let ready = registry(app).deliver(window.label(), action);
    if ready { window.emit_to(window.label(), "menu", action) } else { Ok(()) }
}

fn focused<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
    app.webview_windows().into_values().find(|w| w.is_focused().unwrap_or(false))
}

pub fn menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if id.strip_prefix("open-recent:").is_some_and(|path| focus_owner(app, path)) {
        return;
    }
    let result = match focused(app) {
        Some(window) => send(app, &window, id),
        None => create(app, id),
    };
    let _ = result.map_err(|e| log::error!("menu {id}: {e}"));
}

pub fn agent_open<R: Runtime>(app: &AppHandle<R>, path: &str) {
    let mut windows: Vec<_> = app.webview_windows().into_values().collect();
    windows.sort_by_key(|w| !w.is_focused().unwrap_or(false));
    let labels: Vec<_> = windows.iter().map(|w| w.label().to_string()).collect();
    let action = format!("open-recent:{path}");
    let place = registry(app).place(path, &labels, &action);
    let result = match place {
        Place::Focus(owner) => Ok(focus(app, &owner)),
        Place::Load(label) => match app.get_webview_window(&label) {
            Some(window) => send(app, &window, &action),
            None => build(app, &label),
        },
    };
    let _ = result.map_err(|e| log::error!("agent open {path}: {e}"));
}

pub fn closed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    registry(app).close(label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notebook_belongs_to_one_window_and_frees_up_when_that_window_closes() {
        let mut r = Registry::default();
        assert_eq!(r.claim("main", "/nb/train.ipynb"), None);
        assert_eq!(r.claim("window-1", "/nb/train.ipynb").as_deref(), Some("main"));
        assert_eq!(r.claim("window-1", "/nb/eval.ipynb"), None);
        assert_eq!(r.claim("main", "/nb/train.ipynb"), None);
        assert_eq!(r.claim("main", "/nb/data.ipynb"), None);
        assert_eq!(r.owner("/nb/train.ipynb"), None);
        r.startup.insert("window-2".into(), "open-recent:/nb/data.ipynb".into());
        assert!(!r.is_blank("main") && !r.is_blank("window-2") && r.is_blank("window-3"));
        r.close("window-1");
        assert!(r.is_blank("window-1"));
        assert_eq!(r.claim("window-3", "/nb/eval.ipynb"), None);
        assert_eq!(r.owner("/nb/eval.ipynb"), Some("window-3"));
    }

    #[test]
    fn an_action_for_a_window_still_loading_waits_until_its_page_asks_for_it() {
        let mut r = Registry::default();
        assert!(!r.deliver("main", "open-recent:/nb/train.ipynb"));
        assert!(!r.is_blank("main"));
        assert_eq!(r.take_startup("main").as_deref(), Some("open-recent:/nb/train.ipynb"));
        assert!(r.deliver("main", "open"));
        assert_eq!(r.take_startup("main"), None);
        assert_eq!(r.take_startup("window-1"), None);
        r.close("main");
        assert!(!r.deliver("main", "settings"));
    }

    #[test]
    fn notebooks_the_agent_opens_back_to_back_each_get_their_own_window() {
        let mut r = Registry::default();
        r.take_startup("main");
        let windows = vec!["main".to_string()];
        assert_eq!(r.place("/nb/train.ipynb", &windows, "open-recent:/nb/train.ipynb"), Place::Load("main".into()));
        let Place::Load(second) = r.place("/nb/eval.ipynb", &windows, "open-recent:/nb/eval.ipynb") else { panic!("eval was not given a window") };
        assert_ne!(second, "main");
        assert_eq!(r.take_startup(&second).as_deref(), Some("open-recent:/nb/eval.ipynb"));
        assert_eq!(r.place("/nb/train.ipynb", &windows, "open-recent:/nb/train.ipynb"), Place::Focus("main".into()));
        assert_eq!(r.claim("main", "/nb/train.ipynb"), None);
        assert_eq!(r.claim(&second, "/nb/eval.ipynb"), None);
        let windows = vec!["main".to_string(), second.clone()];
        let Place::Load(third) = r.place("/nb/data.ipynb", &windows, "open-recent:/nb/data.ipynb") else { panic!("data was not given a window") };
        assert!(!windows.contains(&third));
        r.close("main");
        assert_eq!(r.place("/nb/train.ipynb", &[second.clone(), "window-blank".into()], "open-recent:/nb/train.ipynb"), Place::Load("window-blank".into()));
    }
}
