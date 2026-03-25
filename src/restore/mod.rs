use crate::snapshot::store::SnapshotStore;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub mod conflict;
pub mod restore_log;

pub struct RestoreEngine {
    project_root: PathBuf,
    store: SnapshotStore,
}

impl RestoreEngine {
    pub fn new(project_root: PathBuf, store: SnapshotStore) -> Self {
        Self {
            project_root,
            store,
        }
    }

    /// Restore a file to the content at the given snapshot hash.
    ///
    /// Creates parent directories as needed.
    pub fn restore_file(&self, relative_path: &str, snapshot_hash: &str) -> Result<()> {
        let content = self.store.retrieve(snapshot_hash).with_context(|| {
            format!("retrieve snapshot {} for {}", snapshot_hash, relative_path)
        })?;

        let dest = self.project_root.join(relative_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent dirs for {}", relative_path))?;
        }
        std::fs::write(&dest, &content)
            .with_context(|| format!("write restored file {}", relative_path))?;
        Ok(())
    }

    /// Delete a file (for restoring to before a Create event).
    ///
    /// If the file does not exist this is a no-op.
    pub fn delete_file(&self, relative_path: &str) -> Result<()> {
        let path = self.project_root.join(relative_path);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("delete file {}", relative_path))?;
        }
        Ok(())
    }

    /// Get the current hash of a file on disk by reading and storing it.
    pub fn current_hash(&self, relative_path: &str) -> Result<String> {
        let path = self.project_root.join(relative_path);
        let content = std::fs::read(&path)
            .with_context(|| format!("read file {} for hashing", relative_path))?;
        self.store
            .store(&content)
            .with_context(|| format!("store content of {} for hash", relative_path))
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_engine(root: &std::path::Path) -> RestoreEngine {
        let store_dir = root.join("store");
        std::fs::create_dir_all(&store_dir).unwrap();
        let store = SnapshotStore::new(store_dir);
        RestoreEngine::new(root.to_path_buf(), store)
    }

    #[test]
    fn test_restore_file_writes_content() {
        let dir = tempdir().unwrap();
        let engine = make_engine(dir.path());

        // Pre-store content
        let content = b"hello restore";
        let hash = engine.store.store(content).unwrap();

        engine.restore_file("src/foo.rs", &hash).unwrap();

        let on_disk = std::fs::read(dir.path().join("src/foo.rs")).unwrap();
        assert_eq!(on_disk, content);
    }

    #[test]
    fn test_restore_file_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let engine = make_engine(dir.path());

        let content = b"nested content";
        let hash = engine.store.store(content).unwrap();

        engine.restore_file("a/b/c/file.rs", &hash).unwrap();
        assert!(dir.path().join("a/b/c/file.rs").exists());
    }

    #[test]
    fn test_delete_file_removes_file() {
        let dir = tempdir().unwrap();
        let engine = make_engine(dir.path());

        let file_path = dir.path().join("to_delete.rs");
        std::fs::write(&file_path, b"some content").unwrap();
        assert!(file_path.exists());

        engine.delete_file("to_delete.rs").unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn test_delete_file_nonexistent_is_noop() {
        let dir = tempdir().unwrap();
        let engine = make_engine(dir.path());
        // Should not return an error
        engine.delete_file("ghost.rs").unwrap();
    }

    #[test]
    fn test_current_hash_returns_stored_hash() {
        let dir = tempdir().unwrap();
        let engine = make_engine(dir.path());

        let content = b"hash me";
        let file_path = dir.path().join("check.rs");
        std::fs::write(&file_path, content).unwrap();

        let hash = engine.current_hash("check.rs").unwrap();
        assert_eq!(hash.len(), 64);

        // Retrieving by the returned hash gives the same content
        let retrieved = engine.store.retrieve(&hash).unwrap();
        assert_eq!(retrieved, content);
    }
}
