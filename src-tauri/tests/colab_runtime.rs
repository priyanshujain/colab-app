use colab_app_lib::engine::{Core, Emit};
use colab_app_lib::ipc::{KernelChoice, Variant};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Events = Arc<Mutex<Vec<(String, Value)>>>;

fn core(events: &Events) -> Arc<Core> {
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    Core::new(emit, dirs::data_dir().unwrap().join("dev.colab-app"))
}

async fn idle(events: &Events) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while !events.lock().unwrap().iter().any(|(n, v)| n == "kernel-status" && v["status"] == "idle") {
        assert!(Instant::now() < deadline, "kernel never went idle");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "assigns a real free CPU Colab runtime"]
async fn colab_cpu_runtime_detach_reattach_release() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("remote.ipynb");
    let cell = |id: &str, src: &str| json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src});
    let nb = json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [cell("set", "import os\nsecret = 1234\nprint(os.path.isdir('/content'))"), cell("get", "secret + 1")]});
    std::fs::write(&path, nb.to_string()).unwrap();

    let events = Events::default();
    let app = core(&events);
    let p = app.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    let before = app.list_runtimes().await.unwrap();
    app.select_kernel(&p, KernelChoice::Colab { variant: Variant::Default, accelerator: None, high_ram: false }).await.unwrap();
    let endpoint = app.list_runtimes().await.unwrap().into_iter().find(|r| !before.iter().any(|b| b.endpoint == r.endpoint)).unwrap().endpoint;
    idle(&events).await;
    app.run_cell(&p, "set").await.unwrap();
    app.detach(&p).await.unwrap();
    drop(app);

    let events = Events::default();
    let app = core(&events);
    app.open_notebook(&p).await.unwrap();
    idle(&events).await;
    app.run_cell(&p, "get").await.unwrap();
    app.release(&p).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["cells"][0]["outputs"][0]["text"], json!(["True\n"]));
    assert_eq!(saved["cells"][1]["outputs"][0]["data"]["text/plain"], "1235");
    assert!(!app.list_runtimes().await.unwrap().iter().any(|r| r.endpoint == endpoint));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "assigns a real free CPU Colab runtime for several minutes"]
async fn colab_long_loop_survives_drops_and_external_edits_with_widget_buffers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("train.ipynb");
    let cell = |id: &str, src: &str| json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src});
    let train = "import time\nfrom tqdm import tqdm\nfor i in tqdm(range(150)):\n    print(f'epoch {i} loss {1 / (i + 1):.4f}', flush=True)\n    time.sleep(1)";
    let target = "def echo(comm, msg):\n    comm.on_msg(lambda m: comm.send({'size': len(m['buffers'][0])}, buffers=[bytes(reversed(m['buffers'][0]))]))\nget_ipython().kernel.comm_manager.register_target('echo', echo)";
    let nb = json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [cell("train", train), cell("notes", "# notes"), cell("echo", target)]});
    std::fs::write(&path, nb.to_string()).unwrap();

    let events = Events::default();
    let app = core(&events);
    let tokens = colab::auth::import_cli_token(&colab::auth::cli_token_path()).unwrap().expect("~/.config/colab-cli/token.json");
    app.set_auth(colab::Auth::new(tokens, false)).await;
    let p = app.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    let before = app.list_runtimes().await.unwrap();
    app.select_kernel(&p, KernelChoice::Colab { variant: Variant::Default, accelerator: None, high_ram: false }).await.unwrap();
    let endpoint = app.list_runtimes().await.unwrap().into_iter().find(|r| !before.iter().any(|b| b.endpoint == r.endpoint)).unwrap().endpoint;
    idle(&events).await;

    let run = tokio::spawn({
        let (app, p) = (app.clone(), p.clone());
        async move { app.run_cell(&p, "train").await }
    });
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_secs(35)).await;
        app.force_reconnect(&p).await.unwrap();
    }
    let mut disk: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    disk["cells"][1]["source"] = json!(["# notes edited by another process"]);
    std::fs::write(&path, serde_json::to_string_pretty(&disk).unwrap()).unwrap();
    run.await.unwrap().unwrap();
    app.run_cell(&p, "echo").await.unwrap();
    let comm = |msg_type: &str, buffers: Vec<&str>| serde_json::from_value(json!({"path": p, "comm_id": "c1", "msg_type": msg_type, "target_name": "echo", "buffers": buffers, "data": {}, "metadata": {}})).unwrap();
    app.send_comm(comm("comm_open", vec![])).await.unwrap();
    app.send_comm(comm("comm_msg", vec!["AAEC/w=="])).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let echoed = loop {
        if let Some((_, v)) = events.lock().unwrap().iter().find(|(n, v)| n == "comm" && v["data"]["size"] == 4) {
            break v.clone();
        }
        assert!(Instant::now() < deadline, "no comm echo");
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    app.release(&p).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after = app.list_runtimes().await.unwrap();
    eprintln!("assignments after release: {}", after.len());
    assert!(!after.iter().any(|r| r.endpoint == endpoint) && after.len() == before.len());
    assert_eq!(echoed["buffers"], json!(["/wIBAA=="]));

    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["cells"][1]["source"], json!(["# notes edited by another process"]));
    let outputs = saved["cells"][0]["outputs"].as_array().unwrap();
    let stream = |name: &str| outputs.iter().filter(|o| o["name"] == name).flat_map(|o| o["text"].as_array().unwrap()).map(|l| l.as_str().unwrap()).collect::<String>();
    let expected: String = (0..150).map(|i| format!("epoch {i} loss {:.4}\n", 1.0 / (i as f64 + 1.0))).collect();
    assert_eq!(stream("stdout"), expected);
    let bar = outputs.iter().rev().find(|o| o["name"] == "stderr").unwrap()["text"].to_string();
    assert!(bar.contains("150/150") && !bar.contains("149/150"), "{bar}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "assigns a real free CPU Colab runtime"]
async fn connecting_after_detach_reuses_the_detached_runtime_instead_of_assigning_another() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("detached.ipynb");
    let cell = |id: &str, src: &str| json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src});
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [cell("set", "secret = 4321"), cell("get", "secret")]}).to_string()).unwrap();
    let cpu = KernelChoice::Colab { variant: Variant::Default, accelerator: None, high_ram: false };

    let events = Events::default();
    let app = core(&events);
    let p = app.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    let before = app.list_runtimes().await.unwrap();
    app.agent_connect(&p, cpu.clone()).await.unwrap();
    idle(&events).await;
    let ours = app.list_runtimes().await.unwrap().into_iter().filter(|r| !before.iter().any(|b| b.endpoint == r.endpoint)).collect::<Vec<_>>();
    assert_eq!(ours.len(), 1);
    app.run_cell(&p, "set").await.unwrap();
    app.detach(&p).await.unwrap();

    events.lock().unwrap().clear();
    let started = Instant::now();
    assert!(app.agent_connect(&p, cpu).await.unwrap());
    idle(&events).await;
    let connected_in = started.elapsed();
    let after = app.list_runtimes().await.unwrap();
    assert_eq!(after.len(), before.len() + 1, "a second runtime was assigned");
    assert!(after.iter().any(|r| r.endpoint == ours[0].endpoint));
    app.run_cell(&p, "get").await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let outputs = serde_json::to_value(&app.notebook(&p).unwrap().cells[1].outputs).unwrap();
    app.release(&p).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(app.list_runtimes().await.unwrap().len(), before.len());
    assert_eq!(outputs[0]["data"]["text/plain"], "4321", "variables were lost, so this is not the detached runtime");
    eprintln!("reconnected in {connected_in:?}");
}
