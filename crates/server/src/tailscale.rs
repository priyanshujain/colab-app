use crate::ipc::{HostPath, TailscaleState, TailscaleStatus, events};
use crate::kernels::KernelHost;
use crate::protocol::{self, Events, Rpc};
use anyhow::{Context, Result, bail};
use axum::extract::connect_info::{ConnectInfo, Connected};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::serve::IncomingStream;
use colab::{RemoteHost, Socks};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ffi::{CStr, CString, c_char, c_int};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{OnceCell, mpsc, watch};
use tokio::task::JoinHandle;

unsafe extern "C" {
    fn tailscale_new() -> c_int;
    fn tailscale_start(sd: c_int) -> c_int;
    fn tailscale_close(sd: c_int) -> c_int;
    fn tailscale_set_dir(sd: c_int, dir: *const c_char) -> c_int;
    fn tailscale_set_hostname(sd: c_int, hostname: *const c_char) -> c_int;
    fn tailscale_set_logfd(sd: c_int, fd: c_int) -> c_int;
    fn tailscale_listen(sd: c_int, network: *const c_char, addr: *const c_char, out: *mut c_int) -> c_int;
    fn tailscale_accept(listener: c_int, out: *mut c_int) -> c_int;
    fn tailscale_getremoteaddr(listener: c_int, conn: c_int, buf: *mut c_char, len: usize) -> c_int;
    fn tailscale_loopback(sd: c_int, addr: *mut c_char, len: usize, proxy: *mut c_char, local: *mut c_char) -> c_int;
    fn tailscale_errmsg(sd: c_int, buf: *mut c_char, len: usize) -> c_int;
}

fn text(buf: &[c_char]) -> String {
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

struct Node(c_int);

impl Node {
    fn start(dir: &Path, hostname: &str) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let node = Self(unsafe { tailscale_new() });
        let (dir, hostname) = (CString::new(dir.as_os_str().as_bytes())?, CString::new(hostname)?);
        node.check(unsafe { tailscale_set_dir(node.0, dir.as_ptr()) })?;
        node.check(unsafe { tailscale_set_hostname(node.0, hostname.as_ptr()) })?;
        node.check(unsafe { tailscale_set_logfd(node.0, -1) })?;
        node.check(unsafe { tailscale_start(node.0) })?;
        Ok(node)
    }

    fn check(&self, code: c_int) -> Result<()> {
        if code == 0 {
            return Ok(());
        }
        let mut buf = [0; 1024];
        unsafe { tailscale_errmsg(self.0, buf.as_mut_ptr(), buf.len()) };
        bail!("{}", Some(text(&buf)).filter(|m| !m.is_empty()).unwrap_or_else(|| format!("libtailscale error {code}")))
    }

    fn loopback(&self) -> Result<(String, String, String)> {
        let (mut addr, mut proxy, mut local) = ([0; 128], [0; 33], [0; 33]);
        self.check(unsafe { tailscale_loopback(self.0, addr.as_mut_ptr(), addr.len(), proxy.as_mut_ptr(), local.as_mut_ptr()) })?;
        Ok((text(&addr), text(&proxy), text(&local)))
    }

    fn listen(&self, addr: &str) -> Result<Listener> {
        let (network, addr, mut fd) = (CString::new("tcp")?, CString::new(addr)?, -1);
        self.check(unsafe { tailscale_listen(self.0, network.as_ptr(), addr.as_ptr(), &mut fd) })?;
        Ok(Listener(unsafe { UnixStream::from_raw_fd(fd) }))
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        unsafe { tailscale_close(self.0) };
    }
}

struct Listener(UnixStream);

impl Listener {
    fn accept(&self) -> std::result::Result<(UnixStream, String), c_int> {
        let mut fd = -1;
        let code = unsafe { tailscale_accept(self.0.as_raw_fd(), &mut fd) };
        if code != 0 {
            return Err(code);
        }
        let conn = unsafe { UnixStream::from_raw_fd(fd) };
        let mut buf = [0; 64];
        unsafe { tailscale_getremoteaddr(self.0.as_raw_fd(), fd, buf.as_mut_ptr(), buf.len()) };
        conn.set_nonblocking(true).map_err(|_| -1)?;
        Ok((conn, text(&buf).trim_matches(['[', ']']).to_string()))
    }
}

