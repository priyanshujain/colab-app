use colab::auth::{Auth, OAuthClient, Tokens, import_cli_token};
use colab::client::{CcuInfo, ProxyInfo, parse_xssi};
use colab::{Colab, ColabKernel, Runtime};
use futures::{SinkExt, StreamExt};
use kernel::{Content, Kernel, Message};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::{self, handshake::server::{Request, Response}};

async fn accept(listener: &TcpListener) -> (tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, String) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut query = String::new();
    let ws = tokio_tungstenite::accept_hdr_async(tcp, |req: &Request, res: Response| {
        assert_eq!(req.headers()["X-Colab-Runtime-Proxy-Token"], "proxy-token");
        query = req.uri().query().unwrap().to_string();
        Ok(res)
    }).await.unwrap();
    (ws, query)
}

fn reply_to(parent: &Value, msg_type: &str, content: Value) -> tungstenite::Message {
    let mut m = Message::new(msg_type, "server", content);
    m.parent_header = serde_json::from_value(parent["header"].clone()).ok();
    tungstenite::Message::text(serde_json::to_string(&m).unwrap())
}

async fn next_json(ws: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) -> Value {
    loop {
        if let tungstenite::Message::Text(t) = ws.next().await.unwrap().unwrap() {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

#[tokio::test]
async fn execution_survives_proxy_drop_with_stable_session_and_colab_requests_get_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut ws, first_query) = accept(&listener).await;
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
        let (mut ws, second_query) = accept(&listener).await;
        assert_eq!(second_query, first_query);
        ws.send(reply_to(&request, "stream", json!({"name": "stdout", "text": "epoch 2\n"}))).await.unwrap();
        ws.send(reply_to(&request, "status", json!({"execution_state": "idle"}))).await.unwrap();
        ws
    });
    let tokens = Tokens { client: OAuthClient::default(), refresh_token: "r".into(), access_token: "a".into(), expiry: None };
    let colab = Colab::new(Arc::new(Auth::new(tokens, false)));
    let info = ProxyInfo { url, token: "proxy-token".into(), token_expires_in_seconds: 3600 };
    let kernel = ColabKernel::connect(Arc::new(Runtime::new(colab, "m-s-test".into(), &info)), "k1".into(), "/tmp/x.ipynb".as_ref());
    let outputs: Vec<String> = kernel.execute("train()").await.unwrap()
        .filter_map(|m| async move { if let Content::Stream { text, .. } = m.typed() { Some(text) } else { None } })
        .collect().await;
    assert_eq!(outputs, ["epoch 1\n", "epoch 2\n"]);
    server.await.unwrap();
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
