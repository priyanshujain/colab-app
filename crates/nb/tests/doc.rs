use nb::{CellType, Notebook, NotebookDoc};
use std::sync::mpsc::channel;
use std::time::Duration;

const V44: &str = r##"{
 "cells": [
  {"cell_type": "markdown", "metadata": {}, "source": ["# Word vectors\n", "Intro"]},
  {"cell_type": "code", "execution_count": null, "metadata": {"scrolled": true}, "outputs": [{"name": "stdout", "output_type": "stream", "text": ["epoch 1\n", "epoch 2\n"]}], "source": ["for i in range(2):\n", "    print(f'epoch {i+1}')"]}
 ],
 "metadata": {"kernelspec": {"display_name": "Python 3", "language": "python", "name": "python3"}},
 "nbformat": 4,
 "nbformat_minor": 4
}"##;

#[test]
fn notebook_without_cell_ids_gets_stable_ids_and_round_trips() {
    let nb = Notebook::parse(V44).unwrap();
    assert_eq!(nb.nbformat_minor, 5);
    assert!(nb.cells.iter().all(|c| c.id.len() == 8));
    assert_ne!(nb.cells[0].id, nb.cells[1].id);
    assert_eq!(nb.cells[1].execution_count, Some(None));
    assert_eq!(nb.cells[1].source, "for i in range(2):\n    print(f'epoch {i+1}')");
    let saved = nb.to_json().unwrap();
    assert_eq!(Notebook::parse(&saved).unwrap(), nb);
    assert_eq!(Notebook::parse(&saved).unwrap().to_json().unwrap(), saved);
}

#[test]
fn external_edit_merges_per_cell_and_own_writes_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("words.ipynb");
    std::fs::write(&path, Notebook::parse(V44).unwrap().to_json().unwrap()).unwrap();
    let (tx, rx) = channel();
    let doc = NotebookDoc::open(&path, move |nb| tx.send(nb).unwrap()).unwrap();
    let [md, code] = [0, 1].map(|i| doc.notebook().cells[i].id.clone());

    doc.set_active_cell(Some(code.clone()));
    doc.edit(|nb| nb.cells[1].source = "print('typing')".into());
    std::thread::sleep(Duration::from_millis(300));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("typing"), "autosave must wait for the debounce");
    std::thread::sleep(Duration::from_millis(1200));
    assert!(std::fs::read_to_string(&path).unwrap().contains("typing"));
    assert!(rx.recv_timeout(Duration::from_millis(700)).is_err(), "own write reported as external");

    let mut disk = Notebook::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    disk.cells[0].source = "# Edited by Claude".into();
    disk.cells[1].source = "print('from git')".into();
    disk.cells.push(nb::Cell::new(CellType::Code, "import numpy"));
    std::fs::write(&path, disk.to_json().unwrap()).unwrap();

    let seen = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(seen.cell(&md).unwrap().source, "# Edited by Claude");
    assert_eq!(seen.cell(&code).unwrap().source, "print('typing')");
    assert_eq!(seen.cells[2].source, "import numpy");
    drop(doc);
    assert_eq!(Notebook::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(), seen);
}

#[test]
fn streaming_outputs_autosave_while_running_and_survive_an_external_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("train.ipynb");
    std::fs::write(&path, Notebook::parse(V44).unwrap().to_json().unwrap()).unwrap();
    let (tx, rx) = channel();
    let doc = NotebookDoc::open(&path, move |nb| tx.send(nb).unwrap()).unwrap();
    let stale = std::fs::read_to_string(&path).unwrap();
    let code = doc.notebook().cells[1].id.clone();
    doc.set_running(&code, true);
    let stream = |text: String| nb::Output::Stream { name: "stdout".into(), text };
    doc.edit(|nb| nb.cells[1].outputs = Some(vec![stream(String::new())]));
    for i in 0..25 {
        doc.edit(|nb| if let Some(nb::Output::Stream { text, .. }) = nb.cells[1].outputs.as_mut().unwrap().last_mut() { text.push_str(&format!("step {i}\n")) });
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(std::fs::read_to_string(&path).unwrap().contains("step 10"), "continuous output never autosaved");

    let mut disk = Notebook::parse(&stale).unwrap();
    disk.cells[0].source = "# Edited by Claude".into();
    disk.cells[1].source = "for i in range(25):\n    print(f'step {i}')".into();
    std::fs::write(&path, disk.to_json().unwrap()).unwrap();
    let seen = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(seen.cells[0].source, "# Edited by Claude");
    assert_eq!(seen.cells[1].source, "for i in range(25):\n    print(f'step {i}')");
    let expected: String = (0..25).map(|i| format!("step {i}\n")).collect();
    assert_eq!(seen.cells[1].outputs, Some(vec![stream(expected)]));
    doc.set_running(&code, false);
    drop(doc);
    assert_eq!(Notebook::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(), seen);
}
