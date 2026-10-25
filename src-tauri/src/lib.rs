pub mod engine;
pub mod ipc;
pub mod mcp;
mod menu;
mod outputs;

use crate::engine::Core;
use ipc::*;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

type Result<T> = std::result::Result<T, String>;
type App<'a> = State<'a, Arc<Core>>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
async fn open_notebook(handle: tauri::AppHandle, app: App<'_>, path: String) -> Result<OpenedNotebook> {
    let opened = app.open_notebook(&path).await.map_err(err)?;
    if let Err(e) = menu::opened(&handle, &opened.path, None) {
        eprintln!("recent menu: {e:#}");
    }
    Ok(opened)
}

#[tauri::command]
async fn rename_notebook(handle: tauri::AppHandle, app: App<'_>, path: String, new_name: String) -> Result<String> {
    let old = std::fs::canonicalize(&path).map_or(path.clone(), |p| p.to_string_lossy().into());
    let renamed = app.rename_notebook(&path, &new_name).await.map_err(err)?;
    if let Err(e) = menu::opened(&handle, &renamed, Some(&old)) {
        eprintln!("recent menu: {e:#}");
    }
    Ok(renamed)
}

#[tauri::command]
async fn edit_cell(app: App<'_>, path: String, edit: CellEdit) -> Result<()> {
    app.edit_cell(&path, edit).await.map_err(err)
}

#[tauri::command]
async fn run_cell(app: App<'_>, path: String, cell_id: String) -> Result<()> {
    app.run_cell(&path, &cell_id).await.map_err(err)
}

#[tauri::command]
async fn run_all(app: App<'_>, path: String) -> Result<()> {
    app.run_all(&path).await.map_err(err)
}

#[tauri::command]
async fn interrupt(app: App<'_>, path: String) -> Result<()> {
    app.interrupt(&path).await.map_err(err)
}

#[tauri::command]
async fn restart(app: App<'_>, path: String) -> Result<()> {
    app.restart(&path).await.map_err(err)
}

#[tauri::command]
async fn kernel_state(app: App<'_>, path: String) -> Result<KernelStatusEvent> {
    app.kernel_state(&path).await.map_err(err)
}

#[tauri::command]
async fn select_kernel(app: App<'_>, path: String, choice: KernelChoice) -> Result<()> {
    app.select_kernel(&path, choice).await.map_err(err)
}

#[tauri::command]
async fn send_comm(app: App<'_>, message: CommMessage) -> Result<()> {
    app.send_comm(message).await.map_err(err)
}

#[tauri::command]
async fn sign_in(app: App<'_>, client_id: Option<String>, client_secret: Option<String>) -> Result<AuthState> {
    app.sign_in(client_id, client_secret).await.map_err(err)
}

#[tauri::command]
async fn sign_out(app: App<'_>) -> Result<()> {
    app.sign_out().await.map_err(err)
}

#[tauri::command]
async fn auth_state(app: App<'_>) -> Result<AuthState> {
    app.auth_state().await.map_err(err)
}

#[tauri::command]
async fn colab_info(app: App<'_>) -> Result<ColabInfo> {
    app.colab_info().await.map_err(err)
}

#[tauri::command]
async fn list_runtimes(app: App<'_>) -> Result<Vec<Runtime>> {
    app.list_runtimes().await.map_err(err)
}

#[tauri::command]
async fn detach(app: App<'_>, path: String) -> Result<()> {
    app.detach(&path).await.map_err(err)
}

#[tauri::command]
async fn release(app: App<'_>, path: String) -> Result<()> {
    app.release(&path).await.map_err(err)
}

#[tauri::command]
async fn clone_link(app: App<'_>, url: String, dest: String) -> Result<String> {
    app.clone_link(&url, &dest).await.map_err(err)
}

#[tauri::command]
async fn completion(app: App<'_>, pos: CellPosition) -> Result<serde_json::Value> {
    app.completion(pos).await.map_err(err)
}

#[tauri::command]
async fn hover(app: App<'_>, pos: CellPosition) -> Result<serde_json::Value> {
    app.hover(pos).await.map_err(err)
}

#[tauri::command]
async fn definition(app: App<'_>, pos: CellPosition) -> Result<serde_json::Value> {
    app.definition(pos).await.map_err(err)
}

#[tauri::command]
async fn prepare_rename(app: App<'_>, pos: CellPosition) -> Result<serde_json::Value> {
    app.prepare_rename(pos).await.map_err(err)
}

#[tauri::command]
async fn rename_symbol(app: App<'_>, pos: CellPosition, new_name: String) -> Result<()> {
    app.rename_symbol(pos, &new_name).await.map_err(err)
}

#[tauri::command]
async fn check_link(url: String) -> Result<String> {
    colab::clone::fetch(&url).await.map(|f| f.name).map_err(err)
}

#[tauri::command]
fn open_url(url: String) -> Result<()> {
    if !url.starts_with("https://") && !url.starts_with("http://") { return Err(format!("refusing to open {url}")) }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
async fn format_cell(app: App<'_>, path: String, cell_id: String) -> Result<()> {
    app.format(&path, Some(&cell_id)).await.map_err(err)
}

#[tauri::command]
async fn format_notebook(app: App<'_>, path: String) -> Result<()> {
    app.format(&path, None).await.map_err(err)
}

#[tauri::command]
async fn organize_imports(app: App<'_>, path: String, cell_id: Option<String>) -> Result<()> {
    app.organize_imports(&path, cell_id.as_deref()).await.map_err(err)
}

#[tauri::command]
async fn code_actions(app: App<'_>, pos: CellPosition) -> Result<Vec<crate::ipc::CodeActionItem>> {
    app.code_actions(pos).await.map_err(err)
}

#[tauri::command]
async fn apply_code_action(app: App<'_>, path: String, id: u32) -> Result<()> {
    app.apply_code_action(&path, id).await.map_err(err)
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
async fn read_file(app: App<'_>, path: String) -> Result<String> {
    app.read_file(&path).map_err(err)
}

#[tauri::command]
async fn send_input(app: App<'_>, path: String, value: String) -> Result<()> {
    app.send_input(&path, value).await.map_err(err)
}

#[tauri::command]
fn file_menu(window: tauri::Window, path: String) -> Result<()> {
    menu::file_menu(&window, &path).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .menu(menu::initial)
        .on_menu_event(menu::clicked)
        .setup(|app| {
            let handle = app.handle().clone();
            let emit: crate::engine::Emit = Arc::new(move |event, payload| drop(handle.emit(event, payload)));
            let core = Core::new(emit, app.path().app_data_dir()?);
            app.manage(core.clone());
            let server = core.clone();
            tauri::async_runtime::spawn(async move { core.install_tools().await });
            tauri::async_runtime::spawn(async move { crate::mcp::serve(server, crate::mcp::PORT).await.map_err(|e| eprintln!("mcp server: {e:#}")) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_notebook, rename_notebook, edit_cell, run_cell, run_all, interrupt, restart, select_kernel, kernel_state, send_comm,
            sign_in, sign_out, auth_state, colab_info, list_runtimes, detach, release, clone_link,
            completion, hover, definition, prepare_rename, rename_symbol, check_link, open_url, format_cell, format_notebook, organize_imports, code_actions, apply_code_action, read_file, send_input, list_monospace_fonts, list_fonts, file_menu
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
