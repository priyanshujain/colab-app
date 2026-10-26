use colab::auth::{Auth, OAuthClient, Tokens, import_cli_token};
use colab::client::{CcuInfo, ProxyInfo, parse_xssi};
use colab::{Colab, ColabKernel, Runtime};
use futures::{SinkExt, StreamExt};
use kernel::{Content, Kernel, Message};
use kernel::KernelStatus;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{self, handshake::server::{Request, Response}};

type Ws = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

struct Proxy {
    url: String,
    state: Arc<Mutex<&'static str>>,
    log: Arc<Mutex<String>>,
    sockets: mpsc::UnboundedReceiver<(Ws, String)>,
}

impl Proxy {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new("idle"));
        let (tx, sockets) = mpsc::unbounded_channel();
        let log = Arc::new(Mutex::new(String::new()));
        let (kernel_state, kernel_log) = (state.clone(), log.clone());
        tokio::spawn(async move {
            loop {
                let (mut tcp, _) = listener.accept().await.unwrap();
                let mut head = vec![0; 4096];
                let n = tcp.peek(&mut head).await.unwrap();
                if String::from_utf8_lossy(&head[..n]).to_lowercase().contains("upgrade: websocket") {
                    let mut query = String::new();
                    let ws = tokio_tungstenite::accept_hdr_async(tcp, |req: &Request, res: Response| {
                        assert_eq!(req.headers()["X-Colab-Runtime-Proxy-Token"], "proxy-token");
                        query = req.uri().query().unwrap().to_string();
                        Ok(res)
                    }).await.unwrap();
                    let _ = tx.send((ws, query));
                } else {
                    let n = tcp.read(&mut head).await.unwrap();
                    let request = String::from_utf8_lossy(&head[..n]).to_lowercase();
                    let (status, body) = if request.starts_with("get /api/kernels/k1?") {
                        ("200 OK", json!({"id": "k1", "execution_state": *kernel_state.lock().unwrap()}).to_string())
                    } else {
                        assert!(request.starts_with("get /files/tmp/colab-app/k1.jsonl?"), "{request}");
                        let from: usize = request.split("range: bytes=").nth(1).unwrap().split('-').next().unwrap().parse().unwrap();
                        ("206 Partial Content", kernel_log.lock().unwrap()[from..].to_string())
                    };
                    tcp.write_all(format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
                }
            }
        });
        Self { url, state, log, sockets }
    }

    fn output(&self, parent: &Value, msg_type: &str, content: Value) -> tungstenite::Message {
        let mut log = self.log.lock().unwrap();
        let mut m = Message::new(msg_type, "server", content);
        m.parent_header = serde_json::from_value(parent["header"].clone()).ok();
        m.metadata.insert("colab_app_log".into(), log.len().into());
        let line = serde_json::to_string(&m).unwrap();
        log.push_str(&line);
        log.push('\n');
        tungstenite::Message::text(line)
    }

    async fn accept(&mut self) -> (Ws, String) {
        self.sockets.recv().await.unwrap()
    }

    fn kernel(&self) -> ColabKernel {
        let tokens = Tokens { client: OAuthClient::default(), refresh_token: "r".into(), access_token: "a".into(), expiry: None };
        let colab = Colab::new(Arc::new(Auth::new(tokens, false)));
        let info = ProxyInfo { url: self.url.clone(), token: "proxy-token".into(), token_expires_in_seconds: 3600 };
        ColabKernel::connect(Arc::new(Runtime::new(colab, "m-s-test".into(), &info)), "k1".into(), "/tmp/x.ipynb".as_ref())
    }
}

fn reply_to(parent: &Value, msg_type: &str, content: Value) -> tungstenite::Message {
    let mut m = Message::new(msg_type, "server", content);
    m.parent_header = serde_json::from_value(parent["header"].clone()).ok();
    tungstenite::Message::text(serde_json::to_string(&m).unwrap())
}

async fn next_json(ws: &mut Ws) -> Value {
    loop {
        if let tungstenite::Message::Text(t) = ws.next().await.unwrap().unwrap() {
            let msg: Value = serde_json::from_str(&t).unwrap();
            if msg["content"]["silent"] != true {
                return msg;
            }
            assert!(msg["content"]["code"].as_str().unwrap().ends_with("_colab_app_log(\"/tmp/colab-app/k1.jsonl\")"));
        }
    }
}

