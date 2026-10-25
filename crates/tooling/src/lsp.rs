use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, oneshot};

type Pending = Arc<std::sync::Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>;

pub struct Server {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    next: AtomicI64,
    _child: Child,
}

impl Server {
    pub async fn start(
        program: &Path,
        root: &Path,
        on_notify: impl Fn(&str, Value) + Send + 'static,
    ) -> Result<Self> {
        let mut child = Command::new(program)
            .arg("server")
            .current_dir(root)
            .env("VIRTUAL_ENV", root.join(".venv"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
        let pending = Pending::default();
        tokio::spawn(read_loop(child.stdout.take().unwrap(), stdin.clone(), pending.clone(), on_notify));
        let server = Self { stdin, pending, next: AtomicI64::new(1), _child: child };
        let uri = url::Url::from_directory_path(root).map_err(|_| anyhow!("bad root"))?.to_string();
        server
            .request(
                "initialize",
                json!({
                    "processId": std::process::id(),
                    "rootUri": uri,
                    "workspaceFolders": [{"uri": uri, "name": "root"}],
                    "capabilities": {
                        "textDocument": {
                            "completion": {"completionItem": {"documentationFormat": ["markdown", "plaintext"]}},
                            "hover": {"contentFormat": ["markdown", "plaintext"]},
                            "definition": {"linkSupport": false},
                            "publishDiagnostics": {},
                            "formatting": {},
                            "rename": {"prepareSupport": true},
                            "codeAction": {
                                "codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "source", "source.organizeImports", "source.fixAll", "notebook"]}},
                                "resolveSupport": {"properties": ["edit"]}
                            }
                        },
                        "notebookDocument": {"synchronization": {}},
                        "workspace": {"configuration": true, "workspaceFolders": true, "workspaceEdit": {"documentChanges": true}}
                    }
                }),
            )
            .await?;
        server.notify("initialized", json!({})).await?;
        Ok(server)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        write(&self.stdin, json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).await?;
        rx.await.map_err(|_| anyhow!("{method}: server exited"))?
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        write(&self.stdin, json!({"jsonrpc": "2.0", "method": method, "params": params})).await
    }

    pub async fn shutdown(&self) {
        let _ = self.request("shutdown", Value::Null).await;
        let _ = self.notify("exit", Value::Null).await;
    }
}

async fn write(stdin: &Mutex<ChildStdin>, msg: Value) -> Result<()> {
    let body = msg.to_string();
    let mut stdin = stdin.lock().await;
    stdin.write_all(format!("Content-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await?;
    Ok(stdin.flush().await?)
}

async fn read_loop(
    stdout: ChildStdout,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    on_notify: impl Fn(&str, Value),
) {
    let mut reader = BufReader::new(stdout);
    while let Ok(msg) = read_message(&mut reader).await {
        let method = msg["method"].as_str();
        match (msg.get("id").cloned(), method) {
            (Some(id), Some(method)) => {
                let result = match method {
                    "workspace/configuration" => {
                        Value::Array(vec![Value::Null; msg["params"]["items"].as_array().map_or(0, Vec::len)])
                    }
                    _ => Value::Null,
                };
                let _ = write(&stdin, json!({"jsonrpc": "2.0", "id": id, "result": result})).await;
            }
            (Some(id), None) => {
                if let Some(tx) = id.as_i64().and_then(|id| pending.lock().unwrap().remove(&id)) {
                    let _ = tx.send(match msg.get("error") {
                        Some(e) => Err(anyhow!("lsp error: {e}")),
                        None => Ok(msg["result"].clone()),
                    });
                }
            }
            (None, Some(method)) => on_notify(method, msg["params"].clone()),
            _ => {}
        }
    }
    pending.lock().unwrap().clear();
}

async fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<Value> {
    let mut len = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            return Err(anyhow!("eof"));
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse()?;
        }
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}
