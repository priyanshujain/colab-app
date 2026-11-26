use crate::windows::LOCAL;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::menu::{Menu, MenuEvent, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;

const RECENT_LIMIT: usize = 10;

fn recent_file<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("recent.json")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recent {
    pub host: String,
    pub path: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Path(String),
    Entry(Recent),
}

pub fn load_recent(file: &Path) -> Vec<Recent> {
    let stored: Vec<Stored> = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    stored.into_iter().map(|s| match s { Stored::Path(path) => Recent { host: LOCAL.into(), path }, Stored::Entry(r) => r }).collect()
}

pub fn push_recent(file: &Path, host: &str, path: &str, replaces: Option<&str>) -> anyhow::Result<Vec<Recent>> {
    let mut recent = load_recent(file);
    recent.retain(|r| r.host != host || (r.path != path && Some(r.path.as_str()) != replaces));
    recent.insert(0, Recent { host: host.into(), path: path.into() });
    recent.truncate(RECENT_LIMIT);
    std::fs::create_dir_all(file.parent().unwrap())?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&recent)?)?;
    std::fs::rename(tmp, file)?;
    Ok(recent)
}

fn build<R: Runtime>(app: &AppHandle<R>, recent: &[Recent]) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str, key: Option<&str>| {
        let mut b = MenuItemBuilder::with_id(id, text);
        if let Some(k) = key {
            b = b.accelerator(k);
        }
        b.build(app)
    };
    let mut open_recent = SubmenuBuilder::new(app, "Open Recent");
    for Recent { host, path } in recent {
        let name = Path::new(path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into());
        open_recent = if host == LOCAL { open_recent.text(format!("open-recent:{path}"), name) } else { open_recent.text(format!("at:{host}\topen-recent:{path}"), format!("{name} [{host}]")) };
    }
    let open_recent = open_recent.enabled(!recent.is_empty()).build()?;
    let hosts = colab_server::hosts::load(&colab_server::hosts::file()).unwrap_or_default();
    let mut connect = SubmenuBuilder::new(app, "Connect to Host");
    for host in &hosts {
        connect = connect.text(format!("connect:{}", host.label), &host.label);
    }
    let connect = connect.enabled(!hosts.is_empty()).build()?;
    let name = app.package_info().name.clone();
    let app_menu = SubmenuBuilder::new(app, &name).about(None).separator().item(&item("settings", "Settings...", Some("CmdOrCtrl+,"))?)
        .separator().services().separator().hide().hide_others().show_all().separator().quit().build()?;
    let file = SubmenuBuilder::new(app, "File").item(&item("new-window", "New Window", Some("CmdOrCtrl+Shift+N"))?).item(&connect).separator().item(&item("open", "Open...", Some("CmdOrCtrl+O"))?).item(&open_recent)
        .item(&item("clone", "Clone from Colab...", None)?).separator().close_window().build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    let view = SubmenuBuilder::new(app, "View").fullscreen().build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().separator().bring_all_to_front().build()?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window])
}

pub fn initial<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    build(app, &load_recent(&recent_file(app)))
}

pub fn file_menu<R: Runtime>(window: &tauri::Window<R>, path: &str) -> tauri::Result<()> {
    let menu = Menu::with_items(window, &[
        &MenuItemBuilder::with_id(format!("copy-path:{path}"), "Copy Path").build(window)?,
        &MenuItemBuilder::with_id(format!("reveal:{path}"), "Reveal in Finder").build(window)?,
    ])?;
    window.popup_menu(&menu)
}

pub fn clicked(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    if let Some(path) = id.strip_prefix("copy-path:") {
        let _ = app.clipboard().write_text(path).map_err(|e| log::error!("copy path: {e}"));
    } else if let Some(path) = id.strip_prefix("reveal:") {
        let _ = tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| log::error!("reveal: {e}"));
    }
    if id == "new-window" {
        let _ = crate::windows::create(app, LOCAL, "").map_err(|e| log::error!("new window: {e}"));
    } else if let Some(host) = id.strip_prefix("connect:") {
        let _ = crate::windows::create(app, host, "open").map_err(|e| log::error!("connect to {host}: {e}"));
    } else if let Some((host, action)) = id.strip_prefix("at:").and_then(|r| r.split_once('\t')) {
        crate::windows::menu(app, host, action);
    } else if ["open", "clone", "settings"].contains(&id) || id.starts_with("open-recent:") {
        crate::windows::menu(app, LOCAL, id);
    }
}

pub fn opened<R: Runtime>(app: &AppHandle<R>, host: &str, path: &str, replaces: Option<&str>) -> anyhow::Result<()> {
    let recent = push_recent(&recent_file(app), host, path, replaces)?;
    app.set_menu(build(app, &recent)?)?;
    Ok(())
}

pub fn refresh<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    app.set_menu(initial(app)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(host: &str, path: &str) -> Recent {
        Recent { host: host.into(), path: path.into() }
    }

    #[test]
    fn recent_list_moves_reopened_to_front_caps_length_and_keeps_hosts_apart() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("support/recent.json");
        assert!(load_recent(&file).is_empty());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, r#"["/nb/old.ipynb"]"#).unwrap();
        assert_eq!(load_recent(&file), [at(LOCAL, "/nb/old.ipynb")]);
        for i in 0..12 {
            push_recent(&file, LOCAL, &format!("/nb/{i}.ipynb"), None).unwrap();
        }
        let recent = push_recent(&file, LOCAL, "/nb/5.ipynb", None).unwrap();
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!(recent[..3], [at(LOCAL, "/nb/5.ipynb"), at(LOCAL, "/nb/11.ipynb"), at(LOCAL, "/nb/10.ipynb")]);
        assert_eq!(load_recent(&file), recent);
        let renamed = push_recent(&file, LOCAL, "/nb/train.ipynb", Some("/nb/11.ipynb")).unwrap();
        assert_eq!(renamed[..3], [at(LOCAL, "/nb/train.ipynb"), at(LOCAL, "/nb/5.ipynb"), at(LOCAL, "/nb/10.ipynb")]);
        let remote = push_recent(&file, "anton", "/nb/train.ipynb", None).unwrap();
        assert_eq!(remote[..2], [at("anton", "/nb/train.ipynb"), at(LOCAL, "/nb/train.ipynb")]);
    }
}
