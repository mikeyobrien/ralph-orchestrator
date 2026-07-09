use crate::utils::now_rfc3339;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrgentSteerRecord {
    pub messages: Vec<String>,
    pub created_at: String,
}

impl UrgentSteerRecord {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            messages: vec![message.into()],
            created_at: now_rfc3339(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UrgentSteerStore {
    path: PathBuf,
}

impl UrgentSteerStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> io::Result<Option<UrgentSteerRecord>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.path)?;
        let record = serde_json::from_str::<UrgentSteerRecord>(&content)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Some(record))
    }

    pub fn append_message(&self, message: impl Into<String>) -> io::Result<UrgentSteerRecord> {
        let message = message.into();
        let mut record = self
            .load()?
            .unwrap_or_else(|| UrgentSteerRecord::new(message.clone()));

        if record.messages.last() != Some(&message) {
            record.messages.push(message);
        }

        self.write(&record)?;
        Ok(record)
    }

    pub fn write(&self, record: &UrgentSteerRecord) -> io::Result<()> {
        crate::utils::ensure_parent_dir(&self.path)?;

        let payload = serde_json::to_string(record)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(&self.path, payload)
    }

    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub fn take(&self) -> io::Result<Option<UrgentSteerRecord>> {
        let record = self.load()?;
        self.clear()?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_message_creates_and_reloads_record() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = UrgentSteerStore::new(temp_dir.path().join("urgent-steer.json"));

        let record = store.append_message("check tests first").expect("append");
        assert_eq!(record.messages, vec!["check tests first"]);

        let loaded = store.load().expect("load").expect("record");
        assert_eq!(loaded.messages, vec!["check tests first"]);
    }

    #[test]
    fn take_returns_record_and_clears_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = UrgentSteerStore::new(temp_dir.path().join("urgent-steer.json"));
        store.append_message("steer now").expect("append");

        let record = store.take().expect("take").expect("record");
        assert_eq!(record.messages, vec!["steer now"]);
        assert!(store.load().expect("load after take").is_none());
    }
}
