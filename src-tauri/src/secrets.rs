use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;

pub struct Secrets {
    service: String,
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
        Self { service: service.into(), lock: Mutex::new(()) }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, "secrets")?)
    }

    fn values(&self) -> Result<BTreeMap<String, String>> {
        match self.entry()?.get_password() {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(keyring::Error::NoEntry) => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn update(&self, f: impl FnOnce(&mut BTreeMap<String, String>)) -> Result<()> {
        let _l = self.lock.lock().unwrap();
        let mut values = self.values()?;
        f(&mut values);
        Ok(self.entry()?.set_password(&serde_json::to_string(&values)?)?)
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
