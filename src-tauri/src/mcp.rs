use crate::engine::Core;
use crate::ipc::{CellEdit, KernelChoice, Variant};
use nb::{Cell, CellType, Output};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;

pub const PORT: u16 = 7761;

#[derive(Clone)]
pub struct Server {
    core: Arc<Core>,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
pub struct NotebookArgs {
    /// Absolute path of the .ipynb notebook
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CellArgs {
    pub path: String,
    pub cell_id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct EditArgs {
    pub path: String,
    pub cell_id: String,
    pub source: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct InsertArgs {
    pub path: String,
    /// Position of the new cell; the cell count or more appends it
    pub index: u32,
    pub source: String,
    #[serde(default)]
    pub markdown: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Local,
    ColabCpu,
    ColabGpu,
    ColabTpu,
}

#[derive(Deserialize, JsonSchema)]
pub struct ConnectArgs {
    pub path: String,
    pub runtime: Runtime,
    /// Colab accelerator such as T4, A100 or V6E1; omit for the default of the runtime type
    pub accelerator: Option<String>,
    #[serde(default)]
    pub high_ram: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct EnvArgs {
    pub path: String,
    /// Environment variables to set, as name to value
    pub env: std::collections::BTreeMap<String, String>,
}

type Reply = Result<String, String>;

fn fail(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tool_router(router = tool_router)]
impl Server {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core, tool_router: Self::tool_router() }
    }

    #[tool(description = "List the notebooks open in the Colab app")]
    async fn list_notebooks(&self) -> String {
        self.core.open_paths().join("\n")
    }

    #[tool(description = "Open a notebook in the app window, creating an empty one if the file does not exist; reattaches its saved Colab runtime if it has one")]
    async fn open_notebook(&self, Parameters(a): Parameters<NotebookArgs>) -> Reply {
        self.core.agent_open(&a.path).await.map_err(fail)
    }

    #[tool(description = "Connect an open notebook to a kernel: a Colab runtime (assigned on your Google account, may take a minute) or the local .venv; no-op when already connected to the same one")]
    async fn connect_kernel(&self, Parameters(a): Parameters<ConnectArgs>) -> Reply {
        let variant = match a.runtime {
            Runtime::Local => None,
            Runtime::ColabCpu => Some(Variant::Default),
            Runtime::ColabGpu => Some(Variant::Gpu),
            Runtime::ColabTpu => Some(Variant::Tpu),
        };
        let choice = match variant {
            None => KernelChoice::Local,
            Some(Variant::Default) => KernelChoice::Colab { variant: Variant::Default, accelerator: None, high_ram: a.high_ram },
            Some(variant) => {
                let accelerator = match a.accelerator {
                    Some(acc) => acc,
                    None => {
                        let info = self.core.colab_info().await.map_err(fail)?;
                        let eligible = if matches!(variant, Variant::Gpu) { info.eligible_gpus } else { info.eligible_tpus };
                        eligible.into_iter().next().ok_or("this Google account has no eligible accelerator of that type")?
                    }
                };
                KernelChoice::Colab { variant, accelerator: Some(accelerator), high_ram: a.high_ram }
            }
        };
        let changed = self.core.agent_connect(&a.path, choice).await.map_err(fail)?;
        let state = self.core.kernel_state(&a.path).await.map_err(fail)?;
        Ok(format!("{} ({})", if changed { "connected" } else { "already connected" }, serde_json::to_string(&state.status).unwrap_or_default().trim_matches('"')))
    }

    #[tool(description = "Release the notebook's Colab runtime so it stops using quota, or shut down its local kernel; variables are lost")]
    async fn release_kernel(&self, Parameters(a): Parameters<NotebookArgs>) -> Reply {
        self.core.release(&a.path).await.map_err(fail)?;
        self.core.detach(&a.path).await.map_err(fail)?;
        Ok("released".into())
    }

    #[tool(description = "Kernel status and runtime of an open notebook")]
    async fn kernel_status(&self, Parameters(a): Parameters<NotebookArgs>) -> Reply {
        let state = self.core.kernel_state(&a.path).await.map_err(fail)?;
        let live = !matches!(state.status, kernel::KernelStatus::Dead | kernel::KernelStatus::Disconnected);
        Ok(serde_json::json!({"status": state.status, "kernel": state.choice.filter(|_| live)}).to_string())
    }

    #[tool(description = "Read every cell of an open notebook with ids, sources and text outputs")]
    async fn read_notebook(&self, Parameters(a): Parameters<NotebookArgs>) -> Reply {
        let nb = self.core.notebook(&a.path).map_err(fail)?;
        Ok(nb.cells.iter().enumerate().map(|(i, c)| render(i, c)).collect::<Vec<_>>().join("\n\n"))
    }

    #[tool(description = "Replace the source of a cell; the app scrolls to it")]
    async fn edit_cell(&self, Parameters(a): Parameters<EditArgs>) -> Reply {
        self.core.agent_edit(&a.path, CellEdit::Source { cell_id: a.cell_id, source: a.source }).await.map_err(fail)?;
        Ok("edited".into())
    }

    #[tool(description = "Insert a code cell, or a markdown cell when markdown is true, and return its id")]
    async fn insert_cell(&self, Parameters(a): Parameters<InsertArgs>) -> Reply {
        let cell_type = if a.markdown { CellType::Markdown } else { CellType::Code };
        let cell_id = Cell::new(cell_type, "").id;
        self.core.edit_cell(&a.path, CellEdit::Insert { cell_id: cell_id.clone(), index: a.index, cell_type }).await.map_err(fail)?;
        self.core.agent_edit(&a.path, CellEdit::Source { cell_id: cell_id.clone(), source: a.source }).await.map_err(fail)?;
        Ok(cell_id)
    }

    #[tool(description = "Delete a cell")]
    async fn delete_cell(&self, Parameters(a): Parameters<CellArgs>) -> Reply {
        self.core.agent_edit(&a.path, CellEdit::Delete { cell_id: a.cell_id }).await.map_err(fail)?;
        Ok("deleted".into())
    }

    #[tool(description = "Run a code cell on the notebook's kernel (Colab when connected) and wait for its outputs; the app streams them live")]
    async fn run_cell(&self, Parameters(a): Parameters<CellArgs>) -> Reply {
        let cell = self.core.agent_run(&a.path, &a.cell_id).await.map_err(fail)?;
        Ok(outputs(&cell))
    }

    #[tool(description = "Set environment variables in the notebook's kernel (local or Colab, starting the local kernel if needed) so code reads them with os.environ; they last until the kernel restarts. Secrets saved in the app's Settings are already set in every kernel")]
    async fn set_env(&self, Parameters(a): Parameters<EnvArgs>) -> Reply {
        let names = a.env.keys().cloned().collect::<Vec<_>>().join(", ");
        self.core.set_env(&a.path, a.env).await.map_err(fail)?;
        Ok(format!("set {names}"))
    }

    #[tool(description = "Interrupt the running cell of a notebook")]
    async fn interrupt(&self, Parameters(a): Parameters<NotebookArgs>) -> Reply {
        self.core.interrupt(&a.path).await.map_err(fail)?;
        Ok("interrupted".into())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

fn render(index: usize, c: &Cell) -> String {
    let kind = if c.cell_type == CellType::Code { "code" } else { "markdown" };
    let count = c.execution_count.flatten().map_or(String::new(), |n| format!(" [{n}]"));
    let out = outputs(c);
    let out = if out.is_empty() { out } else { format!("\n--- outputs\n{out}") };
    format!("### cell {index} id={} {kind}{count}\n{}{out}", c.id, c.source)
}

fn outputs(c: &Cell) -> String {
    let text = |data: &nb::Extra| {
        let mut parts: Vec<String> = data.keys().filter(|k| *k != "text/plain").map(|k| format!("[{k}]")).collect();
        parts.extend(data.get("text/plain").map(nb_text));
        parts.join("\n")
    };
    c.outputs.iter().flatten().map(|o| match o {
        Output::Stream { text, .. } => text.trim_end().to_string(),
        Output::DisplayData { data, .. } | Output::ExecuteResult { data, .. } => text(data),
        Output::Error { ename, evalue, traceback } => format!("{ename}: {evalue}\n{}", strip_ansi(&traceback.join("\n"))),
    }).collect::<Vec<_>>().join("\n")
}

fn nb_text(v: &serde_json::Value) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| v.as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect())
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            chars.by_ref().find(|c| c.is_ascii_alphabetic());
        } else {
            out.push(c);
        }
    }
    out
}

fn sessions() -> LocalSessionManager {
    let mut manager = LocalSessionManager::default();
    manager.session_config.keep_alive = None;
    manager
}

pub async fn serve(core: Arc<Core>, port: u16) -> anyhow::Result<()> {
    let config = StreamableHttpServerConfig::default().enforce_origin_validation();
    let service: StreamableHttpService<Server, LocalSessionManager> = StreamableHttpService::new(move || Ok(Server::new(core.clone())), Arc::new(sessions()), config);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, axum::Router::new().nest_service("/mcp", service)).await?;
    Ok(())
}
