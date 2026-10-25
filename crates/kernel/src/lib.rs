use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Header {
    pub msg_id: String,
    pub session: String,
    pub username: String,
    pub date: DateTime<Utc>,
    pub msg_type: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Message {
    pub header: Header,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub parent_header: Option<Header>,
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub metadata: Map<String, Value>,
    #[ts(type = "Record<string, unknown>")]
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub channel: Option<Channel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(skip)]
    pub buffers: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Channel {
    Shell,
    Iopub,
    Stdin,
    Control,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "msg_type", content = "content", rename_all = "snake_case")]
#[ts(export)]
pub enum Content {
    ExecuteRequest { code: String, silent: bool, store_history: bool, allow_stdin: bool, stop_on_error: bool },
    ExecuteReply { status: String, execution_count: Option<u32> },
    ExecuteInput { code: String, execution_count: u32 },
    Stream { name: String, text: String },
    DisplayData { #[ts(type = "Record<string, unknown>")] data: Value, #[ts(type = "Record<string, unknown>")] metadata: Value, #[serde(default)] #[ts(type = "Record<string, unknown>")] transient: Value },
    UpdateDisplayData { #[ts(type = "Record<string, unknown>")] data: Value, #[ts(type = "Record<string, unknown>")] metadata: Value, #[ts(type = "Record<string, unknown>")] transient: Value },
    ExecuteResult { execution_count: u32, #[ts(type = "Record<string, unknown>")] data: Value, #[ts(type = "Record<string, unknown>")] metadata: Value },
    Error { ename: String, evalue: String, traceback: Vec<String> },
    Status { execution_state: ExecutionState },
    ClearOutput { wait: bool },
    CommOpen { comm_id: String, target_name: String, #[ts(type = "unknown")] data: Value },
    CommMsg { comm_id: String, #[ts(type = "unknown")] data: Value },
    CommClose { comm_id: String, #[serde(default)] #[ts(type = "unknown")] data: Value },
    InputRequest { prompt: String, password: bool },
    InputReply { value: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ExecutionState {
    Starting,
    Busy,
    Idle,
    Restarting,
    Dead,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum KernelStatus {
    Connecting,
    Idle,
    Busy,
    Reconnecting,
    Disconnected,
    Dead,
}

impl Message {
    pub fn new(msg_type: &str, session: &str, content: Value) -> Self {
        Self {
            header: Header {
                msg_id: uuid::Uuid::new_v4().to_string(),
                session: session.into(),
                username: "colab-app".into(),
                date: Utc::now(),
                msg_type: msg_type.into(),
                version: "5.3".into(),
            },
            parent_header: None,
            metadata: Map::new(),
            content,
            channel: None,
            buffers: Vec::new(),
        }
    }

    pub fn execute_request(session: &str, code: &str) -> Self {
        let content = json!({"code": code, "silent": false, "store_history": true,
            "user_expressions": {}, "allow_stdin": true, "stop_on_error": true});
        let mut msg = Self::new("execute_request", session, content);
        msg.channel = Some(Channel::Shell);
        msg
    }

    pub fn typed(&self) -> Content {
        serde_json::from_value(json!({"msg_type": self.header.msg_type, "content": self.content}))
            .unwrap_or(Content::Other)
    }

    pub fn parent_id(&self) -> Option<&str> {
        self.parent_header.as_ref().map(|h| h.msg_id.as_str())
    }
}

fn empty_as_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Header>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

/// Messages from one execution: iopub and stdin messages whose parent is the request, ending after `status: idle`.
pub type Execution = BoxStream<'static, Message>;

#[async_trait]
pub trait Kernel: Send + Sync {
    async fn execute(&self, code: &str) -> anyhow::Result<Execution>;
    async fn send(&self, msg: Message) -> anyhow::Result<()>;
    async fn interrupt(&self) -> anyhow::Result<()>;
    async fn restart(&self) -> anyhow::Result<()>;
    async fn shutdown(&self) -> anyhow::Result<()>;
    fn status(&self) -> KernelStatus;
    /// Messages not tied to an execution, such as comm messages from widgets and status changes.
    fn subscribe(&self) -> BoxStream<'static, Message>;
}
