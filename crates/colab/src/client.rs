use crate::auth::Auth;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const BASE: &str = "https://colab.research.google.com/tun/m";
const XSSI_PREFIX: &str = ")]}'";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Variant {
    Default,
    Gpu,
    Tpu,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcuInfo {
    #[serde(rename = "currentBalance")]
    pub balance: f64,
    #[serde(rename = "consumptionRateHourly")]
    pub hourly_rate: f64,
    #[serde(default)]
    pub eligible_gpus: Vec<String>,
    #[serde(default)]
    pub eligible_tpus: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInfo {
    pub url: String,
    pub token: String,
    pub token_expires_in_seconds: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub endpoint: String,
    #[serde(default)]
    pub accelerator: String,
    #[serde(default)]
    pub variant: Value,
    pub runtime_proxy_info: ProxyInfo,
}

pub fn parse_xssi<T: serde::de::DeserializeOwned>(body: &str) -> Result<T> {
    let body = body.strip_prefix(XSSI_PREFIX).unwrap_or(body).trim();
    Ok(serde_json::from_str(if body.is_empty() { "null" } else { body })?)
}

#[derive(Clone)]
pub struct Colab {
    pub auth: Arc<Auth>,
    http: reqwest::Client,
}

impl Colab {
    pub fn new(auth: Arc<Auth>) -> Self {
        Self { auth, http: reqwest::Client::new() }
    }

    async fn request<T: serde::de::DeserializeOwned>(&self, method: Method, path: &str, query: &[(&str, &str)], xsrf: Option<&str>) -> Result<T> {
        let mut req = self.http.request(method.clone(), format!("{BASE}/{path}"))
            .bearer_auth(self.auth.access_token().await?)
            .header("Accept", "application/json")
            .header("X-Colab-Client-Agent", "colab-cli")
            .query(&[("authuser", "0")])
            .query(query);
        if let Some(token) = xsrf {
            req = req.header("X-Goog-Colab-Token", token);
        }
        let started = std::time::Instant::now();
        let response = req.send().await.inspect_err(|e| log::warn!("colab {method} {path} failed after {:?}: {e:?}", started.elapsed()))?;
        let status = response.status();
        log::info!("colab {method} {path} {status} in {:?}", started.elapsed());
        let body = response.text().await?;
        if !status.is_success() {
            bail!("{method} {path} failed with {status}: {}", body.chars().take(300).collect::<String>());
        }
        parse_xssi(&body)
    }

    pub async fn ccu_info(&self) -> Result<CcuInfo> {
        self.request(Method::GET, "ccu-info", &[], None).await
    }

    pub async fn assignments(&self) -> Result<Vec<Assignment>> {
        let body: Value = self.request(Method::GET, "assignments", &[], None).await?;
        Ok(serde_json::from_value(body["assignments"].clone()).unwrap_or_default())
    }

    pub async fn assign(&self, variant: Variant, accelerator: Option<&str>, high_ram: bool) -> Result<Assignment> {
        let nbh = uuid::Uuid::new_v4().to_string().replace('-', "_");
        let nbh = format!("{nbh}{}", ".".repeat(44 - nbh.len()));
        let variant = serde_json::to_value(variant)?;
        let mut query = vec![("nbh", nbh.as_str()), ("variant", variant.as_str().unwrap())];
        if let Some(acc) = accelerator {
            query.push(("accelerator", acc));
        }
        if high_ram {
            query.push(("shape", "hm"));
        }
        let first: Value = self.request(Method::GET, "assign", &query, None).await?;
        if first.get("endpoint").is_some() {
            return Ok(serde_json::from_value(first)?);
        }
        let token = first["token"].as_str().context("assign returned no xsrf token")?;
        self.request(Method::POST, "assign", &query, Some(token)).await
    }

    pub async fn unassign(&self, endpoint: &str) -> Result<()> {
        let path = format!("unassign/{endpoint}");
        let first: Value = self.request(Method::GET, &path, &[], None).await?;
        let token = first["token"].as_str().context("unassign returned no xsrf token")?;
        let _: Value = self.request(Method::POST, &path, &[], Some(token)).await?;
        Ok(())
    }
}

struct Proxy {
    url: String,
    token: String,
    expires_at: DateTime<Utc>,
}

pub struct Runtime {
    pub colab: Colab,
    pub endpoint: String,
    proxy: Mutex<Proxy>,
    http: reqwest::Client,
}

impl Runtime {
    pub fn new(colab: Colab, endpoint: String, info: &ProxyInfo) -> Self {
        Self { colab, endpoint, proxy: Mutex::new(proxy(info)), http: reqwest::Client::new() }
    }
}

#[async_trait::async_trait]
pub trait JupyterServer: Send + Sync + 'static {
    async fn url_and_headers(&self) -> Result<(String, Vec<(&'static str, String)>)>;
    fn http(&self) -> reqwest::Client;

    fn socks(&self) -> Option<Socks> {
        None
    }

    fn query(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    fn logs(&self) -> bool {
        true
    }

    async fn request(&self, method: Method, path: &str, extra: &[(&'static str, String)]) -> Result<reqwest::Response> {
        let (url, headers) = self.url_and_headers().await?;
        let mut req = self.http().request(method.clone(), format!("{url}{path}")).query(self.query());
        for (k, v) in headers.iter().chain(extra) {
            req = req.header(*k, v);
        }
        let started = std::time::Instant::now();
        let response = req.send().await.inspect_err(|e| log::warn!("jupyter {method} {path} failed after {:?}: {e:?}", started.elapsed()))?;
        log::info!("jupyter {method} {path} {} in {:?}", response.status(), started.elapsed());
        Ok(response.error_for_status()?)
    }

    async fn jupyter(&self, method: Method, path: &str) -> Result<Value> {
        let body = self.request(method, path, &[]).await?.text().await?;
        Ok(if body.is_empty() { Value::Null } else { serde_json::from_str(&body)? })
    }

    async fn file_from(&self, path: &str, offset: u64) -> Result<String> {
        Ok(self.request(Method::GET, &format!("/files{path}"), &[("Range", format!("bytes={offset}-"))]).await?.text().await?)
    }
}

#[async_trait::async_trait]
impl JupyterServer for Runtime {
    async fn url_and_headers(&self) -> Result<(String, Vec<(&'static str, String)>)> {
        let mut p = self.proxy.lock().await;
        if p.expires_at - Utc::now() < Duration::minutes(5) {
            let a = self.colab.assignments().await?.into_iter().find(|a| a.endpoint == self.endpoint)
                .with_context(|| format!("runtime {} is no longer assigned", self.endpoint))?;
            *p = proxy(&a.runtime_proxy_info);
        }
        Ok((p.url.clone(), vec![("X-Colab-Runtime-Proxy-Token", p.token.clone())]))
    }

    fn http(&self) -> reqwest::Client {
        self.http.clone()
    }

    fn query(&self) -> &'static [(&'static str, &'static str)] {
        &[("authuser", "0")]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Socks {
    pub addr: String,
    pub user: String,
    pub password: String,
}

pub fn split_address(address: &str) -> (String, u16) {
    let address = address.trim();
    if address.parse::<std::net::Ipv6Addr>().is_ok() {
        return (address.into(), 80);
    }
    match address.rsplit_once(':').and_then(|(h, p)| Some((h, p.parse::<u16>().ok()?))) {
        Some((host, port)) => (host.trim_matches(['[', ']']).into(), port),
        None => (address.into(), 80),
    }
}

pub fn base_url(address: &str) -> String {
    match split_address(address) {
        (host, port) if host.contains(':') => format!("http://[{host}]:{port}"),
        (host, port) => format!("http://{host}:{port}"),
    }
}

pub fn not_found(e: &anyhow::Error) -> bool {
    e.downcast_ref::<reqwest::Error>().and_then(reqwest::Error::status) == Some(reqwest::StatusCode::NOT_FOUND)
}

fn client(socks: Option<&Socks>) -> reqwest::Client {
    let Some(s) = socks else { return reqwest::Client::new() };
    reqwest::Proxy::all(format!("socks5h://{}:{}@{}", s.user, s.password, s.addr)).and_then(|p| reqwest::Client::builder().no_proxy().proxy(p).build()).unwrap_or_default()
}

pub struct RemoteHost {
    pub label: String,
    base: String,
    socks: tokio::sync::watch::Receiver<Option<Socks>>,
    http: std::sync::Mutex<(Option<Socks>, reqwest::Client)>,
    patience: std::time::Duration,
}

impl RemoteHost {
    pub fn new(label: &str, address: &str, socks: tokio::sync::watch::Receiver<Option<Socks>>) -> Self {
        Self { label: label.into(), base: base_url(address), socks, http: std::sync::Mutex::new((None, reqwest::Client::new())), patience: std::time::Duration::from_secs(4) }
    }

    pub fn patience(self, patience: std::time::Duration) -> Self {
        Self { patience, ..self }
    }

    pub async fn reachable(&self) -> Result<()> {
        let deadline = tokio::time::Instant::now() + self.patience;
        while tokio::time::Instant::now() < deadline {
            if let Ok(Ok(_)) = tokio::time::timeout_at(deadline, self.http().get(format!("{}/api/kernels", self.base)).send()).await {
                return Ok(());
            }
            tokio::time::sleep_until(deadline.min(tokio::time::Instant::now() + std::time::Duration::from_millis(500))).await;
        }
        bail!("host {} is unavailable", self.label)
    }

    pub async fn rpc(&self) -> Result<crate::kernel::WebSocket> {
        let request = format!("{}/rpc", self.base.replacen("http://", "ws://", 1)).into_client_request()?;
        crate::kernel::websocket(request, self.socks().as_ref()).await
    }
}

#[async_trait::async_trait]
impl JupyterServer for RemoteHost {
    async fn url_and_headers(&self) -> Result<(String, Vec<(&'static str, String)>)> {
        Ok((self.base.clone(), Vec::new()))
    }

    fn http(&self) -> reqwest::Client {
        let socks = self.socks();
        let mut http = self.http.lock().unwrap();
        if http.0 != socks {
            *http = (socks.clone(), client(socks.as_ref()));
        }
        http.1.clone()
    }

    fn socks(&self) -> Option<Socks> {
        self.socks.borrow().clone()
    }

    fn logs(&self) -> bool {
        false
    }
}

fn proxy(info: &ProxyInfo) -> Proxy {
    Proxy { url: info.url.trim_end_matches('/').into(), token: info.token.clone(), expires_at: Utc::now() + Duration::seconds(info.token_expires_in_seconds) }
}
