use crate::install::Tools;
use crate::lsp::Server;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{Mutex, broadcast};

#[derive(Clone, Debug, PartialEq)]
pub struct CellDiagnostics {
    pub uri: String,
    pub diagnostics: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct CodeAction {
    pub server: usize,
    pub action: Value,
}

pub fn text_edits(edit: &Value) -> HashMap<String, Vec<Value>> {
    let mut out: HashMap<String, Vec<Value>> = HashMap::new();
    for (uri, edits) in edit["changes"].as_object().into_iter().flatten() {
        out.entry(uri.clone()).or_default().extend(edits.as_array().cloned().unwrap_or_default());
    }
    for change in edit["documentChanges"].as_array().into_iter().flatten() {
        let uri = change["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
        out.entry(uri).or_default().extend(change["edits"].as_array().cloned().unwrap_or_default());
    }
    out
}

type Diagnostics = Arc<StdMutex<HashMap<String, [Vec<Value>; 2]>>>;

struct OpenNotebook {
    version: i64,
    cells: Vec<(String, String)>,
}

pub struct Project {
    ty: Server,
    ruff: Server,
    diagnostics: Diagnostics,
    events: broadcast::Sender<CellDiagnostics>,
    notebooks: Mutex<HashMap<PathBuf, OpenNotebook>>,
}

pub fn cell_uri(notebook: &Path, cell_id: &str) -> String {
    let file = url::Url::from_file_path(notebook).map(String::from).unwrap_or_default();
    format!("vscode-notebook-cell:{}#{cell_id}", &file["file:".len()..])
}

fn notebook_uri(notebook: &Path) -> String {
    url::Url::from_file_path(notebook).map(String::from).unwrap_or_default()
}

fn on_diagnostics(slot: usize, store: Diagnostics, events: broadcast::Sender<CellDiagnostics>) -> impl Fn(&str, Value) {
    move |method, params| {
        if method != "textDocument/publishDiagnostics" {
            return;
        }
        let uri = params["uri"].as_str().unwrap_or_default().to_string();
        let list = params["diagnostics"].as_array().cloned().unwrap_or_default();
        let merged = {
            let mut store = store.lock().unwrap();
            let entry = store.entry(uri.clone()).or_default();
            entry[slot] = list;
            entry.concat()
        };
        let _ = events.send(CellDiagnostics { uri, diagnostics: merged });
    }
}

impl Project {
    pub async fn start(tools: &Tools, root: &Path) -> Result<Self> {
        let diagnostics = Diagnostics::default();
        let (events, _) = broadcast::channel(256);
        let ty = Server::start(&tools.ty, root, on_diagnostics(0, diagnostics.clone(), events.clone())).await?;
        let ruff = Server::start(&tools.ruff, root, on_diagnostics(1, diagnostics.clone(), events.clone())).await?;
        Ok(Self { ty, ruff, diagnostics, events, notebooks: Mutex::default() })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CellDiagnostics> {
        self.events.subscribe()
    }

    pub fn diagnostics(&self, cell_uri: &str) -> Vec<Value> {
        self.diagnostics.lock().unwrap().get(cell_uri).map(|d| d.concat()).unwrap_or_default()
    }

    async fn both(&self, method: &str, params: Value) -> Result<()> {
        self.ty.notify(method, params.clone()).await?;
        self.ruff.notify(method, params).await
    }

    pub async fn sync(&self, notebook: &Path, code_cells: &[(String, String)]) -> Result<()> {
        let mut notebooks = self.notebooks.lock().await;
        let uri = notebook_uri(notebook);
        if let Some(open) = notebooks.get_mut(notebook) {
            let same_ids = open.cells.len() == code_cells.len()
                && open.cells.iter().zip(code_cells).all(|(a, b)| a.0 == b.0);
            if same_ids {
                open.version += 1;
                let changes: Vec<Value> = open
                    .cells
                    .iter()
                    .zip(code_cells)
                    .filter(|(a, b)| a.1 != b.1)
                    .map(|(_, (id, text))| {
                        json!({"document": {"uri": cell_uri(notebook, id), "version": open.version}, "changes": [{"text": text}]})
                    })
                    .collect();
                open.cells = code_cells.to_vec();
                if changes.is_empty() {
                    return Ok(());
                }
                let params = json!({
                    "notebookDocument": {"uri": uri, "version": open.version},
                    "change": {"cells": {"textContent": changes}}
                });
                return self.both("notebookDocument/didChange", params).await;
            }
            let old = notebooks.remove(notebook).unwrap();
            self.send_close(notebook, &old.cells).await?;
        }
        let params = json!({
            "notebookDocument": {
                "uri": uri,
                "notebookType": "jupyter-notebook",
                "version": 1,
                "cells": code_cells.iter().map(|(id, _)| json!({"kind": 2, "document": cell_uri(notebook, id)})).collect::<Vec<_>>()
            },
            "cellTextDocuments": code_cells.iter().map(|(id, text)| json!({
                "uri": cell_uri(notebook, id), "languageId": "python", "version": 1, "text": text
            })).collect::<Vec<_>>()
        });
        self.both("notebookDocument/didOpen", params).await?;
        notebooks.insert(notebook.to_path_buf(), OpenNotebook { version: 1, cells: code_cells.to_vec() });
        Ok(())
    }

    pub async fn close(&self, notebook: &Path) -> Result<()> {
        match self.notebooks.lock().await.remove(notebook) {
            Some(open) => self.send_close(notebook, &open.cells).await,
            None => Ok(()),
        }
    }

    async fn send_close(&self, notebook: &Path, cells: &[(String, String)]) -> Result<()> {
        let uris: Vec<String> = cells.iter().map(|(id, _)| cell_uri(notebook, id)).collect();
        let params = json!({
            "notebookDocument": {"uri": notebook_uri(notebook)},
            "cellTextDocuments": uris.iter().map(|u| json!({"uri": u})).collect::<Vec<_>>()
        });
        self.both("notebookDocument/didClose", params).await?;
        let mut store = self.diagnostics.lock().unwrap();
        uris.iter().for_each(|u| drop(store.remove(u)));
        Ok(())
    }

    fn position(cell_uri: &str, line: u32, character: u32) -> Value {
        json!({"textDocument": {"uri": cell_uri}, "position": {"line": line, "character": character}})
    }

    pub async fn completion(&self, cell_uri: &str, line: u32, character: u32) -> Result<Value> {
        self.ty.request("textDocument/completion", Self::position(cell_uri, line, character)).await
    }

    pub async fn hover(&self, cell_uri: &str, line: u32, character: u32) -> Result<Value> {
        self.ty.request("textDocument/hover", Self::position(cell_uri, line, character)).await
    }

    pub async fn definition(&self, cell_uri: &str, line: u32, character: u32) -> Result<Value> {
        self.ty.request("textDocument/definition", Self::position(cell_uri, line, character)).await
    }

    pub async fn prepare_rename(&self, cell_uri: &str, line: u32, character: u32) -> Result<Value> {
        self.ty.request("textDocument/prepareRename", Self::position(cell_uri, line, character)).await
    }

    pub async fn rename(&self, cell_uri: &str, line: u32, character: u32, new_name: &str) -> Result<Value> {
        let mut params = Self::position(cell_uri, line, character);
        params["newName"] = new_name.into();
        self.ty.request("textDocument/rename", params).await
    }

    pub async fn format(&self, cell_uri: &str) -> Result<Value> {
        let params = json!({"textDocument": {"uri": cell_uri}, "options": {"tabSize": 4, "insertSpaces": true}});
        self.ruff.request("textDocument/formatting", params).await
    }

    pub async fn code_actions(&self, cell_uri: &str, range: Value, only: Option<&[&str]>) -> Result<Vec<CodeAction>> {
        let lines = |r: &Value| (r["start"]["line"].as_u64(), r["end"]["line"].as_u64());
        let (start, end) = lines(&range);
        let diagnostics: Vec<Value> = self.diagnostics(cell_uri).into_iter().filter(|d| lines(&d["range"]).0 <= end && lines(&d["range"]).1 >= start).collect();
        let mut context = json!({"diagnostics": diagnostics});
        if let Some(only) = only {
            context["only"] = json!(only);
        }
        let params = json!({"textDocument": {"uri": cell_uri}, "range": range, "context": context});
        let mut actions = vec![];
        for (server, s) in [&self.ty, &self.ruff].into_iter().enumerate() {
            let list = s.request("textDocument/codeAction", params.clone()).await.unwrap_or_default();
            actions.extend(list.as_array().into_iter().flatten().filter(|a| a.get("command").is_none_or(|c| !c.is_string())).map(|a| CodeAction { server, action: a.clone() }));
        }
        Ok(actions)
    }

    pub async fn resolve(&self, action: &CodeAction) -> Result<Value> {
        if let Some(edit) = action.action.get("edit") {
            return Ok(edit.clone());
        }
        let server = if action.server == 0 { &self.ty } else { &self.ruff };
        Ok(server.request("codeAction/resolve", action.action.clone()).await?["edit"].clone())
    }

    pub async fn organize_imports(&self, cell_uri: &str) -> Result<Value> {
        let range = json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}});
        let actions = self.code_actions(cell_uri, range, Some(&["notebook.source.organizeImports"])).await?;
        match actions.iter().find(|a| a.server == 1) {
            Some(a) => self.resolve(a).await,
            None => Ok(Value::Null),
        }
    }

    pub async fn shutdown(&self) {
        self.ty.shutdown().await;
        self.ruff.shutdown().await;
    }
}

pub struct Tooling {
    tools: Tools,
    projects: Mutex<HashMap<PathBuf, Arc<Project>>>,
}

impl Tooling {
    pub async fn new(support_dir: &Path) -> Result<Self> {
        Ok(Self { tools: crate::install(support_dir).await?, projects: Mutex::default() })
    }

    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    pub async fn project(&self, root: &Path) -> Result<Arc<Project>> {
        let root = root.canonicalize().map_err(|e| anyhow!("{}: {e}", root.display()))?;
        let mut projects = self.projects.lock().await;
        if let Some(p) = projects.get(&root) {
            return Ok(p.clone());
        }
        let project = Arc::new(Project::start(&self.tools, &root).await?);
        projects.insert(root, project.clone());
        Ok(project)
    }

    pub async fn stop(&self, root: &Path) {
        let removed = match root.canonicalize() {
            Ok(root) => self.projects.lock().await.remove(&root),
            Err(_) => None,
        };
        if let Some(p) = removed {
            p.shutdown().await;
        }
    }
}
