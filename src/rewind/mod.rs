use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::snapshot::checkpoint::CheckpointManager;
use crate::snapshot::store::SnapshotStore;

/// Rewind engine: restores files to prior snapshots and manages pre-rewind checkpoints.
pub struct RewindEngine {
    pub project_root: PathBuf,
    pub store: SnapshotStore,
    pub checkpoint_mgr: CheckpointManager,
}

impl RewindEngine {
    /// Create a new `RewindEngine`.
    pub fn new(project_root: PathBuf, store: SnapshotStore, checkpoint_mgr: CheckpointManager) -> Self {
        Self {
            project_root,
            store,
            checkpoint_mgr,
        }
    }

    /// Retrieve content for `snapshot_hash` from the store and write it to
    /// `project_root / relative_path`.
    pub fn rewind_file(&self, relative_path: &str, snapshot_hash: &str) -> Result<()> {
        let content = self
            .store
            .retrieve(snapshot_hash)
            .with_context(|| format!("retrieve snapshot {snapshot_hash} for {relative_path}"))?;

        let dest = self.project_root.join(relative_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent dirs for {relative_path}"))?;
        }
        std::fs::write(&dest, &content)
            .with_context(|| format!("write file {relative_path}"))?;

        Ok(())
    }

    /// Save the current file states as a pre-rewind checkpoint and return its ID.
    ///
    /// `current_file_states` maps relative path → content hash.
    /// `_target_hash` is accepted for API symmetry but not used.
    pub fn rewind_all(
        &self,
        current_file_states: &HashMap<String, String>,
        _target_hash: &str,
    ) -> Result<u32> {
        let checkpoint_id = self
            .checkpoint_mgr
            .save(current_file_states.clone())
            .context("save pre-rewind checkpoint")?;

        Ok(checkpoint_id)
    }

    /// Load the checkpoint with `checkpoint_id` and restore every file it records.
    pub fn rewind_to_checkpoint(&self, checkpoint_id: u32) -> Result<()> {
        let files = self
            .checkpoint_mgr
            .load(checkpoint_id)
            .with_context(|| format!("load checkpoint {checkpoint_id}"))?;

        for (relative_path, hash) in &files {
            self.rewind_file(relative_path, hash)?;
        }

        Ok(())
    }

    /// Undo the most recent rewind by restoring the pre-rewind checkpoint.
    pub fn undo_rewind(&self, pre_rewind_checkpoint_id: u32) -> Result<()> {
        self.rewind_to_checkpoint(pre_rewind_checkpoint_id)
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_engine(tmp: &tempfile::TempDir) -> RewindEngine {
        let store = SnapshotStore::new(tmp.path().join("store"));
        let checkpoint_mgr = CheckpointManager::new(tmp.path().join("checkpoints"));
        RewindEngine::new(tmp.path().join("project"), store, checkpoint_mgr)
    }

    #[test]
    fn test_rewind_file_restores_content() {
        let tmp = tempdir().unwrap();
        let engine = make_engine(&tmp);

        let original = b"original content";
        let hash = engine.store.store(original).unwrap();

        // Write a modified version first.
        let file_path = engine.project_root.join("src/foo.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"modified content").unwrap();

        engine.rewind_file("src/foo.rs", &hash).unwrap();

        let restored = std::fs::read(&file_path).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_rewind_all_returns_nonzero_checkpoint_id() {
        let tmp = tempdir().unwrap();
        let engine = make_engine(&tmp);

        let mut states = HashMap::new();
        states.insert("src/main.rs".to_string(), "somehash".to_string());

        let id = engine.rewind_all(&states, "target").unwrap();
        assert!(id > 0);
    }
}
