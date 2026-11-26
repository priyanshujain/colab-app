use kernel::Content;
use nb::{Extra, Output};
use serde_json::Value;

fn obj(v: Value) -> Extra {
    match v {
        Value::Object(m) => m,
        _ => Extra::new(),
    }
}

fn squash_cr(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let (body, nl) = line.strip_suffix('\n').map_or((line, ""), |b| (b, "\n"));
            let trimmed = body.trim_end_matches('\r');
            format!("{}{}{nl}", trimmed.rsplit('\r').next().unwrap_or(""), &body[trimmed.len()..])
        })
        .collect()
}

/// Applies one kernel message to a cell's outputs. Returns true when the outputs were cleared.
pub fn apply(outputs: &mut Vec<Output>, clear_wait: &mut bool, content: Content) -> bool {
    let new = match content {
        Content::ClearOutput { wait: true } => {
            *clear_wait = true;
            return false;
        }
        Content::ClearOutput { wait: false } => {
            *clear_wait = false;
            outputs.clear();
            return true;
        }
        Content::Stream { name, text } => Output::Stream { name, text },
        Content::DisplayData { data, metadata, .. } => Output::DisplayData { data: obj(data), metadata: obj(metadata), transient: None },
        Content::ExecuteResult { execution_count, data, metadata } => {
            Output::ExecuteResult { execution_count: Some(execution_count), data: obj(data), metadata: obj(metadata) }
        }
        Content::Error { ename, evalue, traceback } => Output::Error { ename, evalue, traceback },
        _ => return false,
    };
    let cleared = std::mem::take(clear_wait);
    if cleared {
        outputs.clear();
    }
    match (outputs.last_mut(), new) {
        (Some(Output::Stream { name, text }), Output::Stream { name: new_name, text: new_text }) if *name == new_name => {
            let start = text.rfind('\n').map_or(0, |i| i + 1);
            text.push_str(&new_text);
            let tail = squash_cr(&text[start..]);
            text.truncate(start);
            text.push_str(&tail);
        }
        (_, Output::Stream { name, text }) => outputs.push(Output::Stream { name, text: squash_cr(&text) }),
        (_, new) => outputs.push(new),
    }
    cleared
}

pub fn update(output: &mut Output, new_data: Value, new_metadata: Value) {
    if let Output::DisplayData { data, metadata, .. } | Output::ExecuteResult { data, metadata, .. } = output {
        *data = obj(new_data);
        *metadata = obj(new_metadata);
    }
}

fn offset(text: &str, pos: &Value) -> usize {
    let line = pos["line"].as_u64().unwrap_or(0) as usize;
    let character = pos["character"].as_u64().unwrap_or(0) as usize;
    let start: usize = text.split_inclusive('\n').take(line).map(str::len).sum();
    let mut units = 0;
    for (i, c) in text[start..].char_indices() {
        if units >= character || c == '\n' {
            return start + i;
        }
        units += c.len_utf16();
    }
    text.len()
}

pub fn apply_text_edits(text: &str, edits: &Value) -> String {
    let mut ranges: Vec<(usize, usize, &str)> = edits
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| (offset(text, &e["range"]["start"]), offset(text, &e["range"]["end"]), e["newText"].as_str().unwrap_or("")))
        .collect();
    ranges.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = text.to_string();
    for (start, end, new) in ranges {
        out.replace_range(start..end.max(start), new);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stream(text: &str) -> Content {
        Content::Stream { name: "stdout".into(), text: text.into() }
    }

    fn texts(outputs: &[Output]) -> Vec<String> {
        outputs.iter().map(|o| serde_json::to_value(o).unwrap()["text"].as_str().unwrap_or("").to_string()).collect()
    }

    #[test]
    fn tqdm_style_carriage_returns_and_clear_output_wait() {
        let mut outputs = Vec::new();
        let mut wait = false;
        apply(&mut outputs, &mut wait, stream("Epoch 1\n"));
        for pct in [0, 50, 100] {
            apply(&mut outputs, &mut wait, Content::Stream { name: "stderr".into(), text: format!("\r{pct:3}%|##| {pct}/100") });
        }
        apply(&mut outputs, &mut wait, Content::Stream { name: "stderr".into(), text: "\n".into() });
        apply(&mut outputs, &mut wait, stream("done\r\n"));
        assert_eq!(texts(&outputs), ["Epoch 1\n", "100%|##| 100/100\n", "done\r\n"]);

        assert!(!apply(&mut outputs, &mut wait, Content::ClearOutput { wait: true }));
        assert_eq!(outputs.len(), 3);
        assert!(apply(&mut outputs, &mut wait, Content::DisplayData { data: json!({"text/plain": "frame 2"}), metadata: json!({}), transient: json!({}) }));
        assert_eq!(outputs.len(), 1);
        apply(&mut outputs, &mut wait, Content::ExecuteResult { execution_count: 3, data: json!({"text/plain": "42"}), metadata: json!({}) });
        update(&mut outputs[0], json!({"text/plain": "frame 3"}), json!({}));
        let saved = serde_json::to_value(&outputs).unwrap();
        assert_eq!(saved[0]["data"]["text/plain"], "frame 3");
        assert_eq!(saved[1]["execution_count"], 3);
        assert!(saved[0].get("transient").is_none());
    }

    #[test]
    fn ruff_edits_apply_with_utf16_positions() {
        let text = "x=1\ns = 'héllo 😀'  ;  y=2\n";
        let edits = json!([
            {"range": {"start": {"line": 1, "character": 14}, "end": {"line": 1, "character": 22}}, "newText": "\ny = 2"},
            {"range": {"start": {"line": 0, "character": 1}, "end": {"line": 0, "character": 2}}, "newText": " = "},
        ]);
        assert_eq!(apply_text_edits(text, &edits), "x = 1\ns = 'héllo 😀'\ny = 2\n");
    }
}