#[tokio::test]
async fn execution_survives_proxy_drop_with_stable_session_and_colab_requests_get_errors() {
    let mut proxy = Proxy::start().await;
    let kernel = proxy.kernel();
    let server = tokio::spawn(async move {
        let (mut ws, first_query) = proxy.accept().await;
        let request = next_json(&mut ws).await;
        assert_eq!(request["header"]["msg_type"], "execute_request");
        assert_eq!(first_query, format!("session_id={}", request["header"]["session"].as_str().unwrap()));
        ws.send(reply_to(&request, "stream", json!({"name": "stdout", "text": "epoch 1\n"}))).await.unwrap();
        let mut colab = Message::new("colab_request", "server", json!({"request": {"authType": "dfs_ephemeral"}}));
        colab.metadata = serde_json::from_value(json!({"colab_request_type": "request_auth", "colab_msg_id": 1})).unwrap();
        ws.send(tungstenite::Message::text(serde_json::to_string(&colab).unwrap())).await.unwrap();
        let reply = next_json(&mut ws).await;
        assert_eq!((reply["channel"].clone(), reply["header"]["msg_type"].clone()), (json!("stdin"), json!("input_reply")));
        assert_eq!(reply["header"]["session"], request["header"]["session"]);
        assert_eq!(reply["content"]["value"]["type"], "colab_reply");
        assert_eq!(reply["content"]["value"]["colab_msg_id"], 1);
        assert!(reply["content"]["value"]["error"].is_string());
        drop(ws);
        let (mut ws, second_query) = proxy.accept().await;
        assert_eq!(second_query, first_query);
        ws.send(reply_to(&request, "stream", json!({"name": "stdout", "text": "epoch 2\n"}))).await.unwrap();
        ws.send(reply_to(&request, "status", json!({"execution_state": "idle"}))).await.unwrap();
        ws
    });
    let outputs: Vec<String> = kernel.execute("train()").await.unwrap()
        .filter_map(|m| async move { if let Content::Stream { text, .. } = m.typed() { Some(text) } else { None } })
        .collect().await;
    assert_eq!(outputs, ["epoch 1\n", "epoch 2\n"]);
    server.await.unwrap();
}

async fn stdout(execution: kernel::Execution) -> Vec<String> {
    execution.filter_map(|m| async move { if let Content::Stream { text, .. } = m.typed() { Some(text) } else { None } }).collect().await
}

