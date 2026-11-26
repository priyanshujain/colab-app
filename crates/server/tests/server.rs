use colab_server::Config;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::net::UnixStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

struct Client {
    ws: WebSocketStream<UnixStream>,
    events: Vec<(String, Value)>,
    next: u64,
}

impl Client {
    async fn attach(socket: &Path) -> Self {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match colab_server::connect(socket).await {
                Ok(ws) => return Self { ws, events: vec![], next: 1 },
                Err(e) => assert!(Instant::now() < deadline, "server never answered on {}: {e:#}", socket.display()),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.ws.send(Message::Text(json!({"id": id, "method": method, "params": params}).to_string().into())).await.unwrap();
        id
    }

    async fn frame(&mut self, deadline: Instant, what: &str) -> Value {
        let text = tokio::time::timeout_at(deadline.into(), self.ws.next()).await.unwrap_or_else(|_| panic!("timed out waiting for {what}"));
        let Some(Ok(Message::Text(text))) = text else { panic!("connection closed while waiting for {what}") };
        let frame: Value = serde_json::from_str(&text).unwrap();
        if let Some(event) = frame["event"].as_str() {
            self.events.push((event.into(), frame["payload"].clone()));
        }
        frame
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params).await;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let frame = self.frame(deadline, method).await;
            if frame["id"] == id {
                assert!(frame.get("error").is_none(), "{method} failed: {}", frame["error"]);
                return frame["result"].clone();
            }
        }
    }

    async fn event(&mut self, what: &str, pred: impl Fn(&str, &Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut seen = 0;
        loop {
            if let Some((_, v)) = self.events[seen..].iter().find(|(n, v)| pred(n, v)) {
                return v.clone();
            }
            seen = self.events.len();
            self.frame(deadline, what).await;
        }
    }
}

struct Server {
    dir: tempfile::TempDir,
    socket: PathBuf,
}

async fn start() -> Server {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let tools = colab_server::data_dir().join("tools");
    if tools.exists() {
        std::os::unix::fs::symlink(tools, data.join("tools")).unwrap();
    }
    let socket = dir.path().join("run/server.sock");
    let config = Config { socket: socket.clone(), data_dir: data, mcp_port: 0, secrets_service: format!("colab-app-test-{}", uuid::Uuid::new_v4()), tailscale: false, tailscale_hostname: None };
    tokio::spawn(async move { colab_server::run(config).await.unwrap() });
    Client::attach(&socket).await;
    Server { dir, socket }
}

fn notebook(dir: &Path, name: &str, cells: &[(&str, &str)]) -> String {
    let cells: Vec<Value> = cells.iter().map(|(id, src)| json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src})).collect();
    let path = dir.join(name);
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {"kernelspec": {"name": "python3"}}, "cells": cells}).to_string()).unwrap();
    path.to_string_lossy().into()
}

