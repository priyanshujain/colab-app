use anyhow::{Result, bail};
use colab::keystore::Entry;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;

pub struct Secrets {
    entry: Entry,
    lock: Mutex<()>,
}

pub fn valid_name(name: &str) -> Result<()> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        bail!("secret name cannot be empty or contain whitespace: {name:?}");
    }
    Ok(())
}

impl Secrets {
    pub fn new(service: &str) -> Self {
        Self { entry: Entry::new(service, "secrets", "secrets.json"), lock: Mutex::new(()) }
    }

    fn values(&self) -> Result<BTreeMap<String, String>> {
        Ok(self.entry.get()?.map(|t| serde_json::from_str(&t)).transpose()?.unwrap_or_default())
    }

    fn update(&self, f: impl FnOnce(&mut BTreeMap<String, String>)) -> Result<()> {
        let _l = self.lock.lock().unwrap();
        let mut values = self.values()?;
        f(&mut values);
        self.entry.set(&serde_json::to_string(&values)?)
    }

    pub fn list(&self) -> Result<BTreeMap<String, String>> {
        let _l = self.lock.lock().unwrap();
        self.values()
    }

    pub fn set(&self, name: &str, value: &str) -> Result<()> {
        valid_name(name)?;
        self.update(|v| drop(v.insert(name.into(), value.into())))
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        self.update(|v| drop(v.remove(name)))
    }

    pub fn lookup(&self, name: &str) -> Result<Value> {
        Ok(match self.list()?.remove(name) {
            Some(value) => json!({"exists": true, "access": true, "payload": value}),
            None => json!({"exists": false}),
        })
    }
}
