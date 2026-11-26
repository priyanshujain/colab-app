use colab_server::engine::{Core, Emit};
use colab_server::ipc::KernelChoice;
use colab_server::kernels::KernelHost;
use colab_server::protocol::Rpc;
use nb::Output;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

type Events = Arc<Mutex<Vec<(String, Value)>>>;

struct Proxy {
    addr: SocketAddr,
    targets: Arc<Mutex<Vec<String>>>,
    conns: Arc<Mutex<Vec<JoinHandle<()>>>>,
    refuse_until: Arc<Mutex<Instant>>,
    frozen: tokio::sync::watch::Sender<u32>,
    listener: JoinHandle<()>,
}

async fn socks5(client: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    client.read_exact(&mut vec![0; head[1] as usize]).await?;
    client.write_all(&[5, 2]).await?;
    client.read_exact(&mut head).await?;
    let mut user = vec![0; head[1] as usize];
    client.read_exact(&mut user).await?;
    let mut password = vec![0; client.read_u8().await? as usize];
    client.read_exact(&mut password).await?;
    let ok = user == b"tsnet" && password == b"secret";
    client.write_all(&[1, if ok { 0 } else { 1 }]).await?;
    let mut request = [0u8; 4];
    client.read_exact(&mut request).await?;
    let mut name = vec![0; if request[3] == 3 { client.read_u8().await? as usize } else { 4 }];
    client.read_exact(&mut name).await?;
    let port = client.read_u16().await?;
    client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
    Ok(ok.then(|| format!("{}:{port}", String::from_utf8_lossy(&name))))
}

impl Proxy {
    async fn start(target: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (targets, conns, refuse_until) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Instant::now())));
        let frozen = tokio::sync::watch::channel(0).0;
        let (t, c, r, f) = (targets.clone(), conns.clone(), refuse_until.clone(), frozen.clone());
        let listener = tokio::spawn(async move {
            loop {
                let (mut client, _) = listener.accept().await.unwrap();
                if Instant::now() < *r.lock().unwrap() {
                    continue;
                }
                let (t, mut frozen) = (t.clone(), f.subscribe());
                c.lock().unwrap().push(tokio::spawn(async move {
                    let Ok(Some(requested)) = socks5(&mut client).await else { return };
                    t.lock().unwrap().push(requested);
                    let mut server = TcpStream::connect(target).await.unwrap();
                    let froze = tokio::select! {
                        _ = tokio::io::copy_bidirectional(&mut client, &mut server) => false,
                        _ = frozen.changed() => true,
                    };
                    if froze {
                        std::future::pending::<()>().await;
                    }
                }));
            }
        });
        Self { addr, targets, conns, refuse_until, frozen, listener }
    }

    fn socks(&self) -> Option<colab::Socks> {
        Some(colab::Socks { addr: self.addr.to_string(), user: "tsnet".into(), password: "secret".into() })
    }

    fn freeze(&self) {
        self.frozen.send_modify(|n| *n += 1);
    }

    fn cut(&self, refuse: Duration) {
        *self.refuse_until.lock().unwrap() = Instant::now() + refuse;
        self.conns.lock().unwrap().drain(..).for_each(|c| c.abort());
    }

    fn kill(&self) {
        self.listener.abort();
        self.cut(Duration::from_secs(3600));
    }
}

fn data_dir(root: &Path) -> PathBuf {
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let tools = colab_server::data_dir().join("tools");
    if tools.exists() {
        std::os::unix::fs::symlink(tools, data.join("tools")).unwrap();
    }
    data
}

async fn kernel_host(root: &Path) -> SocketAddr {
    let emit: Emit = Arc::new(|_, _| {});
    let core = Core::with_secrets(emit, data_dir(root), "colab-app-tests");
    let kernels = KernelHost::new(core.clone(), root.join("home"));
    let rpc = Rpc { core, events: tokio::sync::broadcast::channel(16).0, shutdown: Default::default(), tailscale: None };
    let app = colab_server::tailscale::router(rpc, kernels);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn notebook(dir: &Path, cells: &[(&str, &str)]) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let cells: Vec<Value> = cells.iter().map(|(id, src)| json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src})).collect();
    let path = dir.join("experiment.ipynb");
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {"kernelspec": {"name": "python3"}}, "cells": cells}).to_string()).unwrap();
    path.to_string_lossy().into()
}

