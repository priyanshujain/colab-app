pub mod bridge;
pub mod engine;
pub mod hosts;
pub mod ipc;
pub mod kernels;
pub mod mcp;
mod outputs;
pub mod protocol;
pub mod secrets;
pub mod tailscale;

use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_default();
    if cfg!(target_os = "macos") { base.join("dev.colab-app") } else { base.join("colab-app") }
}

pub fn socket_path() -> PathBuf {
    let dir = if cfg!(target_os = "macos") { dirs::data_dir().unwrap_or_default().join("colab-app/run") } else { dirs::runtime_dir().map_or_else(|| data_dir().join("run"), |d| d.join("colab-app")) };
    dir.join("server.sock")
}

pub struct Config {
    pub socket: PathBuf,
    pub data_dir: PathBuf,
    pub mcp_port: u16,
    pub secrets_service: String,
    pub tailscale: bool,
    pub tailscale_hostname: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { socket: socket_path(), data_dir: data_dir(), mcp_port: mcp::PORT, secrets_service: "colab-app".into(), tailscale: false, tailscale_hostname: None }
    }
}

pub fn spawn(args: &[&str]) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("server.log"))?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args(args).stdin(std::process::Stdio::null()).stdout(log.try_clone()?).stderr(log);
    unsafe { command.pre_exec(|| if libc::setsid() < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }) };
    command.spawn().map(drop)
}

pub async fn connect(socket: &Path) -> Result<WebSocketStream<UnixStream>> {
    let stream = UnixStream::connect(socket).await?;
    Ok(tokio_tungstenite::client_async("ws://localhost/rpc", stream).await?.0)
}

pub async fn run(config: Config) -> Result<()> {
    let socket = &config.socket;
    if UnixStream::connect(socket).await.is_ok() {
        bail!("colab-server is already running on {}", socket.display());
    }
    let dir = socket.parent().context("socket path has no parent")?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let tcp = bind(config.mcp_port).await.with_context(|| format!("binding 127.0.0.1:{} for MCP", config.mcp_port))?;
    let _ = std::fs::remove_file(socket);
    let unix = tokio::net::UnixListener::bind(socket).with_context(|| format!("binding {}", socket.display()))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let events = tokio::sync::broadcast::channel(4096).0;
    let sender = events.clone();
    let emit: engine::Emit = Arc::new(move |event, payload| drop(sender.send((protocol::ORIGIN.try_with(|o| *o).ok(), event.into(), payload))));
    let core = engine::Core::with_secrets(emit, config.data_dir.clone(), &config.secrets_service);
    std::sync::LazyLock::force(&kernels::TYPES);
    tailscale::machine_name(&config.data_dir);
    let tools = core.clone();
    tokio::spawn(async move { tools.install_tools().await });
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let hosted = kernels::KernelHost::new(core.clone(), dirs::home_dir().context("no home directory")?);
    let remote = protocol::Rpc { core: core.clone(), events: events.clone(), shutdown: shutdown.clone(), tailscale: None };
    let tailscale = tailscale::Tailscale::new(config.data_dir.clone(), events, core.socks.clone(), core.peers.clone(), remote.clone(), hosted);
    let rpc = protocol::Rpc { tailscale: Some(tailscale.clone()), ..remote };
    let (enable, hostname) = (config.tailscale, config.tailscale_hostname.clone());
    tokio::spawn(async move { tailscale.boot(enable, hostname).await.map_err(|e| log::error!("Tailscale: {e:#}")) });
    log::info!("colab-server {} listening on {} and 127.0.0.1:{}", env!("CARGO_PKG_VERSION"), socket.display(), config.mcp_port);
    let result = tokio::select! {
        r = axum::serve(unix, protocol::router(rpc)).into_future() => r.context("rpc listener"),
        r = axum::serve(tcp, mcp::router(core)).into_future() => r.context("mcp listener"),
        () = shutdown.notified() => Ok(()),
    };
    let _ = std::fs::remove_file(socket);
    log::info!("colab-server stopped");
    result
}

async fn bind(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && std::time::Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(100)).await,
            result => return result,
        }
    }
}