#[derive(Clone)]
pub struct LocalApi {
    base: String,
    cred: String,
    http: reqwest::Client,
}

impl LocalApi {
    pub fn new(base: String, cred: String) -> Self {
        Self { base, cred, http: reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap_or_default() }
    }

    async fn call(&self, method: Method, path: &str) -> Result<Value> {
        let response = self.http.request(method, format!("{}/localapi/v0/{path}", self.base)).header("Sec-Tailscale", "localapi").basic_auth("", Some(&self.cred)).send().await?;
        let (status, body) = (response.status(), response.text().await?);
        if !status.is_success() {
            bail!("Tailscale LocalAPI {path}: {status} {}", body.trim());
        }
        Ok(if body.trim().is_empty() { Value::Null } else { serde_json::from_str(&body)? })
    }
}

pub fn allowed(status: &Value, whois: &Value) -> bool {
    let owner = status["User"][status["Self"]["UserID"].to_string()]["LoginName"].as_str();
    let tagged = whois["Node"]["Tags"].as_array().is_some_and(|t| !t.is_empty());
    !tagged && owner.is_some() && whois["UserProfile"]["LoginName"].as_str() == owner
}

fn status_from(hostname: &str, status: &Value) -> TailscaleStatus {
    let some = |v: &Value| v.as_str().map(|s| s.trim_end_matches('.').to_string()).filter(|s| !s.is_empty());
    let state = match status["BackendState"].as_str() {
        Some("Running") => TailscaleState::Running,
        Some("NeedsLogin" | "NeedsMachineAuth") => TailscaleState::NeedsLogin,
        _ => TailscaleState::Starting,
    };
    let running = state == TailscaleState::Running;
    TailscaleStatus {
        state,
        hostname: hostname.into(),
        dns_name: some(&status["Self"]["DNSName"]).filter(|_| running),
        login_name: some(&status["User"][status["Self"]["UserID"].to_string()]["LoginName"]).filter(|_| running),
        auth_url: some(&status["AuthURL"]).filter(|_| !running),
        error: None,
    }
}

pub fn peer<'a>(status: &'a Value, address: &str) -> Option<&'a Value> {
    let name = colab::split_address(address).0.trim_end_matches('.').to_lowercase();
    status["Peer"].as_object()?.values().find(|p| {
        let dns = p["DNSName"].as_str().unwrap_or_default().trim_end_matches('.').to_lowercase();
        dns == name || dns.split('.').next() == Some(name.as_str()) || p["TailscaleIPs"].as_array().is_some_and(|ips| ips.iter().any(|ip| ip.as_str() == Some(name.as_str())))
    })
}

#[derive(Clone)]
pub struct Netmap {
    pub since: Instant,
    pub status: Value,
    api: LocalApi,
}

pub fn patience(netmap: Option<&Netmap>, address: &str) -> Duration {
    let Some(Netmap { since, status, .. }) = netmap else { return Duration::from_secs(4) };
    match peer(status, address).map(|p| p["Online"] != false) {
        Some(false) => Duration::ZERO,
        Some(true) if since.elapsed() < Duration::from_secs(60) => Duration::from_secs(15),
        _ => Duration::from_secs(4),
    }
}

pub fn ping_status(ping: &Value) -> Option<(HostPath, Option<u32>)> {
    let set = |key: &str| ping[key].as_str().is_some_and(|s| !s.is_empty());
    if set("Err") {
        return None;
    }
    let path = if set("Endpoint") && !set("PeerRelay") { HostPath::Direct } else { HostPath::Relay };
    Some((path, ping["LatencySeconds"].as_f64().map(|s| (s * 1000.0).round() as u32)))
}

