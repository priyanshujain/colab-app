use colab_app_lib::engine::{Core, Emit};
use colab_app_lib::ipc::{CellEdit, CellPosition, KernelChoice};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Events = Arc<Mutex<Vec<(String, Value)>>>;

async fn wait_for(events: &Events, what: &str, pred: impl Fn(&str, &Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some((_, v)) = events.lock().unwrap().iter().find(|(n, v)| pred(n, v)) {
            return v.clone();
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_file(path: &std::path::Path, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        if pred(&v) {
            return v;
        }
        assert!(Instant::now() < deadline, "file never matched: {v:#}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn code(id: &str, src: &str) -> Value {
    json!({"cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": src})
}

#[tokio::test(flavor = "multi_thread")]
async fn local_notebook_runs_saves_merges_and_uses_tooling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("train.ipynb");
    let cells = [
        code("load", "import sys, time\nrows = [1, 2, 3]\nfor i in range(101):\n    sys.stderr.write(f'\\r{i:3d}%'); sys.stderr.flush()\n    time.sleep(0.005)\nprint('\\nloaded', len(rows))"),
        code("plot", "from IPython.display import display, update_display\ndisplay('step 0', display_id='status')\nupdate_display('step 9', display_id='status')\nsum(rows)"),
        code("bad", "raise ValueError('bad shape')"),
        code("after", "print('should not run')"),
    ];
    let nb = json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {"kernelspec": {"name": "python3"}}, "cells": cells});
    std::fs::write(&path, serde_json::to_string_pretty(&nb).unwrap()).unwrap();

    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let data = dirs::data_dir().unwrap().join("dev.colab-app");
    let core = Core::with_secrets(emit, data, "colab-app-tests");
    let opened = core.open_notebook(path.to_str().unwrap()).await.unwrap();
    let p = opened.path.clone();

    core.select_kernel(&p, KernelChoice::Local).await.unwrap();
    let selected = wait_for(&events, "selected status", |n, _| n == "kernel-status").await;
    assert_eq!((selected["status"].as_str(), selected["choice"]["kind"].as_str()), (Some("disconnected"), Some("local")));
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!events.lock().unwrap().iter().any(|(n, v)| n == "kernel-status" && v["status"] != "disconnected"), "selecting local started a kernel");
    core.run_all(&p).await.unwrap();
    assert!(events.lock().unwrap().iter().any(|(n, v)| n == "kernel-status" && v["status"] == "connecting" && v["choice"]["kind"] == "local"));

    let saved = wait_file(&path, |v| v["cells"][1]["execution_count"] == 2).await;
    let text = |cell: usize, out: usize| saved["cells"][cell]["outputs"][out]["text"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<String>();
    assert_eq!(text(0, 0), "100%");
    assert_eq!(text(0, 1), "\nloaded 3\n");
    assert_eq!(saved["cells"][1]["outputs"][0]["data"]["text/plain"], "'step 9'");
    assert_eq!(saved["cells"][1]["outputs"][1]["data"]["text/plain"], "6");
    assert_eq!(saved["cells"][2]["outputs"][0]["ename"], "ValueError");
    assert_eq!(saved["cells"][3]["execution_count"], Value::Null);
    let load_emits = events.lock().unwrap().iter().filter(|(n, v)| n == "cell-outputs" && v["cell_id"] == "load").count();
    assert!((2..40).contains(&load_emits), "{load_emits} cell-outputs events for ~0.6s of output");

    core.edit_cell(&p, CellEdit::Insert { cell_id: "ask".into(), index: 4, cell_type: nb::CellType::Code }).await.unwrap();
    core.edit_cell(&p, CellEdit::Source { cell_id: "ask".into(), source: "name = input('name? ')\nprint('hi', name)".into() }).await.unwrap();
    let run = tokio::spawn({
        let (core, p) = (core.clone(), p.clone());
        async move { core.run_cell(&p, "ask").await }
    });
    let req = wait_for(&events, "input request", |n, _| n == "input-request").await;
    assert_eq!((req["cell_id"].as_str(), req["prompt"].as_str()), (Some("ask"), Some("name? ")));
    core.send_input(&p, "ada".into()).await.unwrap();
    run.await.unwrap().unwrap();
    wait_file(&path, |v| v["cells"][4]["outputs"].to_string().contains("hi ada")).await;

    core.edit_cell(&p, CellEdit::Focus { cell_id: Some("plot".into()) }).await.unwrap();
    core.edit_cell(&p, CellEdit::Source { cell_id: "plot".into(), source: "total = sum( rows )".into() }).await.unwrap();
    let mut disk = wait_file(&path, |v| v["cells"][1]["source"] == json!(["total = sum( rows )"])).await;
    disk["cells"][2]["source"] = json!(["raise ValueError('edited outside')"]);
    disk["cells"][1]["source"] = json!(["stale"]);
    std::fs::write(&path, serde_json::to_string_pretty(&disk).unwrap()).unwrap();
    let changed = wait_for(&events, "notebook-changed", |n, _| n == "notebook-changed").await;
    assert_eq!(changed["notebook"]["cells"][2]["source"], "raise ValueError('edited outside')");
    assert_eq!(changed["notebook"]["cells"][1]["source"], "total = sum( rows )");

    let pos = |cell: &str, line, character| CellPosition { path: p.clone(), cell_id: cell.into(), line, character };
    core.edit_cell(&p, CellEdit::Source { cell_id: "after".into(), source: "ro".into() }).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let items = core.completion(pos("after", 0, 2)).await.unwrap();
        let items = items.get("items").unwrap_or(&items).as_array().cloned().unwrap_or_default();
        if items.iter().any(|i| i["label"] == "rows") {
            break;
        }
        assert!(Instant::now() < deadline, "no completion for rows");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    core.format(&p, Some("plot")).await.unwrap();
    wait_file(&path, |v| v["cells"][1]["source"] == json!(["total = sum(rows)"])).await;
    core.detach(&p).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn local_comm_messages_carry_target_name_and_binary_buffers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("widgets.ipynb");
    let target = "def echo(comm, msg):\n    comm.on_msg(lambda m: comm.send({'size': len(m['buffers'][0])}, buffers=[bytes(reversed(m['buffers'][0]))]))\n    comm.send({'opened': msg['content']['target_name']})\nget_ipython().kernel.comm_manager.register_target('echo', echo)";
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [code("t", target)]}).to_string()).unwrap();
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    core.run_cell(&p, "t").await.unwrap();

    let comm = |msg_type: &str, buffers: Vec<&str>| serde_json::from_value(json!({"path": p, "comm_id": "c1", "msg_type": msg_type, "target_name": "echo", "buffers": buffers, "data": {}, "metadata": {}})).unwrap();
    core.send_comm(comm("comm_open", vec![])).await.unwrap();
    let opened = wait_for(&events, "comm reply to open", |n, v| n == "comm" && v["data"]["opened"].is_string()).await;
    assert_eq!(opened["data"]["opened"], "echo");
    core.send_comm(comm("comm_msg", vec!["AAEC/w=="])).await.unwrap();
    let echoed = wait_for(&events, "echoed buffer", |n, v| n == "comm" && v["data"]["size"] == 4).await;
    assert_eq!((echoed["comm_id"].as_str(), echoed["msg_type"].as_str(), echoed["buffers"].clone()), (Some("c1"), Some("comm_msg"), json!(["/wIBAA=="])));
    core.detach(&p).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn local_huge_outputs_and_300_cells_stay_responsive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.ipynb");
    let mut cells: Vec<Value> = (0..300).map(|i| {
        let mut c = code(&format!("c{i}"), &format!("df_{i} = load({i})\ndf_{i}.describe()"));
        c["outputs"] = json!([{"output_type": "stream", "name": "stdout", "text": vec!["row\n"; 50]}]);
        c
    }).collect();
    cells[0] = code("flood", "for i in range(200_000):\n    print(f'line {i}')");
    cells[1] = code("bar", "import sys\nfor i in range(30_001):\n    sys.stderr.write(f'\\r{i}/30000 |' + '#' * (i // 1500)); sys.stderr.flush()");
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": cells}).to_string()).unwrap();

    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let started = Instant::now();
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    assert!(started.elapsed() < Duration::from_millis(500), "opening 300 cells took {:?}", started.elapsed());
    core.edit_cell(&p, CellEdit::Source { cell_id: "c299".into(), source: "df_299.head()".into() }).await.unwrap();
    wait_file(&path, |v| v["cells"][299]["source"] == json!(["df_299.head()"])).await;

    let run = tokio::spawn({
        let (core, p) = (core.clone(), p.clone());
        async move {
            let t = Instant::now();
            core.run_cell(&p, "flood").await.unwrap();
            eprintln!("flood {:?}", t.elapsed());
            core.run_cell(&p, "bar").await.unwrap();
            eprintln!("bar {:?}", t.elapsed());
        }
    });
    let mut worst = Duration::ZERO;
    while !run.is_finished() {
        let t = Instant::now();
        core.edit_cell(&p, CellEdit::Source { cell_id: "c5".into(), source: format!("x = {}", t.elapsed().as_nanos()) }).await.unwrap();
        worst = worst.max(t.elapsed());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    run.await.unwrap();
    let elapsed = started.elapsed();
    eprintln!("run took {elapsed:?}, worst edit {worst:?}");
    assert!(worst < Duration::from_millis(250), "edit took {worst:?} during output flood");
    let emits = events.lock().unwrap().iter().filter(|(n, v)| n == "cell-outputs" && (v["cell_id"] == "flood" || v["cell_id"] == "bar")).count();
    assert!(emits as f64 <= 32.0 * elapsed.as_secs_f64() + 4.0, "{emits} emits in {elapsed:?}");

    let saved = wait_file(&path, |v| v["cells"][1]["outputs"].to_string().contains("30000/30000")).await;
    let lines = saved["cells"][0]["outputs"][0]["text"].as_array().unwrap();
    assert_eq!((lines.len(), lines[199_999].as_str()), (200_000, Some("line 199999\n")));
    assert!(saved["cells"][1]["outputs"].to_string().len() < 2_000);
    assert!(std::fs::metadata(&path).unwrap().len() < 6_000_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn rename_keeps_kernel_and_autosave_reports_save_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Untitled.ipynb");
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [code("a", "lr = 3e-4"), code("b", "print(lr * 2)")]}).to_string()).unwrap();
    std::fs::write(dir.path().join("taken.ipynb"), "{}").unwrap();
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    core.run_cell(&p, "a").await.unwrap();

    let states = || events.lock().unwrap().iter().filter(|(n, v)| n == "save-state" && v["path"] == p).map(|(_, v)| v["dirty"].as_bool().unwrap()).collect::<Vec<_>>();
    wait_for(&events, "clean after autosave", |n, v| n == "save-state" && v["dirty"] == false).await;
    assert_eq!(states(), [true, false]);

    let err = core.rename_notebook(&p, "taken").await.unwrap_err().to_string();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(std::fs::read_to_string(dir.path().join("taken.ipynb")).unwrap(), "{}");

    let renamed = core.rename_notebook(&p, "finetune").await.unwrap();
    assert!(renamed.ends_with("/finetune.ipynb") && !path.exists());
    assert!(core.run_cell(&p, "b").await.is_err());
    core.run_cell(&renamed, "b").await.unwrap();
    let saved = wait_file(std::path::Path::new(&renamed), |v| v["cells"][1]["outputs"].to_string().contains("0.0006")).await;
    assert_eq!(saved["cells"][1]["execution_count"], 2);
    core.detach(&renamed).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn format_organizes_imports_and_quick_fix_rewrites_alias_import() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"arena\"\nversion = \"0.1.0\"\nrequires-python = \">=3.13\"\n").unwrap();
    let path = dir.path().join("cartpole.ipynb");
    let imports = "import gymnasium as gym\nimport numpy as np\nimport torch\nimport torch.nn as nn\nimport matplotlib.pyplot as plt\n\ntorch.manual_seed(0)";
    let cells = [code("imports", imports), code("policy", "policy = nn.Linear(4, 2)\nplt.plot(np.zeros(3))\nenv = gym.make(\"CartPole-v1\")")];
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": cells}).to_string()).unwrap();
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    let codes = |v: &Value| v["diagnostics"].as_array().unwrap().iter().map(|d| d["code"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>();
    let latest = |events: &Events| events.lock().unwrap().iter().rev().find(|(n, v)| n == "diagnostics" && v["cell_id"] == "imports").map(|(_, v)| codes(v)).unwrap_or_default();
    let settle = async |events: &Events, pred: &dyn Fn(&[String]) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !pred(&latest(events)) {
            assert!(Instant::now() < deadline, "diagnostics never matched: {:?}", latest(events));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    settle(&events, &|c| c.contains(&"I001".into()) && c.contains(&"PLR0402".into())).await;

    core.format(&p, None).await.unwrap();
    let sorted = "import gymnasium as gym\nimport matplotlib.pyplot as plt\nimport numpy as np\nimport torch\nimport torch.nn as nn\n\ntorch.manual_seed(0)";
    wait_file(&path, |v| v["cells"][0]["source"].as_array().map(|l| l.iter().map(|l| l.as_str().unwrap()).collect::<String>()) == Some(sorted.into())).await;
    settle(&events, &|c| !c.contains(&"I001".into()) && c.contains(&"PLR0402".into())).await;

    let actions = core.code_actions(CellPosition { path: p.clone(), cell_id: "imports".into(), line: 4, character: 8 }).await.unwrap();
    let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
    assert!(titles.contains(&"Ruff (PLR0402): Disable for this line"), "{titles:?}");
    let fix = actions.iter().find(|a| a.title == "Ruff (PLR0402): Replace with `from torch import nn`").unwrap_or_else(|| panic!("{titles:?}"));
    let shifted = format!("import os\n{sorted}");
    core.edit_cell(&p, CellEdit::Source { cell_id: "imports".into(), source: shifted.clone() }).await.unwrap();
    assert!(core.apply_code_action(&p, fix.id).await.is_err());
    let source = |v: &Value| v["cells"][0]["source"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<String>();
    wait_file(&path, |v| source(v) == shifted).await;

    let actions = core.code_actions(CellPosition { path: p.clone(), cell_id: "imports".into(), line: 5, character: 8 }).await.unwrap();
    let fix = actions.iter().find(|a| a.title == "Ruff (PLR0402): Replace with `from torch import nn`").unwrap();
    core.apply_code_action(&p, fix.id).await.unwrap();
    wait_file(&path, |v| source(v) == shifted.replace("import torch.nn as nn", "from torch import nn")).await;
    settle(&events, &|c| !c.contains(&"PLR0402".into())).await;
    core.detach(&p).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn local_run_saves_jupyterlab_timing_metadata_for_ok_and_failed_cells() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("timed.ipynb");
    let cells = [code("slow", "import time\ntime.sleep(0.3)"), code("bad", "1 / 0")];
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": cells}).to_string()).unwrap();
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    core.run_cell(&p, "slow").await.unwrap();
    core.run_cell(&p, "bad").await.unwrap();
    let saved = wait_file(&path, |v| v["cells"][1]["metadata"]["execution"]["shell.execute_reply"].is_string()).await;
    let at = |cell: usize, key: &str| chrono::DateTime::parse_from_rfc3339(saved["cells"][cell]["metadata"]["execution"][key].as_str().unwrap()).unwrap();
    for cell in 0..2 {
        let [busy, input, reply, idle] = ["iopub.status.busy", "iopub.execute_input", "shell.execute_reply", "iopub.status.idle"].map(|k| at(cell, k));
        assert!(busy <= input && input <= idle && input <= reply, "{busy} {input} {reply} {idle}");
    }
    assert!(at(0, "iopub.status.idle") - at(0, "iopub.status.busy") >= chrono::Duration::milliseconds(300));
    assert_eq!(saved["cells"][0]["outputs"], json!([]));
    assert_eq!(saved["cells"][1]["outputs"][0]["ename"], "ZeroDivisionError");
    let last = events.lock().unwrap().iter().rev().find(|(n, v)| n == "cell-outputs" && v["cell_id"] == "bad").unwrap().1.clone();
    assert_eq!((&last["running"], &last["execution"]), (&json!(false), &saved["cells"][1]["metadata"]["execution"]));
    core.detach(&p).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn rename_symbol_updates_every_cell_and_rejects_blank_positions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"arena\"\nversion = \"0.1.0\"\nrequires-python = \">=3.13\"\n").unwrap();
    let path = dir.path().join("sweep.ipynb");
    let md = json!({"cell_type": "markdown", "id": "notes", "metadata": {}, "source": "lr is the learning rate"});
    let cells = [code("setup", "lr = 3e-4\nsteps = 100"), md, code("train", "for step in range(steps):\n    print(step * lr)")];
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": cells}).to_string()).unwrap();
    let events = Events::default();
    let sink = events.clone();
    let emit: Emit = Arc::new(move |name, payload| sink.lock().unwrap().push((name.into(), payload)));
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), "colab-app-tests");
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;
    wait_for(&events, "diagnostics", |n, _| n == "diagnostics").await;
    let pos = CellPosition { path: p.clone(), cell_id: "train".into(), line: 1, character: 18 };
    let prepared = core.prepare_rename(pos.clone()).await.unwrap();
    let range = prepared.get("range").unwrap_or(&prepared);
    assert_eq!(range["start"], json!({"line": 1, "character": 17}), "{prepared}");

    core.edit_cell(&p, CellEdit::Source { cell_id: "setup".into(), source: "lr = 1e-3\nsteps = 100".into() }).await.unwrap();
    core.rename_symbol(pos.clone(), "learning_rate").await.unwrap();
    let source = |v: &Value, i: usize| v["cells"][i]["source"].as_array().map_or(v["cells"][i]["source"].as_str().unwrap_or_default().into(), |l| l.iter().map(|l| l.as_str().unwrap()).collect::<String>());
    let v = wait_file(&path, |v| source(v, 2).contains("learning_rate")).await;
    assert_eq!(source(&v, 0), "learning_rate = 1e-3\nsteps = 100");
    assert_eq!(source(&v, 1), "lr is the learning rate");
    assert_eq!(source(&v, 2), "for step in range(steps):\n    print(step * learning_rate)");
    assert!(core.rename_symbol(CellPosition { line: 0, character: 30, ..pos }, "x").await.is_err());
    core.detach(&p).await.unwrap();
}
