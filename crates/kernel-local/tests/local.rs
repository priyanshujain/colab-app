use futures::StreamExt;
use kernel::{Content, Kernel, KernelStatus, Message};
use kernel_local::LocalKernel;
use std::time::Duration;

async fn run(k: &LocalKernel, code: &str) -> Vec<Content> {
    k.execute(code).await.unwrap().map(|m: Message| m.typed()).collect().await
}

fn stdout(out: &[Content]) -> String {
    out.iter().filter_map(|c| match c { Content::Stream { name, text } if name == "stdout" => Some(text.as_str()), _ => None }).collect()
}

#[tokio::test]
async fn local_kernel_lifecycle_in_uv_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"demo\"\nversion = \"0.1.0\"\nrequires-python = \">=3.10\"\n").unwrap();
    std::fs::create_dir(dir.path().join("notebooks")).unwrap();
    let k = LocalKernel::start(&dir.path().join("notebooks/a.ipynb"), None).await.unwrap();
    assert!(dir.path().join(".venv/bin/python").exists());
    assert_eq!(k.status(), KernelStatus::Idle);
    assert!(std::fs::read_to_string(dir.path().join("pyproject.toml")).unwrap().contains("ipykernel"));

    let out = run(&k, "import os\nx = 41\nprint(os.getcwd())\nx + 1").await;
    assert_eq!(stdout(&out).trim(), std::fs::canonicalize(dir.path()).unwrap().to_str().unwrap());
    assert!(out.iter().any(|c| matches!(c, Content::ExecuteResult { data, .. } if data["text/plain"] == "42")));

    let mut exec = k.execute("import time\nfor i in range(100):\n    print(i, flush=True)\n    time.sleep(0.1)").await.unwrap();
    while !matches!(exec.next().await.unwrap().typed(), Content::Stream { text, .. } if text.contains('2')) {}
    k.interrupt().await.unwrap();
    let rest: Vec<Content> = tokio::time::timeout(Duration::from_secs(5), exec.map(|m| m.typed()).collect()).await.unwrap();
    assert!(rest.iter().any(|c| matches!(c, Content::Error { ename, .. } if ename == "KeyboardInterrupt")));
    assert_eq!(stdout(&run(&k, "print(x)").await).trim(), "41");

    let mut exec = k.execute("for i in range(30):\n    print(i, flush=True)\n    time.sleep(0.05)").await.unwrap();
    exec.next().await.unwrap();
    drop(exec);
    let mut unclaimed = k.subscribe();
    let late = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Content::Stream { text, .. } = unclaimed.next().await.unwrap().typed() { return text }
        }
    }).await.unwrap();
    assert!(late.trim().parse::<u32>().is_ok());
    while k.status() != KernelStatus::Idle { tokio::time::sleep(Duration::from_millis(50)).await }

    k.restart().await.unwrap();
    let out = run(&k, "print(x)").await;
    assert!(out.iter().any(|c| matches!(c, Content::Error { ename, .. } if ename == "NameError")));
    k.shutdown().await.unwrap();
}

#[tokio::test]
async fn kernel_exits_when_the_app_process_dies() {
    if let Ok(dir) = std::env::var("HOLD_KERNEL_IN") {
        let k = LocalKernel::start(&std::path::Path::new(&dir).join("a.ipynb"), None).await.unwrap();
        println!("KERNEL_PID {}", stdout(&run(&k, "import os\nprint(os.getpid())").await).trim());
        return std::future::pending().await;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut app = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["kernel_exits_when_the_app_process_dies", "--exact", "--nocapture"])
        .env("HOLD_KERNEL_IN", dir.path()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let lines = std::io::BufRead::lines(std::io::BufReader::new(app.stdout.take().unwrap()));
    let pid = lines.map_while(Result::ok).find_map(|l| l.strip_prefix("KERNEL_PID ").map(str::to_owned)).unwrap();
    let alive = || std::process::Command::new("kill").args(["-0", &pid]).status().unwrap().success();
    assert!(alive());
    app.kill().unwrap();
    app.wait().unwrap();
    let gone = (0..50).any(|_| { std::thread::sleep(Duration::from_millis(100)); !alive() });
    assert!(gone, "kernel {pid} outlived the app process");
}
