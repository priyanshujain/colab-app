use serde_json::Value;
use std::time::Duration;
use tooling::{Project, Tooling, cell_uri};

async fn wait_for(project: &Project, uri: &str, check: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    for _ in 0..100 {
        let d = project.diagnostics(uri);
        if check(&d) {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("diagnostics for {uri} never matched: {:#?}", project.diagnostics(uri));
}

fn has(d: &[Value], code: &str) -> bool {
    d.iter().any(|d| d["code"] == code)
}

#[tokio::test]
async fn notebook_language_features_on_real_servers() {
    let support = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let tooling = Tooling::new(support.path()).await.unwrap();
    let uv = |args: &[&str]| assert!(std::process::Command::new(&tooling.tools().uv).args(args).current_dir(&root).status().unwrap().success());
    uv(&["venv", "-q"]);
    uv(&["pip", "install", "-q", "six==1.17.0"]);

    let project = tooling.project(&root).await.unwrap();
    let nb = root.join("analysis.ipynb");
    let cells = |first: &str| {
        vec![
            ("a".to_string(), first.to_string()),
            ("b".to_string(), "y = helper(x_value)\nx_".to_string()),
            ("c".to_string(), "print(missing)\nz=six.PY3\n".to_string()),
        ]
    };
    let first = "import os\nimport six\nx_value = 1\n\ndef helper(n: int) -> int:\n    return n + 1\n";
    project.sync(&nb, &cells(first)).await.unwrap();
    let (a, b, c) = (cell_uri(&nb, "a"), cell_uri(&nb, "b"), cell_uri(&nb, "c"));

    let completion = project.completion(&b, 1, 2).await.unwrap();
    let items = completion.get("items").unwrap_or(&completion).as_array().unwrap();
    assert!(items.iter().any(|i| i["label"] == "x_value"), "{completion}");

    let hover = project.hover(&b, 0, 5).await.unwrap();
    assert!(hover["contents"].to_string().contains("def helper(n: int) -> int"), "{hover}");

    let def = project.definition(&b, 0, 5).await.unwrap();
    assert_eq!(def[0]["uri"], a.as_str(), "{def}");
    assert_eq!(def[0]["range"]["start"]["line"], 4);

    let lib = project.definition(&c, 1, 3).await.unwrap();
    assert!(lib[0]["uri"].as_str().unwrap().contains("/.venv/lib/"), "{lib}");

    wait_for(&project, &a, |d| has(d, "F401")).await;
    let d = wait_for(&project, &c, |d| has(d, "F821") && has(d, "unresolved-reference")).await;
    let source = |code: &str| d.iter().find(|d| d["code"] == code).unwrap()["source"].clone();
    assert_eq!((source("F821"), source("unresolved-reference")), ("Ruff".into(), "ty".into()));

    let edits = project.format(&c).await.unwrap();
    assert!(edits.to_string().contains("z = six.PY3"), "{edits}");

    project.sync(&nb, &cells("x_value = 1\n")).await.unwrap();
    wait_for(&project, &b, |d| d.iter().any(|d| d["code"] == "unresolved-reference" && d["message"].as_str().unwrap().contains("helper"))).await;
    wait_for(&project, &a, |d| !has(d, "F401")).await;

    tooling.stop(&root).await;
}