pub async fn call<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(ws: &mut WebSocketStream<S>, method: &str, params: Value) -> Result<Value> {
    ws.send(Message::Text(json!({"id": 0, "method": method, "params": params}).to_string().into())).await?;
    while let Some(message) = ws.next().await {
        let Message::Text(text) = message? else { continue };
        let reply: Value = serde_json::from_str(&text)?;
        if reply.get("id").is_none() {
            continue;
        }
        if let Some(error) = reply.get("error") {
            bail!("{}", error.as_str().unwrap_or_default());
        }
        return Ok(reply["result"].clone());
    }
    bail!("colab-server closed the connection")
}

async fn tailscale_cli(args: &[String]) -> Result<()> {
    use ipc::{TailscaleState, TailscaleStatus};
    let (method, params) = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["on"] => ("tailscale_enable", json!({})),
        ["on", "--hostname", hostname] => ("tailscale_enable", json!({"hostname": hostname})),
        ["off"] => ("tailscale_disable", Value::Null),
        ["logout"] => ("tailscale_logout", Value::Null),
        ["status"] => ("tailscale_status", Value::Null),
        _ => bail!("usage: colab-server tailscale on [--hostname <name>] | off | logout | status"),
    };
    let mut ws = connect(&socket_path()).await.context("colab-server is not running")?;
    let mut status: TailscaleStatus = serde_json::from_value(call(&mut ws, method, params).await?)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while method == "tailscale_enable" && (status.state == TailscaleState::Starting || status.state == TailscaleState::NeedsLogin && status.auth_url.is_none()) && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(500)).await;
        status = serde_json::from_value(call(&mut ws, "tailscale_status", Value::Null).await?)?;
    }
    let name = status.dns_name.as_deref().unwrap_or(&status.hostname);
    match status.state {
        TailscaleState::Off => println!("Tailscale is off"),
        TailscaleState::Starting => println!("Tailscale is starting as {name}"),
        TailscaleState::NeedsLogin => println!("Sign in to Tailscale as {name}: {}", status.auth_url.as_deref().unwrap_or("no sign-in link yet")),
        TailscaleState::Running => println!("Tailscale is running as {name} for {}", status.login_name.as_deref().unwrap_or("an unknown user")),
        TailscaleState::Error => bail!("Tailscale failed: {}", status.error.unwrap_or_default()),
    }
    Ok(())
}

async fn stop(socket: &Path) -> Result<()> {
    let mut ws = connect(socket).await.context("colab-server is not running")?;
    call(&mut ws, "shutdown", Value::Null).await?;
    for _ in 0..100 {
        if UnixStream::connect(socket).await.is_err() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("colab-server did not stop within 10 seconds")
}

fn log_to_stderr() {
    let _ = fern::Dispatch::new()
        .format(|out, message, record| out.finish(format_args!("{} {} {}: {message}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), record.level(), record.target())))
        .level(log::LevelFilter::Info)
        .level_for("rmcp", log::LevelFilter::Warn)
        .level_for("tracing", log::LevelFilter::Warn)
        .chain(std::io::stderr())
        .apply();
}

async fn terminated() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

pub fn cli(args: &[String]) -> i32 {
    let command = args.first().map(String::as_str);
    if command == Some("mcp") {
        bridge::run(&["run"]);
        return 0;
    }
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let result = match command {
        Some("run") => {
            let mut config = Config::default();
            let mut flags = args[1..].iter().map(String::as_str);
            while let Some(flag) = flags.next() {
                match (flag, flags.clone().next()) {
                    ("--tailscale", _) => config.tailscale = true,
                    ("--tailscale-hostname", Some(name)) => config.tailscale_hostname = flags.next().map(|_| name.to_string()),
                    _ => {
                        eprintln!("usage: colab-server run [--tailscale] [--tailscale-hostname <name>]");
                        return 2;
                    }
                }
            }
            log_to_stderr();
            runtime.block_on(async {
                tokio::select! {
                    r = run(config) => r,
                    () = terminated() => Ok(()),
                }
            })
        }
        Some("stop") => runtime.block_on(stop(&socket_path())),
        Some("hosts") => hosts::cli(&args[1..]),
        Some("tailscale") => runtime.block_on(tailscale_cli(&args[1..])),
        _ => {
            eprintln!("usage: colab-server run [--tailscale] [--tailscale-hostname <name>] | stop | mcp | hosts | tailscale");
            return 2;
        }
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("colab-server: {e:#}");
            1
        }
    }
}
