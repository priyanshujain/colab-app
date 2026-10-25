use kernel::KernelStatus;
use nb::{CellType, Notebook, Output};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OpenedNotebook {
    pub path: String,
    pub notebook: Notebook,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum CellEdit {
    Source { cell_id: String, source: String },
    Insert { cell_id: String, index: u32, cell_type: CellType },
    Delete { cell_id: String },
    Move { cell_id: String, index: u32 },
    ChangeType { cell_id: String, cell_type: CellType },
    Focus { cell_id: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum KernelChoice {
    Local,
    Colab { variant: Variant, accelerator: Option<String>, high_ram: bool },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "UPPERCASE")]
#[ts(export)]
pub enum Variant {
    Default,
    Gpu,
    Tpu,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ColabInfo {
    pub eligible_gpus: Vec<String>,
    pub eligible_tpus: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Runtime {
    pub endpoint: String,
    pub accelerator: String,
    pub variant: Variant,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum AuthState {
    SignedOut,
    SignedIn { email: String },
    ClientRejected { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CellOutputs {
    pub path: String,
    pub cell_id: String,
    pub outputs: Vec<Output>,
    pub execution_count: Option<u32>,
    #[ts(type = "Record<string, string> | null")]
    pub execution: Option<serde_json::Value>,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KernelStatusEvent {
    pub path: String,
    pub status: KernelStatus,
    pub choice: Option<KernelChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotebookChanged {
    pub path: String,
    pub notebook: Notebook,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CommMessage {
    pub path: String,
    pub comm_id: String,
    pub msg_type: String,
    #[serde(default)]
    pub target_name: Option<String>,
    #[serde(default)]
    pub buffers: Vec<String>,
    #[ts(type = "unknown")]
    pub data: serde_json::Value,
    #[ts(type = "Record<string, unknown>")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SaveState {
    pub path: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentFocus {
    pub path: String,
    pub cell_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentOpen {
    pub path: String,
}

pub mod events {
    pub const AGENT_OPEN: &str = "agent-open";
    pub const AGENT_FOCUS: &str = "agent-focus";
    pub const CELL_OUTPUTS: &str = "cell-outputs";
    pub const KERNEL_STATUS: &str = "kernel-status";
    pub const NOTEBOOK_CHANGED: &str = "notebook-changed";
    pub const AUTH_STATE: &str = "auth-state";
    pub const COMM: &str = "comm";
    pub const SAVE_STATE: &str = "save-state";
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CellPosition {
    pub path: String,
    pub cell_id: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Diagnostics {
    pub path: String,
    pub cell_id: String,
    #[ts(type = "unknown[]")]
    pub diagnostics: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InputRequest {
    pub path: String,
    pub cell_id: String,
    pub prompt: String,
    pub password: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CodeActionItem {
    pub id: u32,
    pub title: String,
}

pub mod lsp_events {
    pub const DIAGNOSTICS: &str = "diagnostics";
    pub const INPUT_REQUEST: &str = "input-request";
}
