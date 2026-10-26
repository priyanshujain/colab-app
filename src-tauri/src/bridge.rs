use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub const NOT_RUNNING: &str = "Colab Notebook is not running. Open the app, then retry.";

struct Upstream {
    client: reqwest::Client,
    url: String,
    session: Mutex<Option<String>>,
    initialize: Mutex<Option<String>>,
}

enum Failure {
    Down,
    SessionGone,
    Other(String),
}

impl Upstream {
    async fn post(&self, body: &str, session: Option<String>) -> Result<(Vec<String>, Option<String>), Failure> {
        let mut req = self.client.post(&self.url).header("content-type", "application/json").header("accept", "application/json, text/event-stream").body(body.to_string());
        if let Some(id) = &session {
            req = req.header("mcp-session-id", id);
        }
        let res = req.send().await.map_err(|e| if e.is_connect() { Failure::Down } else { Failure::Other(e.to_string()) })?;
        if res.status() == reqwest::StatusCode::NOT_FOUND && session.is_some() {
            return Err(Failure::SessionGone);
        }
        let new_session = res.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()).map(String::from);
        let sse = res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("text/event-stream"));
        let status = res.status();
        let text = res.text().await.map_err(|e| Failure::Other(e.to_string()))?;
        if !status.is_success() {
            return Err(Failure::Other(format!("HTTP {status}: {text}")));
        }
        let messages = if sse { events(&text) } else { Some(text).filter(|t| !t.trim().is_empty()).into_iter().collect() };
        Ok((messages, new_session))
    }

    async fn initialize(&self, line: &str) -> Result<Vec<String>, Failure> {
        let (messages, session) = self.post(line, None).await?;
        *self.session.lock().await = session;
        Ok(messages)
    }

    async fn reconnect(&self, stale: Option<String>) -> Result<(), Failure> {
        let mut session = self.session.lock().await;
        if *session != stale {
            return Ok(());
        }
        let init = self.initialize.lock().await.clone().ok_or(Failure::Down)?;
        let (_, fresh) = self.post(&init, None).await?;
        self.post(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, fresh.clone()).await?;
        *session = fresh;
        Ok(())
    }

    async fn forward(&self, line: &str) -> Result<Vec<String>, Failure> {
        let session = self.session.lock().await.clone();
        match self.post(line, session.clone()).await {
            Err(Failure::SessionGone) => {
                self.reconnect(session).await?;
                let session = self.session.lock().await.clone();
                self.post(line, session).await.map(|r| r.0)
            }
            result => result.map(|r| r.0),
        }
    }
}

fn events(text: &str) -> Vec<String> {
    text.split("\n\n").filter_map(|event| {
        let data: Vec<&str> = event.lines().filter_map(|l| l.strip_prefix("data:")).map(|d| d.strip_prefix(' ').unwrap_or(d)).collect();
        Some(data.join("\n")).filter(|d| !d.trim().is_empty())
    }).collect()
}

pub async fn bridge(url: &str, input: impl AsyncRead + Unpin, output: impl AsyncWrite + Unpin + Send + 'static) -> anyhow::Result<()> {
    let client = reqwest::Client::builder().pool_max_idle_per_host(0).build()?;
    let upstream = Arc::new(Upstream { client, url: url.into(), session: Mutex::new(None), initialize: Mutex::new(None) });
    let output = Arc::new(Mutex::new(output));
    let mut lines = BufReader::new(input).lines();
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(line) = lines.next_line().await? {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let initializing = message["method"] == "initialize";
        if initializing {
            *upstream.initialize.lock().await = Some(line.clone());
        }
        let (upstream, output) = (upstream.clone(), output.clone());
        let task = async move {
            let result = if initializing { upstream.initialize(&line).await } else { upstream.forward(&line).await };
            let replies = match (result, message.get("id")) {
                (Ok(replies), _) => replies,
                (Err(_), None) => vec![],
                (Err(failure), Some(id)) => {
                    let text = match failure { Failure::Down | Failure::SessionGone => NOT_RUNNING.to_string(), Failure::Other(e) => e };
                    vec![json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": text}}).to_string()]
                }
            };
            let mut out = output.lock().await;
            for reply in replies {
                let _ = out.write_all(format!("{reply}\n").as_bytes()).await;
            }
            let _ = out.flush().await;
        };
        if initializing { task.await } else { drop(tasks.spawn(task)) }
    }
    while tasks.join_next().await.is_some() {}
    Ok(())
}

pub fn run() {
    let url = format!("http://127.0.0.1:{}/mcp", crate::mcp::PORT);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(e) = runtime.block_on(bridge(&url, tokio::io::stdin(), tokio::io::stdout())) {
        eprintln!("colab mcp bridge: {e:#}");
        std::process::exit(1);
    }
}
