use colab_app_lib::engine::{Core, Emit};
use nb::Output;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

async fn run(core: &Arc<Core>, path: &str, id: &str, source: &str) -> String {
    core.edit_cell(path, colab_app_lib::ipc::CellEdit::Source { cell_id: id.into(), source: source.into() }).await.unwrap();
    let cell = core.agent_run(path, id).await.unwrap();
    cell.outputs.unwrap().iter().map(|o| match o {
        Output::Stream { text, .. } => text.clone(),
        Output::Error { ename, evalue, .. } => format!("{ename}: {evalue}\n"),
        Output::DisplayData { .. } | Output::ExecuteResult { .. } => String::new(),
    }).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_secrets_reach_local_kernels_through_colab_userdata_and_env() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s3.ipynb");
    let cell = json!({"cell_type": "code", "id": "c", "metadata": {}, "execution_count": null, "outputs": [], "source": ""});
    std::fs::write(&path, json!({"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": [cell]}).to_string()).unwrap();
    let emit: Emit = Arc::new(|_, _| {});
    let service = format!("colab-app-test-{}", uuid::Uuid::new_v4());
    let core = Core::with_secrets(emit, dirs::data_dir().unwrap().join("dev.colab-app"), &service);
    let key = "wJalr/K7MDENG \"quoted\" 'single'\nline two \u{e9}";
    core.set_secret("AWS_SECRET_ACCESS_KEY", key).await.unwrap();
    core.set_secret("AWS_REGION", "ap-south-1").await.unwrap();
    assert!(core.set_secret("BAD NAME", "x").await.is_err());
    let p = core.open_notebook(path.to_str().unwrap()).await.unwrap().path;

    let read = "import os\nfrom google.colab import userdata\nv = userdata.get('AWS_SECRET_ACCESS_KEY')\nprint(v == os.environ['AWS_SECRET_ACCESS_KEY'], repr(v), userdata.get('AWS_REGION'))";
    assert_eq!(run(&core, &p, "c", read).await, format!("True {} ap-south-1\n", py_repr(key)));
    let missing = "try:\n    userdata.get('HF_TOKEN')\nexcept userdata.SecretNotFoundError as e:\n    print('missing:', e)";
    assert_eq!(run(&core, &p, "c", missing).await, "missing: Secret HF_TOKEN does not exist.\n");

    core.set_secret("AWS_REGION", "us-east-1").await.unwrap();
    core.delete_secret("AWS_SECRET_ACCESS_KEY").await.unwrap();
    let after = "print(os.environ['AWS_REGION'], userdata.get('AWS_REGION'), 'AWS_SECRET_ACCESS_KEY' in os.environ)";
    assert_eq!(run(&core, &p, "c", after).await, "us-east-1 us-east-1 False\n");

    core.set_env(&p, BTreeMap::from([("S3_BUCKET".into(), "team-data".into()), ("RUN".into(), "a=b c".into())])).await.unwrap();
    assert_eq!(run(&core, &p, "c", "print(os.environ['S3_BUCKET'], os.environ['RUN'])").await, "team-data a=b c\n");
    assert!(core.set_env(&p, BTreeMap::from([("A=B".into(), "x".into())])).await.is_err());

    core.restart(&p).await.unwrap();
    let restarted = "import os\nprint(os.environ.get('AWS_REGION'), os.environ.get('S3_BUCKET'))";
    assert_eq!(run(&core, &p, "c", restarted).await, "us-east-1 None\n");
    let count = run(&core, &p, "c", "print('ran')").await;
    assert_eq!((count.as_str(), core.notebook(&p).unwrap().cells[0].execution_count.flatten()), ("ran\n", Some(2)));

    core.delete_secret("AWS_REGION").await.unwrap();
    assert!(core.list_secrets().await.unwrap().is_empty());
    keyring::Entry::new(&service, "secrets").unwrap().delete_credential().unwrap();
}

fn py_repr(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n"))
}
