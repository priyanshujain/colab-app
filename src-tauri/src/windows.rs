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

pub const LOCAL: &str = "local";

pub fn host_of(label: &str) -> String {
    let hex = label.strip_prefix("remote-").and_then(|r| r.rsplit_once('-')).map(|(h, _)| h);
    let bytes = hex.and_then(|h| (0..h.len()).step_by(2).map(|i| h.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect::<Option<Vec<u8>>>());
    bytes.and_then(|b| String::from_utf8(b).ok()).unwrap_or_else(|| LOCAL.into())
}

impl Registry {
    pub fn owner(&self, host: &str, path: &str) -> Option<&str> {
        self.notebooks.iter().find(|(l, p)| *p == path && host_of(l) == host).map(|(l, _)| l.as_str())
    }

    pub fn claim(&mut self, label: &str, path: &str) -> Option<String> {
        match self.owner(&host_of(label), path) {
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

    pub fn place(&mut self, host: &str, path: &str, windows: &[String], action: &str) -> Place {
        if let Some(owner) = self.owner(host, path) {
            return Place::Focus(owner.into());
        }
        let label = windows.iter().find(|l| host_of(l) == host && self.is_blank(l)).cloned().unwrap_or_else(|| {
            let label = next_label(host);
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

fn next_label(host: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    if host == LOCAL { format!("window-{n}") } else { format!("remote-{}-{n}", host.bytes().map(|b| format!("{b:02x}")).collect::<String>()) }
}

fn build<R: Runtime>(app: &AppHandle<R>, label: &str) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::default()).title("").inner_size(1200.0, 850.0).min_inner_size(700.0, 500.0);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(9.0, 25.0));
    builder.build().map(drop)
}

pub fn create<R: Runtime>(app: &AppHandle<R>, host: &str, startup: &str) -> tauri::Result<()> {
    let label = next_label(host);
    registry(app).startup.insert(label.clone(), startup.into());
    build(app, &label)?;
    Ok(focus(app, &label))
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

pub fn focus_owner<R: Runtime>(app: &AppHandle<R>, host: &str, path: &str) -> bool {
    let owner = registry(app).owner(host, path).map(String::from);
    owner.map(|o| focus(app, &o)).is_some()
}

fn send<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>, action: &str) -> tauri::Result<()> {
    let ready = registry(app).deliver(window.label(), action);
    if ready { window.emit_to(window.label(), "menu", action) } else { Ok(()) }
}

fn focused<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
    app.webview_windows().into_values().find(|w| w.is_focused().unwrap_or(false))
}

pub fn menu<R: Runtime>(app: &AppHandle<R>, host: &str, id: &str) {
    if id.strip_prefix("open-recent:").is_some_and(|path| focus_owner(app, host, path)) {
        return;
    }
    let result = match focused(app).filter(|w| host_of(w.label()) == host) {
        Some(window) => send(app, &window, id),
        None => create(app, host, id),
    };
    let _ = result.map_err(|e| log::error!("menu {id}: {e}"));
}

pub fn settings<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let window = focused(app).filter(|w| host_of(w.label()) == LOCAL).or_else(|| app.webview_windows().into_values().find(|w| host_of(w.label()) == LOCAL));
    let result = match window {
        Some(window) => send(app, &window, id).map(|()| focus(app, window.label())),
        None => create(app, LOCAL, id),
    };
    let _ = result.map_err(|e| log::error!("settings: {e}"));
}

fn settings_file<R: Runtime>(app: &AppHandle<R>) -> std::path::PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("app.json")
}

pub fn opens_agent_notebooks<R: Runtime>(app: &AppHandle<R>) -> bool {
    let settings: serde_json::Value = std::fs::read_to_string(settings_file(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    settings["open_agent_notebooks"].as_bool().unwrap_or(false)
}

pub fn set_opens_agent_notebooks<R: Runtime>(app: &AppHandle<R>, on: bool) -> std::io::Result<()> {
    let file = settings_file(app);
    std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
    std::fs::write(file, serde_json::json!({"open_agent_notebooks": on}).to_string())
}

pub fn agent_open<R: Runtime>(app: &AppHandle<R>, host: &str, path: &str) {
    let mut windows: Vec<_> = app.webview_windows().into_values().collect();
    windows.sort_by_key(|w| !w.is_focused().unwrap_or(false));
    let labels: Vec<_> = windows.iter().map(|w| w.label().to_string()).collect();
    let action = format!("open-recent:{path}");
    let place = registry(app).place(host, path, &labels, &action);
    let result = match place {
        Place::Focus(owner) => Ok(focus(app, &owner)),
        Place::Load(label) => match app.get_webview_window(&label) {
            Some(window) => send(app, &window, &action),
            None => build(app, &label),
        },
    };
    let _ = result.map_err(|e| log::error!("agent open {path}: {e}"));
}

pub fn agent_close<R: Runtime>(app: &AppHandle<R>, host: &str, path: &str) {
    let owner = registry(app).owner(host, path).map(String::from);
    if let Some(window) = owner.and_then(|o| app.get_webview_window(&o)) {
        let _ = app.run_on_main_thread(move || { let _ = window.close().map_err(|e| log::error!("agent close: {e}")); }).map_err(|e| log::error!("agent close: {e}"));
    }
}

pub fn paths<R: Runtime>(app: &AppHandle<R>, host: &str) -> Vec<String> {
    registry(app).notebooks.iter().filter(|(l, _)| host_of(l) == host).map(|(_, p)| p.clone()).collect()
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
        assert_eq!(r.owner(LOCAL, "/nb/train.ipynb"), None);
        r.startup.insert("window-2".into(), "open-recent:/nb/data.ipynb".into());
        assert!(!r.is_blank("main") && !r.is_blank("window-2") && r.is_blank("window-3"));
        r.close("window-1");
        assert!(r.is_blank("window-1"));
        assert_eq!(r.claim("window-3", "/nb/eval.ipynb"), None);
        assert_eq!(r.owner(LOCAL, "/nb/eval.ipynb"), Some("window-3"));
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
        assert_eq!(r.place(LOCAL, "/nb/train.ipynb", &windows, "open-recent:/nb/train.ipynb"), Place::Load("main".into()));
        let Place::Load(second) = r.place(LOCAL, "/nb/eval.ipynb", &windows, "open-recent:/nb/eval.ipynb") else { panic!("eval was not given a window") };
        assert_ne!(second, "main");
        assert_eq!(r.take_startup(&second).as_deref(), Some("open-recent:/nb/eval.ipynb"));
        assert_eq!(r.place(LOCAL, "/nb/train.ipynb", &windows, "open-recent:/nb/train.ipynb"), Place::Focus("main".into()));
        assert_eq!(r.claim("main", "/nb/train.ipynb"), None);
        assert_eq!(r.claim(&second, "/nb/eval.ipynb"), None);
        let windows = vec!["main".to_string(), second.clone()];
        let Place::Load(third) = r.place(LOCAL, "/nb/data.ipynb", &windows, "open-recent:/nb/data.ipynb") else { panic!("data was not given a window") };
        assert!(!windows.contains(&third));
        r.close("main");
        assert_eq!(r.place(LOCAL, "/nb/train.ipynb", &[second.clone(), "window-blank".into()], "open-recent:/nb/train.ipynb"), Place::Load("window-blank".into()));
    }

    #[test]
    fn the_same_path_on_two_hosts_opens_in_two_windows_and_events_find_the_right_one() {
        let mut r = Registry::default();
        let gpu = next_label("gpu box");
        assert!(gpu.starts_with("remote-") && !gpu.contains(' '), "{gpu}");
        assert_eq!((host_of(&gpu).as_str(), host_of("main").as_str(), host_of("window-4").as_str(), host_of("remote-zz-3").as_str()), ("gpu box", LOCAL, LOCAL, LOCAL));
        assert_eq!(r.claim("main", "/home/pj/train.ipynb"), None);
        assert_eq!(r.claim(&gpu, "/home/pj/train.ipynb"), None);
        assert_eq!(r.owner("gpu box", "/home/pj/train.ipynb"), Some(gpu.as_str()));
        assert_eq!(r.owner(LOCAL, "/home/pj/train.ipynb"), Some("main"));
        let windows = vec!["main".to_string(), gpu.clone(), "window-blank".to_string()];
        let Place::Load(label) = r.place("gpu box", "/home/pj/eval.ipynb", &windows, "open-recent:/home/pj/eval.ipynb") else { panic!("eval was not given a window") };
        assert_eq!(host_of(&label), "gpu box");
        assert_eq!(r.place(LOCAL, "/home/pj/eval.ipynb", &windows, "open-recent:/home/pj/eval.ipynb"), Place::Load("window-blank".into()));
    }
}
