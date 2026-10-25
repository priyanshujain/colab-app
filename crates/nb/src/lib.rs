use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

mod doc;
pub use doc::{NotebookDoc, merge};

pub type Extra = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Notebook {
    pub nbformat: u32,
    pub nbformat_minor: u32,
    #[ts(type = "Record<string, unknown>")]
    pub metadata: Extra,
    pub cells: Vec<Cell>,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum CellType {
    Code,
    Markdown,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Cell {
    pub id: String,
    pub cell_type: CellType,
    pub source: String,
    #[ts(type = "Record<string, unknown>")]
    pub metadata: Extra,
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub execution_count: Option<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub outputs: Option<Vec<Output>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub attachments: Option<Value>,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "output_type", rename_all = "snake_case")]
#[ts(export)]
pub enum Output {
    Stream {
        name: String,
        text: String,
    },
    DisplayData {
        #[ts(type = "Record<string, unknown>")]
        data: Extra,
        #[ts(type = "Record<string, unknown>")]
        metadata: Extra,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "Record<string, unknown>")]
        transient: Option<Value>,
    },
    ExecuteResult {
        execution_count: Option<u32>,
        #[ts(type = "Record<string, unknown>")]
        data: Extra,
        #[ts(type = "Record<string, unknown>")]
        metadata: Extra,
    },
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
    },
}

impl Notebook {
    pub fn parse(json: &str) -> anyhow::Result<Self> {
        let mut value: Value = serde_json::from_str(json)?;
        cells_mut(&mut value).for_each(lines_to_text);
        let mut assigned = false;
        for cell in cells_mut(&mut value).filter_map(Value::as_object_mut) {
            if !cell.contains_key("id") {
                cell.insert("id".into(), new_id().into());
                assigned = true;
            }
        }
        if assigned && value["nbformat_minor"].as_u64().is_some_and(|m| m < 5) {
            value["nbformat_minor"] = 5.into();
        }
        Ok(serde_json::from_value(value)?)
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
        let mut value = serde_json::to_value(self)?;
        cells_mut(&mut value).for_each(text_to_lines);
        value.serialize(&mut ser)?;
        out.push(b'\n');
        Ok(String::from_utf8(out)?)
    }

    pub fn cell(&self, id: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.id == id)
    }
}

impl Cell {
    pub fn new(cell_type: CellType, source: impl Into<String>) -> Self {
        let code = cell_type == CellType::Code;
        Self {
            id: new_id(),
            cell_type,
            source: source.into(),
            metadata: Extra::new(),
            execution_count: code.then_some(None),
            outputs: code.then(Vec::new),
            attachments: None,
            extra: Extra::new(),
        }
    }
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn lines_to_text(cell: &mut Value) {
    let join = |v: &mut Value| {
        if let Value::Array(lines) = v {
            *v = Value::String(lines.iter().filter_map(Value::as_str).collect());
        }
    };
    cell.get_mut("source").map(join);
    for output in cell.get_mut("outputs").and_then(Value::as_array_mut).into_iter().flatten() {
        output.get_mut("text").map(join);
    }
}

fn text_to_lines(cell: &mut Value) {
    let split = |v: &mut Value| {
        if let Value::String(text) = v {
            *v = text.split_inclusive('\n').map(|l| Value::String(l.into())).collect();
        }
    };
    cell.get_mut("source").map(split);
    for output in cell.get_mut("outputs").and_then(Value::as_array_mut).into_iter().flatten() {
        output.get_mut("text").map(split);
    }
}

fn cells_mut(notebook: &mut Value) -> impl Iterator<Item = &mut Value> {
    notebook.get_mut("cells").and_then(Value::as_array_mut).into_iter().flatten()
}