pub async fn reach(netmap: Option<&Netmap>, host: &RemoteHost, address: &str) -> Option<(Option<HostPath>, Option<u32>)> {
    let Some((netmap, p)) = netmap.and_then(|n| Some((n, peer(&n.status, address)?))) else {
        let started = Instant::now();
        return host.reachable().await.ok().map(|()| (None, Some(started.elapsed().as_millis() as u32)));
    };
    if p["Online"] == false {
        return None;
    }
    let ip = p["TailscaleIPs"][0].as_str().unwrap_or_default();
    let ping = async |kind: &str, wait: Duration| tokio::time::timeout(wait, netmap.api.call(Method::POST, &format!("ping?ip={ip}&type={kind}"))).await;
    match ping("TSMP", patience(Some(netmap), address)).await {
        Ok(Ok(tsmp)) => {
            let (_, latency) = ping_status(&tsmp)?;
            let disco = ping("disco", Duration::from_secs(2)).await.ok().and_then(Result::ok).and_then(|d| ping_status(&d));
            Some(disco.map_or((None, latency), |(path, latency)| (Some(path), latency)))
        }
        Ok(Err(e)) => (log::warn!("Tailscale ping {}: {e:#}", host.label), Some((None, None))).1,
        Err(_) => None,
    }
}

#[derive(Serialize, Deserialize)]
struct Settings {
    enabled: bool,
    hostname: String,
}

fn short_hostname() -> String {
    let mut buf = [0u8; 256];
    unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len() - 1) };
    let name = CStr::from_bytes_until_nul(&buf).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    name.split('.').next().unwrap_or_default().to_lowercase()
}

pub fn default_hostname() -> String {
    format!("{}-colab", short_hostname())
}

pub fn machine(device: &str) -> String {
    let name = device.split('.').next().unwrap_or_default().to_lowercase();
    name.strip_suffix("-colab").unwrap_or(&name).to_string()
}

pub fn machine_name(dir: &Path) -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let settings = Tailscale::settings(dir);
        if settings.enabled { machine(&settings.hostname) } else { short_hostname() }
    }).clone()
}

#[derive(Clone)]
struct Peer {
    addr: String,
    client: Arc<OnceCell<Option<(String, Vec<String>)>>>,
}

impl Connected<IncomingStream<'_, Incoming>> for Peer {
    fn connect_info(stream: IncomingStream<'_, Incoming>) -> Self {
        Self { addr: stream.remote_addr().clone(), client: Arc::default() }
    }
}

struct Incoming(mpsc::UnboundedReceiver<(UnixStream, String)>);

impl axum::serve::Listener for Incoming {
    type Io = tokio::net::UnixStream;
    type Addr = String;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let Some((conn, addr)) = self.0.recv().await else { return std::future::pending().await };
            match tokio::net::UnixStream::from_std(conn) {
                Ok(io) => return (io, addr),
                Err(e) => log::warn!("Tailscale connection from {addr}: {e}"),
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(String::new())
    }
}

fn names(status: &Value) -> Vec<String> {
    let dns = status["Self"]["DNSName"].as_str().unwrap_or_default().trim_end_matches('.').to_lowercase();
    let ips = status["Self"]["TailscaleIPs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from);
    [dns.split('.').next().unwrap_or_default().to_string(), dns.clone()].into_iter().chain(ips).filter(|n| !n.is_empty()).collect()
}

async fn check(api: &LocalApi, addr: &str) -> Result<Option<(String, Vec<String>)>> {
    let whois = api.call(Method::GET, &format!("whois?addr={addr}")).await?;
    let status = api.call(Method::GET, "status").await?;
    Ok(allowed(&status, &whois).then(|| (machine(whois["Node"]["Name"].as_str().unwrap_or_default()), names(&status))))
}

async fn guard(State(api): State<LocalApi>, ConnectInfo(peer): ConnectInfo<Peer>, mut request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let headers = request.headers();
    let host = headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok()).map(|h| colab::split_address(h).0.trim_matches(['[', ']']).to_lowercase());
    let cross_site = headers.contains_key(axum::http::header::ORIGIN);
    let client = peer.client.get_or_init(|| async { check(&api, &peer.addr).await.unwrap_or_else(|e| (log::warn!("Tailscale whois {}: {e:#}", peer.addr), None).1) }).await;
    let Some((client, _)) = client.as_ref().filter(|(_, names)| !cross_site && host.as_ref().is_some_and(|h| names.contains(h))) else {
        log::warn!("rejected Tailscale request from {} for host {host:?}{}", peer.addr, if cross_site { " with an Origin" } else { "" });
        return (axum::http::StatusCode::FORBIDDEN, "forbidden").into_response();
    };
    request.extensions_mut().insert(crate::kernels::Client(client.clone()));
    next.run(request).await
}

