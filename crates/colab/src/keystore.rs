use anyhow::Result;

pub struct Entry {
    service: String,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    user: String,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    file: String,
}

impl Entry {
    pub fn new(service: &str, user: &str, file: &str) -> Self {
        Self { service: service.into(), user: user.into(), file: file.into() }
    }
}

#[cfg(target_os = "macos")]
impl Entry {
    fn keychain(&self) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, &self.user)?)
    }

    pub fn get(&self) -> Result<Option<String>> {
        match self.keychain()?.get_password() {
            Ok(text) => Ok(Some(text)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set(&self, value: &str) -> Result<()> {
        if self.keychain()?.set_password(value).is_err() {
            self.delete()?;
            self.keychain()?.set_password(value)?;
        }
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        match self.keychain()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn data_dir(service: &str) -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share")).join(service)
}

#[cfg(not(target_os = "macos"))]
impl Entry {
    fn path(&self) -> std::path::PathBuf {
        data_dir(&self.service).join(&self.file)
    }

    pub fn get(&self) -> Result<Option<String>> {
        match std::fs::read_to_string(self.path()) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set(&self, value: &str) -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let path = self.path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?.write_all(value.as_bytes())?;
        Ok(std::fs::rename(tmp, path)?)
    }

    pub fn delete(&self) -> Result<()> {
        match std::fs::remove_file(self.path()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}
