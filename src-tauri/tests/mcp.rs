use colab_app_lib::engine::{Core, Emit};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ClientConfig};
use rmcp::transport::{StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Events = Arc<Mutex<Vec<(String, Value)>>>;

#[tokio::test(flavor = "multi_thread")]
async fn claude_drives_an_open_notebook_over_mcp_and_the_app_follows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("analysis.ipynb");
    let cell = json!({"cell_type": "code", "id": "setup", "metadata": {}, "execution_count": null, "outputs": [], "source": "data = [3, 1, 2]"});
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {"kernelspec": {"name": "python3"}}, "cells": [cell]}).to_string()).unwrap();

    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::new(emit, dirs::data_dir().unwrap().join("dev.colab-app"));

    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    tokio::spawn(colab_app_lib::mcp::serve(core.clone(), port));
    let url = format!("http://127.0.0.1:{port}/mcp");
    let deadline = Instant::now() + Duration::from_secs(5);
    while tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        assert!(Instant::now() < deadline, "mcp server never listened");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let client = ClientConfig::default().serve(StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(url.clone()))).await.unwrap();
    let call = async |name: &'static str, args: Value| {
        let r = client.call_tool(CallToolRequestParams::new(name).with_arguments(args.as_object().cloned().unwrap_or_default())).await.unwrap();
        let r = serde_json::to_value(r).unwrap();
        (r["content"][0]["text"].as_str().unwrap_or_default().to_string(), r["isError"] == true)
    };

    assert_eq!(call("list_notebooks", json!({})).await, (String::new(), false));
    let (p, err) = call("open_notebook", json!({"path": path})).await;
    assert!(!err && p.ends_with("analysis.ipynb"), "{p}");
    assert!(events.lock().unwrap().iter().any(|(n, v)| n == "agent-open" && v["path"] == p.as_str()), "app window was not asked to show the notebook");
    assert_eq!(call("list_notebooks", json!({})).await, (p.clone(), false));
    assert_eq!(call("connect_kernel", json!({"path": p, "runtime": "local"})).await, ("connected (idle)".into(), false));
    assert_eq!(call("connect_kernel", json!({"path": p, "runtime": "local"})).await, ("already connected (idle)".into(), false));
    let (status, _) = call("kernel_status", json!({"path": p})).await;
    assert_eq!(serde_json::from_str::<Value>(&status).unwrap(), json!({"status": "idle", "kernel": {"kind": "local"}}));
    let (id, err) = call("insert_cell", json!({"path": p, "index": 99, "source": "data.sort()\nprint('sorted', data)\ndata[-1] * 10"})).await;
    assert!(!err && !id.is_empty());
    let focus = events.lock().unwrap().iter().any(|(n, v)| n == "agent-focus" && v["cell_id"] == id.as_str() && v["path"] == p.as_str());
    let shown = events.lock().unwrap().iter().any(|(n, v)| n == "notebook-changed" && v["notebook"]["cells"][1]["source"].as_str().is_some_and(|s| s.contains("data.sort()")));
    assert!(focus && shown, "app was not told to show the inserted cell");

    assert_eq!(call("run_cell", json!({"path": p, "cell_id": "setup"})).await, (String::new(), false));
    assert_eq!(call("run_cell", json!({"path": p, "cell_id": id})).await, ("sorted [1, 2, 3]\n30".into(), false));
    let streamed = events.lock().unwrap().iter().any(|(n, v)| n == "cell-outputs" && v["cell_id"] == id.as_str() && v["running"] == false && v["execution_count"] == 2);
    assert!(streamed, "outputs were not streamed to the app");

    assert_eq!(call("edit_cell", json!({"path": p, "cell_id": id, "source": "import time\nfor i in range(3): print(i)\n1 / 0"})).await.1, false);
    let (out, err) = call("run_cell", json!({"path": p, "cell_id": id})).await;
    assert!(!err && out.starts_with("0\n1\n2\nZeroDivisionError: division by zero\n") && !out.contains('\x1b'), "{out}");

    let (text, _) = call("read_notebook", json!({"path": p})).await;
    assert!(text.starts_with("### cell 0 id=setup code [1]\ndata = [3, 1, 2]\n\n### cell 1 id="), "{text}");
    assert!(text.contains(&format!("id={id} code [3]\nimport time")) && text.contains("--- outputs\n0\n1\n2\nZeroDivisionError"), "{text}");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&path).unwrap().contains("ZeroDivisionError") {
        assert!(Instant::now() < deadline, "run was not saved to disk");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(call("delete_cell", json!({"path": p, "cell_id": id})).await.1, false);
    assert_eq!(core.notebook(&p).unwrap().cells.len(), 1);
    let (msg, err) = call("read_notebook", json!({"path": dir.path().join("other.ipynb")})).await;
    assert!(err && msg.contains("not open"), "{msg}");
    let fresh = dir.path().join("new.ipynb");
    let (q, err) = call("open_notebook", json!({"path": fresh})).await;
    assert!(!err && fresh.exists(), "{q}");
    assert_eq!(call("insert_cell", json!({"path": q, "index": 0, "source": "# Notes", "markdown": true})).await.1, false);
    let deadline = Instant::now() + Duration::from_secs(10);
    while nb::Notebook::parse(&std::fs::read_to_string(&fresh).unwrap()).unwrap().cells.first().is_none_or(|c| c.source != "# Notes") {
        assert!(Instant::now() < deadline, "created notebook never saved the inserted cell");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (msg, err) = call("open_notebook", json!({"path": "relative.ipynb"})).await;
    assert!(err && msg.contains("absolute .ipynb"), "{msg}");

    let browser = reqwest::Client::new().post(&url).header("Origin", "https://evil.example").header("Content-Type", "application/json").header("Accept", "application/json, text/event-stream")
        .body(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string()).send().await.unwrap();
    assert_eq!(browser.status(), 403);
    assert_eq!(call("release_kernel", json!({"path": p})).await, ("released".into(), false));
    let (status, _) = call("kernel_status", json!({"path": p})).await;
    assert_eq!(serde_json::from_str::<Value>(&status).unwrap(), json!({"status": "disconnected", "kernel": null}));
    let (out, _) = call("run_cell", json!({"path": p, "cell_id": "setup"})).await;
    assert_eq!(out, "");
    assert_eq!(call("release_kernel", json!({"path": p})).await, ("released".into(), false));
}