pub async fn serve(app: axum::Router, api: LocalApi, conns: mpsc::UnboundedReceiver<(UnixStream, String)>) {
    let app = app.layer(axum::middleware::from_fn_with_state(api, guard));
    drop(axum::serve(Incoming(conns), app.into_make_service_with_connect_info::<Peer>()).await)
}

pub fn router(rpc: Rpc, kernels: Arc<KernelHost>) -> axum::Router {
    protocol::router(rpc).merge(crate::kernels::router(kernels))
}

struct Up {
    node: Node,
    listener: Arc<Listener>,
    tasks: Vec<JoinHandle<()>>,
}

pub struct Tailscale {
    dir: PathBuf,
    events: Events,
    socks: watch::Sender<Option<Socks>>,
    peers: watch::Sender<Option<Netmap>>,
    app: axum::Router,
    pub(crate) kernels: Arc<KernelHost>,
    up: tokio::sync::Mutex<Option<JoinHandle<Option<Up>>>>,
    loopback: Mutex<Option<(LocalApi, Socks)>>,
    status: Mutex<TailscaleStatus>,
}

impl Tailscale {
    pub fn new(dir: PathBuf, events: Events, socks: watch::Sender<Option<Socks>>, peers: watch::Sender<Option<Netmap>>, rpc: Rpc, kernels: Arc<KernelHost>) -> Arc<Self> {
        let hostname = Self::settings(&dir).hostname;
        let status = TailscaleStatus { state: TailscaleState::Off, hostname, dns_name: None, login_name: None, auth_url: None, error: None };
        Arc::new(Self { dir, events, socks, peers, app: router(rpc, kernels.clone()), kernels, up: Default::default(), loopback: Mutex::default(), status: Mutex::new(status) })
    }

