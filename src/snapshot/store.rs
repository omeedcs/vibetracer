use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Content-addressed store for file snapshots, keyed by SHA-256 hash.
///
/// Files are stored under `base_dir/<2-char prefix>/<remaining 62 chars>`,
/// mirroring the layout used by Git's object store.
pub struct SnapshotStore {
    base_dir: PathBuf,
}

impl SnapshotStore {
    /// Create a new store rooted at `base_dir`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Compute the SHA-256 hex digest of `content`.
    fn hash(content: &[u8]) -> String {
        let digest = Sha256::digest(content);
        hex::encode(digest)
    }

    /// Path where an object with the given hex `hash` would be stored.
    fn object_path(&self, hash: &str) -> PathBuf {
        let (prefix, rest) = hash.split_at(2);
        self.base_dir.join(prefix).join(rest)
    }

    /// Store `content` and return its hex hash.
    ///
    /// If an object with the same hash already exists the write is skipped
    /// (deduplication).
    pub fn store(&self, content: &[u8]) -> Result<String> {
        let hash = Self::hash(content);
        let path = self.object_path(&hash);

        if !path.exists() {
            // Ensure the prefix directory exists.
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create prefix dir for hash {hash}"))?;
            }
            std::fs::write(&path, content)
                .with_context(|| format!("write object {hash}"))?;
        }

        Ok(hash)
    }

    /// Retrieve the raw bytes stored under `hash`.
    pub fn retrieve(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.object_path(hash);
        std::fs::read(&path).with_context(|| format!("read object {hash}"))
    }

    /// Return `true` if an object with `hash` exists in the store.
    pub fn exists(&self, hash: &str) -> bool {
        self.object_path(hash).exists()
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_store_and_retrieve_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().to_path_buf());

        let content = b"unit test content";
        let hash = store.store(content).unwrap();
        assert_eq!(hash.len(), 64);
        assert_eq!(store.retrieve(&hash).unwrap(), content);
    }

    #[test]
    fn test_exists_returns_false_for_unknown_hash() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().to_path_buf());
        assert!(!store.exists("0000000000000000000000000000000000000000000000000000000000000000"));
    }
}
