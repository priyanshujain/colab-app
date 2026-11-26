use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

fn load<T: DeserializeOwned>(path: &Path) -> Result<BTreeMap<String, T>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

fn write<T: Serialize>(path: &Path, map: &BTreeMap<String, T>) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(map)?)?;
    Ok(std::fs::rename(tmp, path)?)
}

fn key(notebook: &Path) -> String {
    std::path::absolute(notebook).unwrap_or(notebook.into()).to_string_lossy().into()
}

pub fn get<T: DeserializeOwned>(path: &Path, notebook: &Path) -> Result<Option<T>> {
    Ok(load(path)?.remove(&key(notebook)))
}

pub fn save<T: Serialize>(path: &Path, notebook: &Path, saved: &T) -> Result<()> {
    let mut map = load::<Value>(path)?;
    map.insert(key(notebook), serde_json::to_value(saved)?);
    write(path, &map)
}

pub fn remove(path: &Path, notebook: &Path) -> Result<()> {
    let mut map = load::<Value>(path)?;
    if map.remove(&key(notebook)).is_some() {
        write(path, &map)?;
    }
    Ok(())
}

pub fn rename(path: &Path, from: &Path, to: &Path) -> Result<()> {
    let mut map = load::<Value>(path)?;
    let Some(saved) = map.remove(&key(from)) else { return Ok(()) };
    map.insert(key(to), saved);
    write(path, &map)
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
        assert_eq!(get::<Saved>(&path, old).unwrap(), Some(Saved { endpoint: "m-s-old".into(), kernel_id: "k-old".into(), assigned: None }));
        assert_eq!(get::<Saved>(&path, new).unwrap().unwrap().assigned, assigned);
        std::fs::remove_file(path).unwrap();
    }
}
