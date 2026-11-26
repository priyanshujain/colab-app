use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use crate::keystore;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub const SCOPES: &str = "openid https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/colaboratory https://www.googleapis.com/auth/drive.file";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const KEYCHAIN_SERVICE: &str = "colab-app";
const KEYCHAIN_USER: &str = "google-oauth";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_secret: String,
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self {
            client_id: "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com".into(),
            client_secret: "d-FL95Q19q7MQmFpd7hHD0Ty".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(flatten)]
    pub client: OAuthClient,
    pub refresh_token: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Google rejected the OAuth client: {0}. Enter your own OAuth client ID.")]
    ClientRejected(String),
    #[error("signed out: {0}")]
    SignedOut(String),
}

pub struct PendingSignIn {
    pub url: String,
    client: OAuthClient,
    verifier: String,
}

impl PendingSignIn {
    pub async fn complete_sign_in(self, code: &str, redirect_uri: &str) -> Result<Auth> {
        let auth = Auth::new(Tokens { client: self.client.clone(), refresh_token: String::new(), access_token: String::new(), expiry: None }, true);
        let body = auth.token_request(&[("code", code), ("redirect_uri", redirect_uri), ("grant_type", "authorization_code"), ("code_verifier", &self.verifier)], &self.client).await?;
        let mut tokens = auth.tokens.lock().await;
        tokens.refresh_token = body["refresh_token"].as_str().context("no refresh_token")?.into();
        apply(&mut tokens, &body);
        save(&tokens)?;
        drop(tokens);
        Ok(auth)
    }
}

pub struct Loopback {
    listener: tokio::net::TcpListener,
    pub redirect_uri: String,
}

impl Loopback {
    pub async fn bind() -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let redirect_uri = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
        Ok(Self { listener, redirect_uri })
    }

    pub async fn code(&self, auth_url: &str) -> Result<String> {
        let state = url::Url::parse(auth_url)?.query_pairs().find(|(k, _)| k == "state").context("auth url has no state")?.1.into_owned();
        receive_code(&self.listener, &state).await
    }
}

pub struct Auth {
    tokens: Mutex<Tokens>,
    http: reqwest::Client,
    persist: bool,
}

impl Auth {
    pub fn new(tokens: Tokens, persist: bool) -> Self {
        Self { tokens: Mutex::new(tokens), http: reqwest::Client::new(), persist }
    }

    pub fn load(import_cli: bool) -> Result<Option<Self>> {
        if let Some(json) = keychain().get()? {
            return Ok(Some(Self::new(serde_json::from_str(&json)?, true)));
        }
        if !import_cli {
            return Ok(None);
        }
        let Some(tokens) = import_cli_token(&cli_token_path())? else { return Ok(None) };
        save(&tokens)?;
        Ok(Some(Self::new(tokens, true)))
    }

    pub fn begin_sign_in(client: OAuthClient, redirect_uri: &str) -> Result<PendingSignIn> {
        let state = uuid::Uuid::new_v4().to_string();
        let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let challenge = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, sha2::Sha256::digest(&verifier));
        let url = url::Url::parse_with_params("https://accounts.google.com/o/oauth2/v2/auth", [
            ("client_id", client.client_id.as_str()), ("redirect_uri", redirect_uri), ("response_type", "code"),
            ("scope", SCOPES), ("access_type", "offline"), ("prompt", "consent"), ("state", &state),
            ("code_challenge", &challenge), ("code_challenge_method", "S256"),
        ])?.into();
        Ok(PendingSignIn { url, client, verifier })
    }

    pub fn sign_out() -> Result<()> {
        keychain().delete()
    }

    pub async fn access_token(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if tokens.expiry.is_some_and(|e| e - Utc::now() > Duration::seconds(60)) && !tokens.access_token.is_empty() {
            return Ok(tokens.access_token.clone());
        }
        let refresh = tokens.refresh_token.clone();
        let body = self.token_request(&[("refresh_token", refresh.as_str()), ("grant_type", "refresh_token")], &tokens.client).await?;
        apply(&mut tokens, &body);
        if self.persist {
            save(&tokens)?;
        }
        Ok(tokens.access_token.clone())
    }

    pub async fn email(&self) -> Result<String> {
        let token = self.access_token().await?;
        let info: Value = self.http.get("https://openidconnect.googleapis.com/v1/userinfo").bearer_auth(token).send().await?.error_for_status()?.json().await?;
        Ok(info["email"].as_str().unwrap_or_default().into())
    }

    async fn token_request(&self, params: &[(&str, &str)], client: &OAuthClient) -> Result<Value> {
        let mut form = vec![("client_id", client.client_id.as_str()), ("client_secret", client.client_secret.as_str())];
        form.extend_from_slice(params);
        let response = self.http.post(TOKEN_URL).form(&form).send().await?;
        let status = response.status();
        let body: Value = response.json().await?;
        if status.is_success() {
            return Ok(body);
        }
        Err(token_error(&body))
    }
}

fn token_error(body: &Value) -> anyhow::Error {
    let error = body["error"].as_str().unwrap_or("unknown");
    let message = format!("{error}: {}", body["error_description"].as_str().unwrap_or_default());
    match error {
        "invalid_client" | "unauthorized_client" | "deleted_client" | "disabled_client" => AuthError::ClientRejected(message).into(),
        "invalid_grant" => AuthError::SignedOut(message).into(),
        _ => anyhow::anyhow!("token request failed: {message}"),
    }
}

fn apply(tokens: &mut Tokens, body: &Value) {
    tokens.access_token = body["access_token"].as_str().unwrap_or_default().into();
    tokens.expiry = Some(Utc::now() + Duration::seconds(body["expires_in"].as_i64().unwrap_or(0)));
}

fn keychain() -> keystore::Entry {
    keystore::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER, "colab-token.json")
}

fn save(tokens: &Tokens) -> Result<()> {
    keychain().set(&serde_json::to_string(tokens)?)
}

pub fn cli_token_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".config/colab-cli/token.json")
}

pub fn import_cli_token(path: &Path) -> Result<Option<Tokens>> {
    let Ok(text) = std::fs::read_to_string(path) else { return Ok(None) };
    let json: Value = serde_json::from_str(&text)?;
    let field = |k: &str| json[k].as_str().map(String::from);
    let (Some(client_id), Some(client_secret), Some(refresh_token)) = (field("client_id"), field("client_secret"), field("refresh_token")) else {
        return Ok(None);
    };
    let expiry = field("expiry").and_then(|e| e.parse::<DateTime<Utc>>().ok().or_else(|| format!("{e}Z").parse().ok()));
    Ok(Some(Tokens { client: OAuthClient { client_id, client_secret }, refresh_token, access_token: field("token").unwrap_or_default(), expiry }))
}

async fn receive_code(listener: &tokio::net::TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut socket, _) = listener.accept().await?;
        let mut buf = vec![0; 8192];
        let n = socket.read(&mut buf).await?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let Some(target) = request.split_whitespace().nth(1) else { continue };
        let url = url::Url::parse(&format!("http://localhost{target}"))?;
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        if query.get("state").map(String::as_str) != Some(state) {
            continue;
        }
        let page = if query.contains_key("code") { "Signed in. You can close this tab." } else { "Sign in failed. You can close this tab." };
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes()).await?;
        if let Some(code) = query.get("code") {
            return Ok(code.clone());
        }
        bail!("sign in failed: {}", query.get("error").map(String::as_str).unwrap_or("unknown"));
    }
}