fn stream_text(outputs: &Value) -> String {
    outputs.as_array().into_iter().flatten().filter(|o| o["output_type"] == "stream").map(|o| match &o["text"] {
        Value::Array(lines) => lines.iter().filter_map(Value::as_str).collect(),
        text => text.as_str().unwrap_or_default().to_string(),
    }).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cell_keeps_running_and_saving_after_its_client_leaves_and_a_new_client_sees_it() {
    let server = start().await;
    assert_eq!(std::fs::metadata(&server.socket).unwrap().permissions().mode() & 0o777, 0o600);
    let file = notebook(server.dir.path(), "train.ipynb", &[
        ("setup", "import sys\nprint('python', sys.version_info[0])"),
        ("train", "import time\nfor step in range(10):\n    print(f'step {step} loss {1 / (step + 1):.3f}', flush=True)\n    time.sleep(0.5)"),
    ]);

    let mut first = Client::attach(&server.socket).await;
    let path = first.call("open_notebook", json!({"path": file})).await["path"].as_str().unwrap().to_string();
    first.call("select_kernel", json!({"path": path, "choice": {"kind": "local"}})).await;
    first.call("run_cell", json!({"path": path, "cell_id": "setup"})).await;
    let setup = first.event("setup outputs", |n, v| n == "cell-outputs" && v["cell_id"] == "setup" && v["running"] == false).await;
    assert_eq!((stream_text(&setup["outputs"]).as_str(), setup["execution_count"].as_u64()), ("python 3\n", Some(1)));
    assert_eq!(first.call("kernel_state", json!({"path": path})).await["status"], "idle");

    first.send("run_cell", json!({"path": path, "cell_id": "train"})).await;
    let started = first.event("first training line", |n, v| n == "cell-outputs" && v["cell_id"] == "train" && stream_text(&v["outputs"]).contains("step 0")).await;
    assert_eq!(started["running"], true);
    first.ws.close(None).await.unwrap();
    drop(first);

    let mut second = Client::attach(&server.socket).await;
    assert_eq!(second.call("open_notebooks", json!({})).await, json!([path]));
    let expected: String = (0..10).map(|step| format!("step {step} loss {:.3}\n", 1.0 / (step as f64 + 1.0))).collect();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let train = &saved["cells"][1];
        if train["execution_count"] == 2 && stream_text(&train["outputs"]) == expected {
            break;
        }
        assert!(Instant::now() < deadline, "saved notebook never held every training line: {train:#}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    while second.call("kernel_state", json!({"path": path})).await["status"] != "idle" {
        assert!(Instant::now() < deadline, "the kernel never went idle after the cell finished");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn typed_edits_apply_in_order_reach_the_other_client_and_a_format_reaches_both() {
    let server = start().await;
    let file = notebook(server.dir.path(), "eval.ipynb", &[("load", "rows = [3, 1, 2]")]);
    let mut editor = Client::attach(&server.socket).await;
    let mut watcher = Client::attach(&server.socket).await;
    let path = editor.call("open_notebook", json!({"path": file})).await["path"].as_str().unwrap().to_string();
    assert_eq!(watcher.call("open_notebook", json!({"path": file})).await["notebook"]["cells"][0]["source"], "rows = [3, 1, 2]");

    let typed = "rows.sort( key = lambda r : -r )\nrows";
    editor.send("edit_cell", json!({"path": path, "edit": {"kind": "insert", "cell_id": "sort", "index": 1, "cell_type": "code"}})).await;
    for end in 1..=typed.len() {
        editor.send("edit_cell", json!({"path": path, "edit": {"kind": "source", "cell_id": "sort", "source": &typed[..end]}})).await;
    }
    editor.call("format_cell", json!({"path": path, "cell_id": "sort"})).await;
    let formatted = "rows.sort(key=lambda r: -r)\nrows";
    let changed = watcher.event("typed notebook", |n, v| n == "notebook-changed" && v["notebook"]["cells"][1]["source"] == typed).await;
    assert_eq!(changed["path"], path);
    assert_eq!(changed["notebook"]["cells"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect::<Vec<_>>(), ["load", "sort"]);
    watcher.event("formatted notebook", |n, v| n == "notebook-changed" && v["notebook"]["cells"][1]["source"] == formatted).await;

    let own = editor.event("format result", |n, _| n == "notebook-changed").await;
    assert_eq!(own["notebook"]["cells"][1]["source"], formatted);
    assert_eq!(editor.events.iter().filter(|(n, _)| n == "notebook-changed").count(), 1, "the editing client got its own keystrokes back: {:?}", editor.events);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_browses_the_server_disk_and_starts_a_sign_in_the_server_finishes() {
    let server = start().await;
    let root = server.dir.path().join("projects");
    std::fs::create_dir_all(root.join("mnist/.venv")).unwrap();
    std::fs::create_dir_all(root.join("Archive")).unwrap();
    notebook(&root, "train.ipynb", &[]);
    std::fs::write(root.join("notes.txt"), "x").unwrap();
    let mut client = Client::attach(&server.socket).await;
    assert_eq!(client.call("version", json!({})).await, env!("CARGO_PKG_VERSION"));

    let listing = client.call("list_dir", json!({"path": root.join("mnist/..").to_string_lossy()})).await;
    assert_eq!(listing["path"], std::fs::canonicalize(&root).unwrap().to_string_lossy().as_ref());
    assert_eq!(listing["entries"], json!([{"name": "Archive", "dir": true}, {"name": "mnist", "dir": true}, {"name": "train.ipynb", "dir": false}]));
    let home = client.call("list_dir", json!({"path": ""})).await;
    assert_eq!(home["path"], dirs::home_dir().unwrap().to_string_lossy().as_ref());

    let id = client.send("complete_sign_in", json!({"code": "4/abc", "redirect_uri": "http://127.0.0.1:5000/"})).await;
    let reply = loop {
        let frame = client.frame(Instant::now() + Duration::from_secs(10), "complete_sign_in").await;
        if frame["id"] == id { break frame }
    };
    assert_eq!(reply["error"], "no sign in is in progress");
    let url = client.call("begin_sign_in", json!({"client_id": null, "client_secret": null, "redirect_uri": "http://127.0.0.1:5000/"})).await;
    let url = url::Url::parse(url.as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.host_str(), Some("accounts.google.com"));
    assert_eq!(query["redirect_uri"], "http://127.0.0.1:5000/");
    assert_eq!(query["code_challenge_method"], "S256");
}
