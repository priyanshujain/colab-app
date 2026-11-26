mod menu;
mod server;
mod tray;
mod windows;

use serde_json::{Value, json};
use windows::{LOCAL, host_of};
use tauri::Manager;

type Result<T> = std::result::Result<T, String>;

fn snake(params: Value) -> Value {
    let Value::Object(map) = params else { return params };
    map.into_iter().map(|(k, v)| (k.chars().flat_map(|c| if c.is_ascii_uppercase() { vec!['_', c.to_ascii_lowercase()] } else { vec![c] }).collect::<String>(), v)).collect()
}

#[tauri::command]
async fn call(handle: tauri::AppHandle, window: tauri::WebviewWindow, method: String, params: Option<Value>) -> Result<Value> {
    let params = snake(params.unwrap_or_default());
    let host = host_of(window.label());
    let result = server::of(&handle, &host).call(&method, params.clone()).await?;
    let recent = match method.as_str() {
        "open_notebook" => {
            let path = result["path"].as_str().unwrap_or_default();
            if !windows::claim(&handle, window.label(), path) {
                return Ok(Value::Null);
            }
            let name = std::path::Path::new(path).file_name().map_or("", |n| n.to_str().unwrap_or_default());
            drop(window.set_title(&if host == LOCAL { name.into() } else { format!("{name} [{host}]") }));
            menu::opened(&handle, &host, path, None)
        }
        "rename_notebook" => {
            let path = params["path"].as_str().unwrap_or_default();
            let old = if host == LOCAL { std::fs::canonicalize(path).map_or(path.into(), |p| p.to_string_lossy().into_owned()) } else { path.into() };
            menu::opened(&handle, &host, result.as_str().unwrap_or_default(), Some(&old))
        }
        _ => Ok(()),
    };
    if let Err(e) = recent {
        log::error!("recent menu: {e:#}");
    }
    Ok(result)
}

#[tauri::command]
fn window_startup(handle: tauri::AppHandle, window: tauri::WebviewWindow) -> Option<String> {
    windows::startup(&handle, window.label())
}

#[tauri::command]
fn window_host(window: tauri::WebviewWindow) -> Option<String> {
    Some(host_of(window.label())).filter(|h| h != LOCAL)
}

#[tauri::command]
async fn sign_in(handle: tauri::AppHandle, window: tauri::WebviewWindow, client_id: Option<String>, client_secret: Option<String>) -> Result<Value> {
    let server = server::of(&handle, &host_of(window.label()));
    let loopback = colab::Loopback::bind().await.map_err(|e| format!("{e:#}"))?;
    let url = server.call("begin_sign_in", json!({"client_id": client_id, "client_secret": client_secret, "redirect_uri": loopback.redirect_uri})).await?;
    let url = url.as_str().unwrap_or_default();
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    let code = loopback.code(url).await.map_err(|e| format!("{e:#}"))?;
    server.call("complete_sign_in", json!({"code": code, "redirect_uri": loopback.redirect_uri})).await
}

