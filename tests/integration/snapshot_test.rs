use std::collections::HashMap;
use tempfile::tempdir;
use vibetracer::event::{EditEvent, EditKind};
use vibetracer::snapshot::checkpoint::CheckpointManager;
use vibetracer::snapshot::edit_log::EditLog;
use vibetracer::snapshot::store::SnapshotStore;

// ─── SnapshotStore tests ──────────────────────────────────────────────────────

#[test]
fn test_store_and_retrieve_content() {
    let dir = tempdir().unwrap();
    let store = SnapshotStore::new(dir.path().to_path_buf());

    let content = b"hello, vibetracer!";
    let hash = store.store(content).expect("store content");

    // hash must be a 64-char hex string (SHA-256)
    assert_eq!(hash.len(), 64);

    // retrieve roundtrip
    let retrieved = store.retrieve(&hash).expect("retrieve content");
    assert_eq!(retrieved, content);

    // same content → same hash
    let hash2 = store.store(content).expect("store again");
    assert_eq!(hash, hash2);
}

#[test]
fn test_store_deduplicates() {
    let dir = tempdir().unwrap();
    let store = SnapshotStore::new(dir.path().to_path_buf());

    let content = b"deduplicate me";
    store.store(content).expect("first store");
    store.store(content).expect("second store");

    // The two-char prefix directory should exist exactly once
    let hash = store.store(content).expect("third store");
    let prefix = &hash[..2];
    let prefix_dir = dir.path().join(prefix);
    assert!(prefix_dir.exists(), "prefix dir should exist");

    // Only one file inside the prefix dir (not duplicated)
    let entries: Vec<_> = std::fs::read_dir(&prefix_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "content should be stored exactly once");
}

// ─── EditLog tests ────────────────────────────────────────────────────────────

fn make_event(id: u64, file: &str) -> EditEvent {
    EditEvent {
        id,
        ts: 1_700_000_000_000 + id as i64,
        file: file.to_string(),
        kind: EditKind::Modify,
        patch: format!("@@ -1 +1 @@\n-old{id}\n+new{id}"),
        before_hash: None,
        after_hash: format!("hash{id}"),
        intent: None,
        tool: None,
        lines_added: 1,
        lines_removed: 1,
        agent_id: None,
        agent_label: None,
        operation_id: None,
        operation_intent: None,
        tool_name: None,
        restore_id: None,
    }
}

#[test]
fn test_edit_log_append_and_read() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("edits.jsonl");
    let log = EditLog::new(log_path.clone());

    let e1 = make_event(1, "src/main.rs");
    let e2 = make_event(2, "src/lib.rs");

    log.append(&e1).expect("append e1");
    log.append(&e2).expect("append e2");

    // count via the instance method
    let count = log.count().expect("count");
    assert_eq!(count, 2);

    // read_all via static method
    let events = EditLog::read_all(&log_path).expect("read_all");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, 1);
    assert_eq!(events[0].file, "src/main.rs");
    assert_eq!(events[1].id, 2);
    assert_eq!(events[1].file, "src/lib.rs");
}

// ─── CheckpointManager tests ──────────────────────────────────────────────────

#[test]
fn test_checkpoint_save_and_load() {
    let dir = tempdir().unwrap();
    let mgr = CheckpointManager::new(dir.path().to_path_buf());

    let mut files = HashMap::new();
    files.insert("src/main.rs".to_string(), "aabbcc".to_string());
    files.insert("src/lib.rs".to_string(), "ddeeff".to_string());

    let id1 = mgr.save(files.clone()).expect("save checkpoint 1");
    assert_eq!(id1, 1);

    let loaded = mgr.load(id1).expect("load checkpoint 1");
    assert_eq!(loaded.get("src/main.rs").unwrap(), "aabbcc");
    assert_eq!(loaded.get("src/lib.rs").unwrap(), "ddeeff");

    // Save a second checkpoint
    let mut files2 = HashMap::new();
    files2.insert("src/main.rs".to_string(), "112233".to_string());
    let id2 = mgr.save(files2).expect("save checkpoint 2");
    assert_eq!(id2, 2);

    // list returns both IDs in sorted order
    let ids = mgr.list().expect("list");
    assert_eq!(ids, vec![1, 2]);

    // Verify second checkpoint data
    let loaded2 = mgr.load(id2).expect("load checkpoint 2");
    assert_eq!(loaded2.get("src/main.rs").unwrap(), "112233");
    assert!(!loaded2.contains_key("src/lib.rs"));
}
