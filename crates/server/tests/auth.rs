use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use colab_server::kernels::Client;
use colab_server::tailscale::{LocalApi, allowed, serve};
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

fn fixture(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/tailscale/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

#[test]
fn only_the_user_who_signed_the_node_in_gets_in_from_an_untagged_device() {
    let status = fixture("status");
    assert!(allowed(&status, &fixture("whois-anton")));
    assert!(allowed(&status, &fixture("whois-anton-rex")));
    assert!(!allowed(&status, &fixture("whois-shared")));
    assert!(!allowed(&status, &fixture("whois-tagged")));

    let mut signed_out = fixture("status");
    signed_out["User"] = Value::Null;
    assert!(!allowed(&signed_out, &fixture("whois-anton")));
    let mut stray = fixture("whois-anton");
    stray["UserProfile"]["LoginName"] = Value::Null;
    signed_out["User"] = json!({});
    assert!(!allowed(&signed_out, &stray));
}

async fn localapi(State(whois): State<HashMap<&'static str, &'static str>>, axum::extract::Path(path): axum::extract::Path<String>, Query(query): Query<HashMap<String, String>>, headers: HeaderMap) -> Result<axum::Json<Value>, StatusCode> {
    if headers.get("Sec-Tailscale").is_none_or(|v| v != "localapi") || headers.get("Authorization").is_none_or(|v| v != "Basic OmNyZWQ=") {
        return Err(StatusCode::FORBIDDEN);
    }
    match path.as_str() {
        "status" => Ok(axum::Json(fixture("status"))),
        "whois" => whois.get(query["addr"].as_str()).map(|f| axum::Json(fixture(f))).ok_or(StatusCode::NOT_FOUND),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn get(conns: &mpsc::UnboundedSender<(std::os::unix::net::UnixStream, String)>, from: &str, headers: &str) -> (u16, String) {
    let (server, client) = std::os::unix::net::UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    client.set_nonblocking(true).unwrap();
    conns.send((server, from.into())).unwrap();
    let mut client = tokio::net::UnixStream::from_std(client).unwrap();
    client.write_all(format!("GET /api/kernels HTTP/1.1\r\n{headers}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).await.unwrap();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    (status, response.split("\r\n\r\n").nth(1).unwrap_or_default().to_string())
}

#[tokio::test]
async fn the_tailscale_listener_lets_in_only_the_owner_asking_for_this_device_without_an_origin() {
    let whois = HashMap::from([("100.68.126.75", "whois-anton"), ("100.90.25.52", "whois-anton-rex"), ("100.64.0.7", "whois-shared"), ("100.64.0.8", "whois-tagged")]);
    let fake = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", fake.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(fake, axum::Router::new().route("/localapi/v0/{path}", axum::routing::get(localapi)).with_state(whois)).await });
    let app = axum::Router::new().route("/api/kernels", axum::routing::get(|axum::Extension(client): axum::Extension<Client>| async move { client.0 }));
    let (conns, rx) = mpsc::unbounded_channel();
    tokio::spawn(serve(app, LocalApi::new(base, "cred".into()), rx));

    for host in ["mac-work", "mac-work.example.ts.net", "MAC-WORK.example.ts.net:80", "100.124.92.115", "[fd7a:115c:a1e0::b401:5c9f]:80"] {
        assert_eq!(get(&conns, "100.68.126.75", &format!("Host: {host}\r\n")).await, (200, "anton".into()), "{host}");
    }
    assert_eq!(get(&conns, "100.90.25.52", "Host: mac-work\r\n").await, (200, "anton-rex".into()));
    assert_eq!(get(&conns, "100.64.0.7", "Host: mac-work\r\n").await.0, 403);
    assert_eq!(get(&conns, "100.64.0.8", "Host: mac-work\r\n").await.0, 403);
    assert_eq!(get(&conns, "100.64.0.9", "Host: mac-work\r\n").await.0, 403);
    assert_eq!(get(&conns, "100.68.126.75", "Host: mac-work\r\nOrigin: http://mac-work\r\n").await.0, 403);
    assert_eq!(get(&conns, "100.68.126.75", "Host: rebind.example.com\r\n").await.0, 403);
    assert_eq!(get(&conns, "100.68.126.75", "Host: anton.example.ts.net\r\n").await.0, 403);
}