#[tauri::command]
fn list_hosts() -> Result<Vec<colab_server::hosts::Host>> {
    colab_server::hosts::load(&colab_server::hosts::file()).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn add_host(handle: tauri::AppHandle, label: String, address: String) -> Result<Vec<colab_server::hosts::Host>> {
    let hosts = colab_server::hosts::add(&colab_server::hosts::file(), &label, &address).map_err(|e| format!("{e:#}"))?;
    let _ = menu::refresh(&handle).map_err(|e| log::error!("menu: {e:#}"));
    Ok(hosts)
}

#[tauri::command]
fn remove_host(handle: tauri::AppHandle, label: String) -> Result<Vec<colab_server::hosts::Host>> {
    let hosts = colab_server::hosts::remove(&colab_server::hosts::file(), &label).map_err(|e| format!("{e:#}"))?;
    let _ = menu::refresh(&handle).map_err(|e| log::error!("menu: {e:#}"));
    Ok(hosts)
}

#[tauri::command]
async fn check_link(url: String) -> Result<String> {
    colab::clone::fetch(&url).await.map(|f| f.name).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn open_url(url: String) -> Result<()> {
    if !url.starts_with("https://") && !url.starts_with("http://") { return Err(format!("refusing to open {url}")) }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_monospace_fonts() -> Result<Vec<String>> {
    tauri::async_runtime::spawn_blocking(monospace_fonts).await.map_err(|e| e.to_string())
}

fn monospace_fonts() -> Vec<String> {
    use font_kit::{family_name::FamilyName, properties::Properties, source::SystemSource};
    let source = SystemSource::new();
    let mut fonts: Vec<String> = source.all_families().unwrap_or_default().into_iter()
        .filter(|f| !f.starts_with('.'))
        .filter(|f| source.select_best_match(&[FamilyName::Title(f.clone())], &Properties::new())
            .ok().and_then(|h| h.load().ok()).is_some_and(|font| font.is_monospace()))
        .collect();
    fonts.push("JetBrains Mono".into());
    fonts.sort_unstable_by_key(|f| f.to_lowercase());
    fonts.dedup();
    fonts
}

#[tauri::command]
async fn list_fonts() -> Result<Vec<String>> {
    static FONTS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    if let Some(f) = FONTS.get() {
        return Ok(f.clone());
    }
    let fonts = tauri::async_runtime::spawn_blocking(all_fonts).await.map_err(|e| e.to_string())?;
    Ok(FONTS.get_or_init(|| fonts).clone())
}

fn all_fonts() -> Vec<String> {
    let mut fonts: Vec<String> = font_kit::source::SystemSource::new().all_families().unwrap_or_default().into_iter().filter(|f| !f.starts_with('.')).collect();
    fonts.extend(["Inter".into(), "JetBrains Mono".into()]);
    fonts.sort_unstable_by_key(|f| f.to_lowercase());
    fonts.dedup();
    fonts
}

#[tauri::command]
fn open_agent_notebooks(handle: tauri::AppHandle) -> bool {
    windows::opens_agent_notebooks(&handle)
}

#[tauri::command]
fn set_open_agent_notebooks(handle: tauri::AppHandle, on: bool) -> Result<()> {
    windows::set_opens_agent_notebooks(&handle, on).map_err(|e| e.to_string())
}

#[tauri::command]
fn server_status(handle: tauri::AppHandle) -> &'static str {
    server::of(&handle, LOCAL).status()
}

#[tauri::command]
fn start_server(handle: tauri::AppHandle) {
    server::of(&handle, LOCAL).start()
}

#[tauri::command]
async fn stop_server(handle: tauri::AppHandle, restart: bool) {
    server::of(&handle, LOCAL).stop(restart).await
}

#[tauri::command]
fn file_menu(window: tauri::Window, path: String) -> Result<()> {
    menu::file_menu(&window, &path).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).level_for("rmcp", log::LevelFilter::Warn).level_for("tracing", log::LevelFilter::Warn).max_file_size(5_000_000).rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3)).timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .menu(menu::initial)
        .on_menu_event(menu::clicked)
        .manage(windows::Windows::default())
        .manage(server::Servers::default())
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Destroyed => {
                windows::closed(window.app_handle(), window.label());
                server::closed(window.app_handle(), window.label());
            }
            tauri::WindowEvent::Focused(true) => { let _ = menu::refresh(window.app_handle()).map_err(|e| log::error!("menu: {e:#}")); }
            _ => {}
        })
        .setup(|app| {
            server::of(app.handle(), LOCAL);
            tray::create(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, window_startup, window_host, sign_in, list_hosts, add_host, remove_host, check_link, open_url, list_monospace_fonts, list_fonts, open_agent_notebooks, set_open_agent_notebooks, server_status, start_server, stop_server, file_menu])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            #[cfg(target_os = "macos")]
            tauri::RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { has_visible_windows: false, .. } => { let _ = windows::create(app, LOCAL, "").map_err(|e| log::error!("new window: {e}")); }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn lists_installed_monospace_fonts_and_skips_proportional_ones() {
        let fonts = super::monospace_fonts();
        assert!(fonts.iter().any(|f| f == "Menlo"), "{fonts:?}");
        assert!(fonts.iter().any(|f| f == "JetBrains Mono"));
        assert!(!fonts.iter().any(|f| f == "Helvetica" || f == "Times New Roman"), "{fonts:?}");
        let all = super::all_fonts();
        assert!(["Helvetica", "Menlo", "Inter", "JetBrains Mono"].iter().all(|f| all.iter().any(|a| a == f)), "{all:?}");
        assert!(!all.iter().any(|f| f.starts_with('.')) && all.is_sorted_by_key(|f| f.to_lowercase()), "{all:?}");
    }
}
