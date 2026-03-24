use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::Result;
use chrono::Utc;

use crate::event::{EditEvent, EditKind};
use crate::snapshot::{edit_log::EditLog, store::SnapshotStore};
use crate::watcher::differ::compute_diff;

/// Result returned by [`Recorder::process_file_change`] when a real change is detected.
pub struct RecordResult {
    /// The edit event that was created and persisted.
    pub event: EditEvent,
    /// The file content before the change (empty string if first seen).
    pub old_content: String,
    /// The file content after the change.
    pub new_content: String,
}

/// Optional enrichment fields that a daemon or hook can attach to an edit event.
#[derive(Debug, Clone, Default)]
pub struct Enrichment {
    pub agent_id: Option<String>,
    pub agent_label: Option<String>,
    pub operation_id: Option<String>,
    pub operation_intent: Option<String>,
    pub tool_name: Option<String>,
    pub restore_id: Option<u64>,
}

/// Encapsulates the recording pipeline: snapshot storage, edit logging, file
/// hash tracking, and edit ID assignment.
///
/// Both the TUI and the daemon share this module so that recording logic lives
/// in exactly one place.
pub struct Recorder {
    /// Absolute path to the project root (used to compute relative paths).
    pub project_root: PathBuf,
    snapshot_store: SnapshotStore,
    edit_log: EditLog,
    file_hashes: HashMap<String, String>,
    edit_id_counter: u64,
}

impl Recorder {
    /// Create a new recorder.
    ///
    /// * `project_root` -- absolute path to the watched project directory.
    /// * `session_dir`  -- path to the current session directory (contains
    ///   `snapshots/` and `edits.jsonl`).
    pub fn new(project_root: PathBuf, session_dir: PathBuf) -> Self {
        Self {
            project_root,
            snapshot_store: SnapshotStore::new(session_dir.join("snapshots")),
            edit_log: EditLog::new(session_dir.join("edits.jsonl")),
            file_hashes: HashMap::new(),
            edit_id_counter: 1,
        }
    }

