use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub endpoint: String,
    pub kernel_id: String,
    #[serde(default)]
    pub assigned: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn default_path() -> PathBuf {
    dirs::data_dir().unwrap_or_default().join("colab-app/colab-runtimes.json")
}

fn load(path: &Path) -> Result<BTreeMap<String, Saved>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

fn write(path: &Path, map: &BTreeMap<String, Saved>) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(map)?)?;
    Ok(std::fs::rename(tmp, path)?)
}

fn key(notebook: &Path) -> String {
    std::path::absolute(notebook).unwrap_or(notebook.into()).to_string_lossy().into()
}

pub fn get(path: &Path, notebook: &Path) -> Result<Option<Saved>> {
    Ok(load(path)?.remove(&key(notebook)))
}

pub fn save(path: &Path, notebook: &Path, saved: &Saved) -> Result<()> {
    let mut map = load(path)?;
    map.insert(key(notebook), saved.clone());
    write(path, &map)
}

pub fn remove(path: &Path, notebook: &Path) -> Result<()> {
    let mut map = load(path)?;
    if map.remove(&key(notebook)).is_some() {
        write(path, &map)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtimes_saved_before_allocation_time_still_load_and_new_ones_keep_it() {
        let path = std::env::temp_dir().join(format!("colab-runtimes-{}.json", uuid::Uuid::new_v4()));
        let (old, new) = (Path::new("/work/old.ipynb"), Path::new("/work/new.ipynb"));
        std::fs::write(&path, format!(r#"{{"{}": {{"endpoint": "m-s-old", "kernel_id": "k-old"}}}}"#, key(old))).unwrap();
        let assigned = Some("2026-11-13T08:11:32Z".parse().unwrap());
        save(&path, new, &Saved { endpoint: "m-s-new".into(), kernel_id: "k-new".into(), assigned }).unwrap();
        assert_eq!(get(&path, old).unwrap(), Some(Saved { endpoint: "m-s-old".into(), kernel_id: "k-old".into(), assigned: None }));
        assert_eq!(get(&path, new).unwrap().unwrap().assigned, assigned);
        std::fs::remove_file(path).unwrap();
    }
}
