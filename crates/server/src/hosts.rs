use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Host {
    pub label: String,
    pub address: String,
}

pub fn file() -> std::path::PathBuf {
    crate::data_dir().join("hosts.json")
}

pub fn load(file: &Path) -> Result<Vec<Host>> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

fn save(file: &Path, hosts: &[Host]) -> Result<()> {
    std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(hosts)? + "\n")?;
    Ok(std::fs::rename(tmp, file)?)
}

pub fn add(file: &Path, label: &str, address: &str) -> Result<Vec<Host>> {
    let (label, address) = (label.trim(), address.trim());
    if label.is_empty() || address.is_empty() || label == "local" {
        bail!("a host needs a label other than local and an address");
    }
    let mut hosts = load(file)?;
    if hosts.iter().any(|h| h.label == label) {
        bail!("host {label} already exists");
    }
    hosts.push(Host { label: label.into(), address: address.into() });
    save(file, &hosts)?;
    Ok(hosts)
}

pub fn remove(file: &Path, label: &str) -> Result<Vec<Host>> {
    let mut hosts = load(file)?;
    let before = hosts.len();
    hosts.retain(|h| h.label != label);
    if hosts.len() == before {
        bail!("no host named {label}");
    }
    save(file, &hosts)?;
    Ok(hosts)
}

pub fn cli(args: &[String]) -> Result<()> {
    let file = file();
    let hosts = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["add", label, address] => add(&file, label, address)?,
        ["remove", label] => remove(&file, label)?,
        ["list"] => load(&file)?,
        _ => bail!("usage: colab-server hosts add <label> <address> | list | remove <label>"),
    };
    hosts.iter().for_each(|h| println!("{}\t{}", h.label, h.address));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_added_listed_and_removed_by_label_in_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data/hosts.json");
        assert!(load(&file).unwrap().is_empty());
        add(&file, "anton", "anton").unwrap();
        add(&file, " gpu ", "100.68.126.75 ").unwrap();
        assert!(add(&file, "anton", "other").unwrap_err().to_string().contains("already exists"));
        assert!(add(&file, "local", "mac-work").is_err());
        let text: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(text, serde_json::json!([{"label": "anton", "address": "anton"}, {"label": "gpu", "address": "100.68.126.75"}]));
        assert_eq!(remove(&file, "anton").unwrap(), [Host { label: "gpu".into(), address: "100.68.126.75".into() }]);
        assert!(remove(&file, "anton").is_err());
        assert_eq!(load(&file).unwrap().len(), 1);
    }
}