    /// Process a file-system change at `abs_path`.
    ///
    /// Returns `Ok(Some(RecordResult))` when the file genuinely changed,
    /// `Ok(None)` when the content is identical to the last known snapshot,
    /// or an error if IO / serialization fails.
    ///
    /// When a change is detected the method:
    /// 1. Computes the relative path from the project root.
    /// 2. Reads the new content from disk.
    /// 3. Retrieves the old content from the snapshot store (empty if first seen).
    /// 4. Returns `None` if content is unchanged.
    /// 5. Computes a unified diff.
    /// 6. Determines the edit kind (Create / Modify / Delete).
    /// 7. Stores the new snapshot.
    /// 8. Applies enrichment fields (if provided).
    /// 9. Appends the event to the edit log.
    /// 10. Sends the event on the channel.
    /// 11. Returns the `RecordResult`.
    pub fn process_file_change(
        &mut self,
        abs_path: &Path,
        event_tx: &mpsc::Sender<EditEvent>,
        enrichment: Option<&Enrichment>,
    ) -> Result<Option<RecordResult>> {
        // 1. Compute relative path from project root.
        let rel_path = abs_path
            .strip_prefix(&self.project_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| abs_path.to_string_lossy().to_string());

        // 2. Read new content from disk (treat missing file as empty / delete).
        let new_content = std::fs::read_to_string(abs_path).unwrap_or_default();

        // 3. Look up old content from snapshot store (empty if first edit).
        let old_content = if let Some(hash) = self.file_hashes.get(&rel_path) {
            self.snapshot_store
                .retrieve(hash)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // 4. Skip if content hasn't changed.
        if old_content == new_content {
            return Ok(None);
        }

        // 5. Compute diff.
        let diff = compute_diff(&old_content, &new_content, &rel_path);

        // 6. Determine edit kind.
        let kind = if !abs_path.exists() {
            EditKind::Delete
        } else if self.file_hashes.contains_key(&rel_path) {
            EditKind::Modify
        } else {
            EditKind::Create
        };

        // 7. Store new snapshot.
        let after_hash = self.snapshot_store.store(new_content.as_bytes())?;
        let before_hash = self.file_hashes.get(&rel_path).cloned();

        // Build the edit event.
        let mut edit = EditEvent {
            id: self.edit_id_counter,
            ts: Utc::now().timestamp_millis(),
            file: rel_path.clone(),
            kind,
            patch: diff.patch,
            before_hash,
            after_hash: after_hash.clone(),
            intent: None,
            tool: None,
            lines_added: diff.lines_added,
            lines_removed: diff.lines_removed,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };

        // 8. Apply enrichment fields if provided.
        if let Some(enrich) = enrichment {
            edit.agent_id = enrich.agent_id.clone();
            edit.agent_label = enrich.agent_label.clone();
            edit.operation_id = enrich.operation_id.clone();
            edit.operation_intent = enrich.operation_intent.clone();
            edit.tool_name = enrich.tool_name.clone();
            edit.restore_id = enrich.restore_id;
        }

        self.edit_id_counter += 1;

        // 9. Append to edit log.
        self.edit_log.append(&edit)?;

        // Update internal hash tracking.
        self.file_hashes.insert(rel_path, after_hash);

        // 10. Send event on channel (ignore send errors -- receiver may have dropped).
        let _ = event_tx.send(edit.clone());

        // 11. Return the result.
        Ok(Some(RecordResult {
            event: edit,
            old_content,
            new_content,
        }))
    }

    /// Return a reference to the current file hash map (relative path -> SHA-256).
    pub fn current_file_hashes(&self) -> &HashMap<String, String> {
        &self.file_hashes
    }

    /// Return a reference to the underlying snapshot store.
    pub fn snapshot_store(&self) -> &SnapshotStore {
        &self.snapshot_store
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tempfile::tempdir;

    /// Helper: create a Recorder backed by a temp directory, plus the project root path.
    fn setup() -> (Recorder, PathBuf) {
        let tmp = tempdir().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();

        let session_dir = tmp.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();

        let recorder = Recorder::new(project_root.clone(), session_dir);
        // We need to keep tmp alive, but we leak it so paths stay valid.
        std::mem::forget(tmp);
        (recorder, project_root)
    }

    #[test]
    fn first_file_change_creates_event() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        // Write a new file inside the project.
        let file_path = project_root.join("hello.txt");
        std::fs::write(&file_path, "hello world\n").unwrap();

        let result = recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();

        assert!(result.is_some(), "first change should produce a result");
        let result = result.unwrap();

        assert_eq!(result.event.file, "hello.txt");
        assert_eq!(result.event.kind, EditKind::Create);
        assert_eq!(result.event.id, 1);
        assert!(result.old_content.is_empty());
        assert_eq!(result.new_content, "hello world\n");
        assert!(result.event.lines_added > 0);
    }

    #[test]
    fn second_change_creates_modify_event() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        let file_path = project_root.join("app.rs");
        std::fs::write(&file_path, "fn main() {}\n").unwrap();
        recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();

        // Modify the file.
        std::fs::write(&file_path, "fn main() { println!(\"hi\"); }\n").unwrap();
        let result = recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.event.kind, EditKind::Modify);
        assert_eq!(result.event.id, 2);
        assert_eq!(result.old_content, "fn main() {}\n");
        assert_eq!(result.new_content, "fn main() { println!(\"hi\"); }\n");
        assert!(result.event.before_hash.is_some());
    }

    #[test]
    fn unchanged_file_returns_none() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        let file_path = project_root.join("stable.txt");
        std::fs::write(&file_path, "unchanged\n").unwrap();
        recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();

        // Process same file again without changes.
        let result = recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();
        assert!(result.is_none(), "unchanged file should return None");
    }

    #[test]
    fn enrichment_fields_applied() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        let file_path = project_root.join("enriched.txt");
        std::fs::write(&file_path, "data\n").unwrap();

        let enrichment = Enrichment {
            agent_id: Some("agent-42".to_string()),
            agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-7".to_string()),
            operation_intent: Some("refactor auth".to_string()),
            tool_name: Some("Edit".to_string()),
            restore_id: Some(99),
        };

        let result = recorder
            .process_file_change(&file_path, &tx, Some(&enrichment))
            .unwrap()
            .unwrap();

        assert_eq!(result.event.agent_id, Some("agent-42".to_string()));
        assert_eq!(result.event.agent_label, Some("claude-1".to_string()));
        assert_eq!(result.event.operation_id, Some("op-7".to_string()));
        assert_eq!(
            result.event.operation_intent,
            Some("refactor auth".to_string())
        );
        assert_eq!(result.event.tool_name, Some("Edit".to_string()));
        assert_eq!(result.event.restore_id, Some(99));
    }

    #[test]
    fn edit_id_increments() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        let file_a = project_root.join("a.txt");
        let file_b = project_root.join("b.txt");
        std::fs::write(&file_a, "aaa\n").unwrap();
        std::fs::write(&file_b, "bbb\n").unwrap();

        let r1 = recorder
            .process_file_change(&file_a, &tx, None)
            .unwrap()
            .unwrap();
        let r2 = recorder
            .process_file_change(&file_b, &tx, None)
            .unwrap()
            .unwrap();

        assert_eq!(r1.event.id, 1);
        assert_eq!(r2.event.id, 2);
    }

    #[test]
    fn file_hashes_updated() {
        let (mut recorder, project_root) = setup();
        let (tx, _rx) = mpsc::channel();

        assert!(recorder.current_file_hashes().is_empty());

        let file_path = project_root.join("tracked.txt");
        std::fs::write(&file_path, "v1\n").unwrap();
        recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();

        assert!(recorder.current_file_hashes().contains_key("tracked.txt"));

        // Modify and check hash changes.
        let hash_v1 = recorder.current_file_hashes()["tracked.txt"].clone();
        std::fs::write(&file_path, "v2\n").unwrap();
        recorder
            .process_file_change(&file_path, &tx, None)
            .unwrap();
        let hash_v2 = recorder.current_file_hashes()["tracked.txt"].clone();

        assert_ne!(hash_v1, hash_v2);
    }
}
