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

    pub fn close(&mut self, label: &str) {
        self.notebooks.remove(label);
        self.startup.remove(label);
        self.ready.remove(label);
    }
}

pub type Windows = Mutex<Registry>;

fn registry<R: Runtime>(app: &AppHandle<R>) -> std::sync::MutexGuard<'_, Registry> {
    app.state::<Windows>().inner().lock().unwrap()
}

pub fn create<R: Runtime>(app: &AppHandle<R>, startup: &str) -> tauri::Result<()> {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let label = format!("window-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    registry(app).startup.insert(label.clone(), startup.into());
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::default()).title("").inner_size(1200.0, 850.0).min_inner_size(700.0, 500.0);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(9.0, 25.0));
    builder.build().map(drop)
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
    if focus_owner(app, path) {
        return;
    }
    let blank = app.webview_windows().into_values().filter(|w| registry(app).is_blank(w.label())).max_by_key(|w| w.is_focused().unwrap_or(false));
    let result = match blank {
        Some(window) => send(app, &window, &format!("open-recent:{path}")),
        None => create(app, &format!("open-recent:{path}")),
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
}
