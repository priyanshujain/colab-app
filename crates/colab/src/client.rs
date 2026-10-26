use crate::auth::Auth;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

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

    pub async fn url_and_token(&self) -> Result<(String, String)> {
        let mut p = self.proxy.lock().await;
        if p.expires_at - Utc::now() < Duration::minutes(5) {
            let a = self.colab.assignments().await?.into_iter().find(|a| a.endpoint == self.endpoint)
                .with_context(|| format!("runtime {} is no longer assigned", self.endpoint))?;
            *p = proxy(&a.runtime_proxy_info);
        }
        Ok((p.url.clone(), p.token.clone()))
    }

    pub async fn jupyter(&self, method: Method, path: &str) -> Result<Value> {
        let (url, token) = self.url_and_token().await?;
        let started = std::time::Instant::now();
        let response = self.http.request(method.clone(), format!("{url}{path}")).header("X-Colab-Runtime-Proxy-Token", token)
            .query(&[("authuser", "0")]).send().await.inspect_err(|e| log::warn!("runtime {method} {path} failed after {:?}: {e:?}", started.elapsed()))?;
        log::info!("runtime {method} {path} {} in {:?}", response.status(), started.elapsed());
        let response = response.error_for_status()?;
        let body = response.text().await?;
        Ok(if body.is_empty() { Value::Null } else { serde_json::from_str(&body)? })
    }

    pub async fn file_from(&self, path: &str, offset: u64) -> Result<String> {
        let (url, token) = self.url_and_token().await?;
        let response = self.http.get(format!("{url}/files{path}")).header("X-Colab-Runtime-Proxy-Token", token).header("Range", format!("bytes={offset}-"))
            .query(&[("authuser", "0")]).send().await?.error_for_status()?;
        Ok(response.text().await?)
    }
}

fn proxy(info: &ProxyInfo) -> Proxy {
    Proxy { url: info.url.trim_end_matches('/').into(), token: info.token.clone(), expires_at: Utc::now() + Duration::seconds(info.token_expires_in_seconds) }
}
