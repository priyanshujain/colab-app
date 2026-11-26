#![cfg(target_os = "macos")]

use colab_server::engine::{Core, Emit};
use colab_server::ipc::KernelChoice;
use colab_server::kernels::KernelHost;
use colab_server::protocol::Rpc;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn core(data: &Path) -> Arc<Core> {
    std::fs::create_dir_all(data).unwrap();
    let tools = colab_server::data_dir().join("tools");
    if tools.exists() {
        std::os::unix::fs::symlink(tools, data.join("tools")).unwrap();
    }
    let emit: Emit = Arc::new(|_, _| {});
    Core::with_secrets(emit, data.into(), "colab-app-tests")
}

fn awake(name: &str) -> bool {
    let out = String::from_utf8(std::process::Command::new("pmset").args(["-g", "assertions"]).output().unwrap().stdout).unwrap();
    let me = format!("pid {}(", std::process::id());
    out.lines().any(|l| l.contains(&me) && l.contains(&format!("PreventUserIdleSystemSleep named: \"Colab Notebook kernel running for {name}\"")))
}

async fn until(what: &str, secs: u64, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_busy_hosted_or_local_kernel_keeps_the_mac_awake_until_it_goes_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("b/home/experiment")).unwrap();
    let b = core(&dir.join("b/data"));
    let kernels = KernelHost::new(b.clone(), dir.join("b/home"));
    let rpc = Rpc { core: b, events: tokio::sync::broadcast::channel(16).0, shutdown: Default::default(), tailscale: None };
    let app = colab_server::tailscale::router(rpc, kernels);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let a = core(&dir.join("a/data"));
    colab_server::hosts::add(&dir.join("a/data/hosts.json"), "b", &addr.to_string()).unwrap();
    std::fs::create_dir_all(dir.join("a/experiment")).unwrap();
    let file = dir.join("a/experiment/train.ipynb");
    let cell = json!({"cell_type": "code", "id": "train", "metadata": {}, "execution_count": null, "outputs": [], "source": "import time\ntime.sleep(4)"});
    std::fs::write(&file, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [cell]}).to_string()).unwrap();
    let path = a.open_notebook(&file.to_string_lossy()).await.unwrap().path;
    for (choice, name) in [(KernelChoice::Remote { host: "b".into() }, "another machine"), (KernelChoice::Local, "train.ipynb")] {
        a.select_kernel(&path, choice).await.unwrap();
        assert_eq!(a.kernel_state(&path).await.unwrap().status, kernel::KernelStatus::Idle);
        assert!(!awake(name), "an idle kernel holds a sleep assertion");
        let (core, p) = (a.clone(), path.clone());
        let run = tokio::spawn(async move { core.agent_run(&p, "train").await });
        until("the sleep assertion", 3, || awake(name)).await;
        run.await.unwrap().unwrap();
        until("the sleep assertion to go", 2, || !awake(name)).await;
    }
}