async fn start_epoch_then_sleep(proxy: &mut Proxy, kernel: &ColabKernel) -> (Value, Ws, tokio::task::JoinHandle<Vec<String>>, Instant) {
    let (mut ws, _) = proxy.accept().await;
    let run = tokio::spawn(stdout(kernel.execute("train()").await.unwrap()));
    let request = next_json(&mut ws).await;
    *proxy.state.lock().unwrap() = "busy";
    ws.send(proxy.output(&request, "status", json!({"execution_state": "busy"}))).await.unwrap();
    ws.send(proxy.output(&request, "stream", json!({"name": "stdout", "text": "epoch 1\n"}))).await.unwrap();
    while kernel.status() != KernelStatus::Busy {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    proxy.output(&request, "stream", json!({"name": "stdout", "text": "epoch 2 while asleep\n"}));
    (request, ws, run, Instant::now())
}

#[tokio::test]
async fn cell_still_running_after_laptop_sleep_resumes_with_output_printed_while_asleep() {
    let mut proxy = Proxy::start().await;
    let kernel = proxy.kernel();
    let (request, _half_open, run, slept) = start_epoch_then_sleep(&mut proxy, &kernel).await;
    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(60), proxy.accept()).await.expect("dead connection was never replaced");
    assert!(slept.elapsed() >= Duration::from_secs(20), "reconnected after {:?} while pongs were missing", slept.elapsed());
    let hook = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert!(hook.contains(r#""silent":true"#) && hook.contains("_colab_app_log"), "{hook}");
    assert_eq!(kernel.status(), KernelStatus::Busy);
    assert!(!run.is_finished());
    ws.send(proxy.output(&request, "stream", json!({"name": "stdout", "text": "epoch 3\n"}))).await.unwrap();
    *proxy.state.lock().unwrap() = "idle";
    ws.send(proxy.output(&request, "status", json!({"execution_state": "idle"}))).await.unwrap();
    assert_eq!(run.await.unwrap(), ["epoch 1\n", "epoch 2 while asleep\n", "epoch 3\n"]);
    assert_eq!(kernel.status(), KernelStatus::Idle);
}

#[tokio::test]
async fn cell_that_finished_during_laptop_sleep_ends_with_all_its_output() {
    let mut proxy = Proxy::start().await;
    let kernel = proxy.kernel();
    let (request, _half_open, run, _) = start_epoch_then_sleep(&mut proxy, &kernel).await;
    proxy.output(&request, "error", json!({"ename": "ValueError", "evalue": "loss is nan", "traceback": []}));
    proxy.output(&request, "status", json!({"execution_state": "idle"}));
    *proxy.state.lock().unwrap() = "idle";
    let outputs = tokio::time::timeout(Duration::from_secs(30), run).await.expect("execution kept waiting for an idle that was lost").unwrap();
    assert_eq!(outputs, ["epoch 1\n", "epoch 2 while asleep\n"]);
    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(60), proxy.accept()).await.expect("dead connection was never replaced");
    let _next = kernel.execute("print('after wake')").await.unwrap();
    assert_eq!(next_json(&mut ws).await["content"]["code"], "print('after wake')");
    assert_eq!(kernel.status(), KernelStatus::Idle);
}

#[test]
fn imports_colab_cli_token_and_parses_xssi_backend_responses() {
    let path = std::env::temp_dir().join(format!("{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&path, r#"{"token": "ya29.x", "refresh_token": "1//0g", "token_uri": "https://oauth2.googleapis.com/token", "client_id": "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com", "client_secret": "d-FL95Q19q7MQmFpd7hHD0Ty", "scopes": ["openid"], "universe_domain": "googleapis.com", "account": "", "expiry": "2026-10-05T11:38:23Z"}"#).unwrap();
    let tokens = import_cli_token(&path).unwrap().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(tokens.client, OAuthClient::default());
    assert_eq!((tokens.refresh_token.as_str(), tokens.access_token.as_str()), ("1//0g", "ya29.x"));
    assert_eq!(tokens.expiry.unwrap().to_rfc3339(), "2026-10-05T11:38:23+00:00");
    let ccu: CcuInfo = parse_xssi(")]}'\n{\"currentBalance\":12.5, \"consumptionRateHourly\":1.76, \"assignmentsCount\":1, \"eligibleGpus\":[\"T4\"], \"ineligibleGpus\":[\"H100\"], \"eligibleTpus\":[\"V5E1\"], \"ineligibleTpus\":[\"V6E1\"]}").unwrap();
    assert_eq!(ccu, CcuInfo { balance: 12.5, hourly_rate: 1.76, eligible_gpus: vec!["T4".into()], eligible_tpus: vec!["V5E1".into()] });
}

#[test]
fn colab_links_map_to_public_download_urls() {
    use colab::clone::download_url;
    assert_eq!(
        download_url("https://colab.research.google.com/drive/1aBcD_eFgH-ijKlmNoP?usp=sharing#scrollTo=xyz").unwrap(),
        ("https://drive.google.com/uc?export=download&id=1aBcD_eFgH-ijKlmNoP".into(), "1aBcD_eFgH-ijKlmNoP.ipynb".into())
    );
    assert_eq!(
        download_url("https://colab.research.google.com/github/tensorflow/docs/blob/master/site/en/tutorials/quickstart/beginner.ipynb").unwrap(),
        ("https://raw.githubusercontent.com/tensorflow/docs/master/site/en/tutorials/quickstart/beginner.ipynb".into(), "beginner.ipynb".into())
    );
    assert!(download_url("https://drive.google.com/file/d/abc/view").is_err());
    assert!(download_url("https://colab.research.google.com/github/tensorflow/docs").is_err());
}