    fn settings(dir: &Path) -> Settings {
        std::fs::read_to_string(dir.join("tailscale.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| Settings { enabled: false, hostname: default_hostname() })
    }

    fn save(&self, enabled: bool, hostname: &str) -> Result<()> {
        let (file, tmp) = (self.dir.join("tailscale.json"), self.dir.join("tailscale.json.tmp"));
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(&tmp, serde_json::to_string_pretty(&Settings { enabled, hostname: hostname.into() })? + "\n")?;
        Ok(std::fs::rename(tmp, file)?)
    }

    pub fn status(&self) -> TailscaleStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn proxy(&self) -> Option<Socks> {
        self.socks.borrow().clone()
    }

    fn set(&self, status: TailscaleStatus) {
        let socks = self.loopback.lock().unwrap().as_ref().map(|l| l.1.clone()).filter(|_| status.state == TailscaleState::Running);
        self.socks.send_if_modified(|s| (*s != socks).then(|| *s = socks).is_some());
        let mut current = self.status.lock().unwrap();
        if *current != status {
            *current = status.clone();
            drop(current);
            let _ = self.events.send((None, events::TAILSCALE_STATUS.into(), json!(status)));
        }
    }

    fn with(&self, state: TailscaleState, hostname: &str, error: Option<String>) {
        self.set(TailscaleStatus { state, hostname: hostname.into(), dns_name: None, login_name: None, auth_url: None, error });
    }

    pub async fn boot(self: &Arc<Self>, enable: bool, hostname: Option<String>) -> Result<()> {
        if enable || Self::settings(&self.dir).enabled {
            self.enable(hostname).await?;
        }
        Ok(())
    }

    pub async fn enable(self: &Arc<Self>, hostname: Option<String>) -> Result<TailscaleStatus> {
        let mut up = self.up.lock().await;
        let saved = Self::settings(&self.dir).hostname;
        let hostname = hostname.map(|h| h.trim().to_lowercase()).filter(|h| !h.is_empty()).unwrap_or(saved.clone());
        if up.is_some() && hostname == saved && self.status().state != TailscaleState::Error {
            return Ok(self.status());
        }
        self.stop(&mut up).await;
        self.save(true, &hostname)?;
        self.with(TailscaleState::Starting, &hostname, None);
        let me = self.clone();
        *up = Some(tokio::spawn(async move { me.start(hostname).await }));
        Ok(self.status())
    }

    pub async fn disable(&self) -> Result<TailscaleStatus> {
        let mut up = self.up.lock().await;
        self.stop(&mut up).await;
        let hostname = Self::settings(&self.dir).hostname;
        self.save(false, &hostname)?;
        self.with(TailscaleState::Off, &hostname, None);
        Ok(self.status())
    }

    pub async fn logout(&self) -> Result<TailscaleStatus> {
        let api = self.loopback.lock().unwrap().as_ref().map(|l| l.0.clone()).context("Tailscale is off")?;
        api.call(Method::POST, "logout").await?;
        self.disable().await
    }

    async fn stop(&self, up: &mut Option<JoinHandle<Option<Up>>>) {
        let Some(handle) = up.take() else { return };
        let Ok(Some(up)) = handle.await else { return };
        for task in up.tasks {
            task.abort();
            let _ = task.await;
        }
        *self.loopback.lock().unwrap() = None;
        self.socks.send_replace(None);
        self.peers.send_replace(None);
        let _ = up.listener.0.shutdown(std::net::Shutdown::Both);
        let _ = tokio::task::spawn_blocking(move || drop(up.node)).await;
    }

    async fn start(self: Arc<Self>, hostname: String) -> Option<Up> {
        let (dir, name) = (self.dir.join("tailscale"), hostname.clone());
        let started = tokio::task::spawn_blocking(move || -> Result<_> {
            let node = Node::start(&dir, &name)?;
            let loopback = node.loopback()?;
            let listener = node.listen(":80")?;
            Ok((node, loopback, listener))
        }).await;
        let (node, (addr, proxy, cred), listener) = match started.map_err(anyhow::Error::from).and_then(|r| r) {
            Ok(started) => started,
            Err(e) => {
                log::error!("starting Tailscale: {e:#}");
                self.with(TailscaleState::Error, &hostname, Some(format!("{e:#}")));
                return None;
            }
        };
        let api = LocalApi::new(format!("http://{addr}"), cred);
        *self.loopback.lock().unwrap() = Some((api.clone(), Socks { addr, user: "tsnet".into(), password: proxy }));
        let (listener, (tx, rx)) = (Arc::new(listener), mpsc::unbounded_channel());
        let accepting = listener.clone();
        std::thread::spawn(move || loop {
            match accepting.accept() {
                Ok(conn) => if tx.send(conn).is_err() { break },
                Err(libc::EBADF) => break,
                Err(code) => {
                    log::warn!("accepting a Tailscale connection failed with {code}, retrying");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        let serve = tokio::spawn(serve(self.app.clone(), api.clone(), rx));
        let me = self.clone();
        let poll = tokio::spawn(async move {
            let mut since = None;
            loop {
                me.set(match api.call(Method::GET, "status").await {
                    Ok(status) => {
                        if status["BackendState"] == "Running" {
                            me.peers.send_replace(Some(Netmap { since: *since.get_or_insert_with(Instant::now), status: status.clone(), api: api.clone() }));
                        }
                        status_from(&hostname, &status)
                    }
                    Err(e) => TailscaleStatus { state: TailscaleState::Error, hostname: hostname.clone(), dns_name: None, login_name: None, auth_url: None, error: Some(format!("{e:#}")) },
                });
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        log::info!("Tailscale started as {}, serving on port 80", self.status().hostname);
        Some(Up { node, listener, tasks: vec![serve, poll] })
    }
}
