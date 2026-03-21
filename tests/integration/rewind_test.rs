use std::collections::HashMap;
use tempfile::tempdir;
use vibetracer::snapshot::checkpoint::CheckpointManager;
use vibetracer::snapshot::store::SnapshotStore;
use vibetracer::rewind::RewindEngine;

fn make_engine(tmp: &tempfile::TempDir) -> RewindEngine {
    let store = SnapshotStore::new(tmp.path().join("store"));
    let checkpoint_mgr = CheckpointManager::new(tmp.path().join("checkpoints"));
    RewindEngine::new(tmp.path().join("project"), store, checkpoint_mgr)
}

#[test]
fn test_rewind_restores_file() {
    let tmp = tempdir().unwrap();
    let engine = make_engine(&tmp);

    // Store the original content in the snapshot store.
    let original = b"original content";
    let hash = engine.store.store(original).expect("store snapshot");

    // Create the file on disk with modified content.
    let file_path = engine.project_root.join("src/lib.rs");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, b"modified content").unwrap();

    // Rewind the file to the stored snapshot.
    engine.rewind_file("src/lib.rs", &hash).expect("rewind file");

    // Verify the file content was restored.
    let restored = std::fs::read(&file_path).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn test_rewind_creates_pre_rewind_checkpoint() {
    let tmp = tempdir().unwrap();
    let engine = make_engine(&tmp);

    let mut current_states = HashMap::new();
    current_states.insert("src/main.rs".to_string(), "abc123".to_string());
    current_states.insert("src/lib.rs".to_string(), "def456".to_string());

    let checkpoint_id = engine
        .rewind_all(&current_states, "target_hash")
        .expect("rewind_all");

    assert!(checkpoint_id > 0, "checkpoint ID should be > 0, got {checkpoint_id}");
}