fn text(cell: &nb::Cell) -> String {
    cell.outputs.iter().flatten().map(|o| match o {
        Output::Stream { text, .. } => text.clone(),
        Output::ExecuteResult { data, .. } => data.get("text/plain").and_then(Value::as_str).unwrap_or_default().to_string(),
        other => format!("{other:?}"),
    }).collect()
}

async fn hosted(b: SocketAddr) -> Vec<String> {
    let kernels: Vec<Value> = reqwest::get(format!("http://{b}/api/kernels")).await.unwrap().json().await.unwrap();
    kernels.iter().map(|k| k["id"].as_str().unwrap().to_string()).collect()
}

fn client(dir: &Path, proxy: &Proxy, service: &str) -> (Arc<Core>, Events) {
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let data = data_dir(dir);
    colab_server::hosts::add(&data.join("hosts.json"), "b", "b-colab.example.ts.net").unwrap();
    let a = Core::with_secrets(emit, data, service);
    a.socks.send_replace(proxy.socks());
    (a, events)
}

async fn wait_for(events: &Events, what: &str, pred: impl Fn(&Value) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !events.lock().unwrap().iter().any(|(n, v)| n == "kernel-status" && pred(v)) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn notebook_on_a_runs_on_b_through_the_tailscale_proxy_keeps_its_kernel_across_drops_and_detaches_and_releases_it() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    let b = kernel_host(&dir.join("b")).await;
    std::fs::create_dir_all(dir.join("b/home/experiment")).unwrap();
    let proxy = Proxy::start(b).await;
    let service = format!("colab-app-test-{}", uuid::Uuid::new_v4());
    let (a, events) = client(&dir.join("a"), &proxy, &service);
    a.set_secret("HF_TOKEN", "hf_live_7f3a").await.unwrap();
    let remote = KernelChoice::Remote { host: "b".into() };

    let scratch = notebook(&dir.join("a/scratch"), &[("where", "import os, sys\nfrom google.colab import userdata\nprint(os.getcwd(), sys.prefix, userdata.get('HF_TOKEN'))")]);
    let scratch = a.open_notebook(&scratch).await.unwrap().path;
    a.select_kernel(&scratch, remote.clone()).await.unwrap();
    let work = dir.join("b/data/kernels/scratch");
    assert_eq!(text(&a.agent_run(&scratch, "where").await.unwrap()), format!("{} {} hf_live_7f3a\n", work.display(), work.join(".venv").display()));

    let file = notebook(&dir.join("a/experiment"), &[
        ("where", "import os, sys\nstate = 41\nprint(os.getcwd(), sys.prefix)"),
        ("train", "import time\nfor step in range(8):\n    print(f'step {step}', flush=True)\n    time.sleep(0.5)\nstate += 1"),
        ("after", "state"),
    ]);
    let path = a.open_notebook(&file).await.unwrap().path;
    a.select_kernel(&path, remote.clone()).await.unwrap();
    let home = dir.join("b/home/experiment");
    assert_eq!(text(&a.agent_run(&path, "where").await.unwrap()), format!("{} {}\n", home.display(), home.join(".venv").display()));
    let targets = proxy.targets.lock().unwrap().clone();
    assert!(targets.len() >= 2 && targets.iter().all(|t| t == "b-colab.example.ts.net:80"), "{targets:?}");

    let (core, p) = (a.clone(), path.clone());
    let run = tokio::spawn(async move { core.agent_run(&p, "train").await });
    let deadline = Instant::now() + Duration::from_secs(30);
    while !a.notebook(&path).unwrap().cell("train").is_some_and(|c| text(c).contains("step 1")) {
        assert!(Instant::now() < deadline, "train never printed step 1");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    events.lock().unwrap().clear();
    proxy.cut(Duration::from_secs(3));
    wait_for(&events, "reconnecting", |v| v["status"] == "reconnecting").await;
    let train = run.await.unwrap().unwrap();
    assert_eq!(text(&train), (0..8).map(|i| format!("step {i}\n")).collect::<String>());
    assert_eq!(text(&a.agent_run(&path, "after").await.unwrap()), "42");
    assert_eq!(hosted(b).await.len(), 2);

    a.detach(&path).await.unwrap();
    a.select_kernel(&path, remote.clone()).await.unwrap();
    assert_eq!(text(&a.agent_run(&path, "after").await.unwrap()), "42");
    assert_eq!(hosted(b).await.len(), 2);

    a.agent_close(&path).await.unwrap();
    let path = a.open_notebook(&file).await.unwrap().path;
    wait_for(&events, "reattached", |v| v["path"] == path.as_str() && v["status"] == "idle" && v["choice"]["host"] == "b").await;
    assert_eq!(text(&a.agent_run(&path, "after").await.unwrap()), "42");

    a.detach(&path).await.unwrap();
    a.release(&path).await.unwrap();
    assert_eq!(hosted(b).await.len(), 1);
    assert!(text(&a.agent_run(&scratch, "where").await.unwrap()).ends_with(" hf_live_7f3a\n"));

    a.select_kernel(&path, remote.clone()).await.unwrap();
    assert_eq!(text(&a.agent_run(&path, "where").await.unwrap()), format!("{} {}\n", home.display(), home.join(".venv").display()));
    proxy.kill();
    wait_for(&events, "reconnecting after kill", |v| v["path"] == path.as_str() && v["status"] == "reconnecting").await;
    let started = Instant::now();
    let error = a.select_kernel(&path, remote).await.unwrap_err().to_string();
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    assert_eq!(error, "host b is unavailable");

    a.agent_close(&path).await.unwrap();
    a.open_notebook(&file).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    a.agent_close(&path).await.unwrap();
    let back = Proxy::start(b).await;
    a.socks.send_replace(back.socks());
    events.lock().unwrap().clear();
    let path = a.open_notebook(&file).await.unwrap().path;
    wait_for(&events, "reattached after b came back", |v| v["path"] == path.as_str() && v["status"] == "idle" && v["choice"]["host"] == "b").await;
    assert_eq!(text(&a.agent_run(&path, "after").await.unwrap()), "41");
    assert_eq!(hosted(b).await.len(), 2);

    a.delete_secret("HF_TOKEN").await.unwrap();
    colab::keystore::Entry::new(&service, "secrets", "secrets.json").delete().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn output_written_into_a_silently_dropped_connection_arrives_once_and_in_order_after_the_client_reconnects() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    let b = kernel_host(&dir.join("b")).await;
    let proxy = Proxy::start(b).await;
    let (a, events) = client(&dir.join("a"), &proxy, "colab-app-tests");
    let file = notebook(&dir.join("a/experiment"), &[
        ("train", "import time\nfor step in range(12):\n    print(f'step {step}', flush=True)\n    time.sleep(0.25)\nprint('done')"),
        ("after", "step"),
    ]);
    let path = a.open_notebook(&file).await.unwrap().path;
    a.select_kernel(&path, KernelChoice::Remote { host: "b".into() }).await.unwrap();

    let (core, p) = (a.clone(), path.clone());
    let run = tokio::spawn(async move { core.agent_run(&p, "train").await });
    let deadline = Instant::now() + Duration::from_secs(30);
    while !a.notebook(&path).unwrap().cell("train").is_some_and(|c| text(c).contains("step 1")) {
        assert!(Instant::now() < deadline, "train never printed step 1");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    events.lock().unwrap().clear();
    proxy.freeze();
    wait_for(&events, "reconnecting after the silent drop", |v| v["status"] == "reconnecting").await;
    let train = run.await.unwrap().unwrap();
    assert_eq!(text(&train), (0..12).map(|i| format!("step {i}\n")).chain(["done\n".into()]).collect::<String>());
    assert_eq!(text(&a.agent_run(&path, "after").await.unwrap()), "11");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tailscale_listener_serves_no_mcp_and_refuses_tailscale_settings_and_the_proxy() {
    use futures::{SinkExt, StreamExt};
    let tmp = tempfile::tempdir().unwrap();
    let b = kernel_host(tmp.path()).await;
    assert_eq!(reqwest::get(format!("http://{b}/mcp")).await.unwrap().status(), 404);
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{b}/rpc")).await.unwrap();
    for (id, method) in ["version", "tailscale_status", "tailscale_enable", "tailscale_disable", "tailscale_logout", "tailscale_proxy"].into_iter().enumerate() {
        ws.send(json!({"id": id, "method": method}).to_string().into()).await.unwrap();
        let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(reply))) = ws.next().await else { panic!("no reply to {method}") };
        let reply: Value = serde_json::from_str(&reply).unwrap();
        let expected = if id == 0 { json!({"id": 0, "result": env!("CARGO_PKG_VERSION")}) } else { json!({"id": id, "error": format!("{method} is not available over Tailscale")}) };
        assert_eq!(reply, expected);
    }
}
