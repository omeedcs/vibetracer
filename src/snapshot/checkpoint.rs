use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A full project state snapshot.
#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Auto-incremented ID (1-based).
    pub id: u32,
    /// Unix timestamp (milliseconds) when the checkpoint was saved.
    pub ts: i64,
    /// Map of relative file path → SHA-256 hex hash.
    pub files: HashMap<String, String>,
}

/// Manages a directory of JSON checkpoint files (`001.json`, `002.json`, …).
pub struct CheckpointManager {
    dir: PathBuf,
}

impl CheckpointManager {
    /// Create a manager backed by the directory at `dir`.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Filename for a given checkpoint ID (e.g. `001.json`).
    fn filename(id: u32) -> String {
        format!("{id:03}.json")
    }

    /// Determine the next available ID by scanning existing checkpoint files.
    fn next_id(&self) -> Result<u32> {
        let existing = self.list()?;
        Ok(existing.last().copied().unwrap_or(0) + 1)
    }

    /// Save a new checkpoint with the given `files` map.
    ///
    /// Returns the auto-assigned checkpoint ID.
    pub fn save(&self, files: HashMap<String, String>) -> Result<u32> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("create checkpoint dir {:?}", self.dir))?;

        let id = self.next_id()?;
        let ts = chrono::Utc::now().timestamp_millis();
        let checkpoint = Checkpoint { id, ts, files };

        let path = self.dir.join(Self::filename(id));
        let json = serde_json::to_string_pretty(&checkpoint)
            .context("serialize checkpoint")?;
        std::fs::write(&path, json)
            .with_context(|| format!("write checkpoint {path:?}"))?;

        Ok(id)
    }

    /// Load the checkpoint with the given `id` and return its `files` map.
    pub fn load(&self, id: u32) -> Result<HashMap<String, String>> {
        let path = self.dir.join(Self::filename(id));
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read checkpoint {path:?}"))?;
        let checkpoint: Checkpoint = serde_json::from_str(&raw)
            .with_context(|| format!("deserialize checkpoint {path:?}"))?;
        Ok(checkpoint.files)
    }

    /// Return a sorted list of all checkpoint IDs present in the directory.
    pub fn list(&self) -> Result<Vec<u32>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.dir)
            .with_context(|| format!("read checkpoint dir {:?}", self.dir))?
        {
            let entry = entry.context("read dir entry")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".json") {
                if let Ok(id) = stem.parse::<u32>() {
                    ids.push(id);
                }
            }
        }
        ids.sort_unstable();
        Ok(ids)
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_creates_file() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_path_buf());

        let mut files = HashMap::new();
        files.insert("a.rs".to_string(), "hash1".to_string());

        let id = mgr.save(files).unwrap();
        assert_eq!(id, 1);
        assert!(dir.path().join("001.json").exists());
    }

    #[test]
    fn test_list_empty_dir() {
        let dir = tempdir().unwrap();
        let mgr = CheckpointManager::new(dir.path().to_path_buf());
        assert_eq!(mgr.list().unwrap(), Vec::<u32>::new());
    }
}
