use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::event::EditEvent;
use crate::session::SessionManager;
use crate::snapshot::edit_log::EditLog;
use crate::snapshot::store::SnapshotStore;
use crate::watcher::differ::compute_diff;

use super::pagination::{PageParams, read_edits_paged};

/// Context for MCP tool handler dispatch. Holds the path to the sessions
/// directory and provides helper methods for locating session artifacts.
pub struct HandlerContext {
    sessions_dir: PathBuf,
}

impl HandlerContext {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Returns the directory for `session_id`, or an error if it does not exist.
    pub fn session_dir(&self, session_id: &str) -> Result<PathBuf> {
        let dir = self.sessions_dir.join(session_id);
        if !dir.exists() {
            bail!("session not found: {}", session_id);
        }
        Ok(dir)
    }

    /// Returns the `edits.jsonl` path for `session_id`, or an error if it does
    /// not exist.
    pub fn edits_path(&self, session_id: &str) -> Result<PathBuf> {
        let dir = self.session_dir(session_id)?;
        let path = dir.join("edits.jsonl");
        if !path.exists() {
            bail!("edits.jsonl not found for session {}", session_id);
        }
        Ok(path)
    }

    /// Creates a `SnapshotStore` rooted at the session's `snapshots/`
    /// directory.
    pub fn snapshot_store(&self, session_id: &str) -> Result<SnapshotStore> {
        let dir = self.session_dir(session_id)?;
        Ok(SnapshotStore::new(dir.join("snapshots")))
    }

    /// Extract pagination parameters from a JSON arguments object. Missing or
    /// non-integer values fall back to defaults.
    pub fn page_params(args: &Value) -> PageParams {
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(super::pagination::DEFAULT_LIMIT as u64) as u32;
        PageParams { offset, limit }
    }

    // ── Tool handlers ────────────────────────────────────────────────────

    /// `list_sessions` — enumerate all sessions with metadata.
    pub fn handle_list_sessions(&self, _args: &Value) -> Result<Value> {
        let mgr = SessionManager::new(self.sessions_dir.clone());
        let metas = mgr.list()?;

        let mut sessions = Vec::new();
        for m in &metas {
            let edits_path = self.sessions_dir.join(&m.id).join("edits.jsonl");
            let edit_count = EditLog::new(edits_path).count().unwrap_or(0);

            sessions.push(json!({
                "id": m.id,
                "project_path": m.project_path,
                "started_at": m.started_at,
                "mode": m.mode,
                "agent_count": m.agents.len(),
                "edit_count": edit_count,
            }));
        }

        let total_count = sessions.len();
        Ok(json!({
            "sessions": sessions,
            "total_count": total_count,
        }))
    }

    /// `get_timeline` — paginated chronological edit list.
    pub fn handle_get_timeline(&self, args: &Value) -> Result<Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("session_id is required")?;

        let edits_path = self.edits_path(session_id)?;
        let params = Self::page_params(args);

        // Optional glob-based file filter.
        let file_filter = args
            .get("file_filter")
            .and_then(|v| v.as_str())
            .and_then(|pat| glob::Pattern::new(pat).ok());

        let filter_fn = |e: &EditEvent| -> bool {
            if let Some(ref pat) = file_filter {
                pat.matches(&e.file)
            } else {
                true
            }
        };

        let result = read_edits_paged(&edits_path, &params, Some(&filter_fn))?;

        let edits: Vec<Value> = result
            .events
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "ts": e.ts,
                    "file": e.file,
                    "kind": e.kind,
                    "lines_added": e.lines_added,
                    "lines_removed": e.lines_removed,
                    "agent_label": e.agent_label,
                    "operation_id": e.operation_id,
                    "intent": e.intent,
                })
            })
            .collect();

        Ok(json!({
            "edits": edits,
            "total_count": result.total_count,
        }))
    }

    /// `get_frame` — reconstruct file state at a given frame.
    pub fn handle_get_frame(&self, args: &Value) -> Result<Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("session_id is required")?;
        let frame_id = args
            .get("frame_id")
            .and_then(|v| v.as_u64())
            .context("frame_id is required")? as u64;

        let edits_path = self.edits_path(session_id)?;
        let all_edits = EditLog::read_all(&edits_path)?;
        let max_id = all_edits.len() as u64;

        if frame_id == 0 || frame_id > max_id {
            bail!(
                "frame {} out of range (session has {} edits)",
                frame_id,
                max_id
            );
        }

        let file_filter = args.get("file").and_then(|v| v.as_str());
        let store = self.snapshot_store(session_id)?;

        // For each file, find the last edit at or before frame_id.
        let mut latest_by_file: HashMap<String, &EditEvent> = HashMap::new();
        for edit in &all_edits {
            if edit.id > frame_id {
                break;
            }
            if let Some(f) = file_filter {
                if edit.file != f {
                    continue;
                }
            }
            latest_by_file.insert(edit.file.clone(), edit);
        }

        let mut files: Vec<Value> = Vec::new();
        let mut sorted_paths: Vec<&String> = latest_by_file.keys().collect();
        sorted_paths.sort();

        for path in sorted_paths {
            let edit = latest_by_file[path];
            let content = match store.retrieve(&edit.after_hash) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => String::new(),
            };
            files.push(json!({
                "path": path,
                "content": content,
                "hash": edit.after_hash,
            }));
        }

        Ok(json!({ "files": files }))
    }

    /// `diff_frames` — diff file state between two frames.
    pub fn handle_diff_frames(&self, args: &Value) -> Result<Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("session_id is required")?;
        let frame_a = args
            .get("frame_a")
            .and_then(|v| v.as_u64())
            .context("frame_a is required")?;
        let frame_b = args
            .get("frame_b")
            .and_then(|v| v.as_u64())
            .context("frame_b is required")?;

        let file_filter = args.get("file").and_then(|v| v.as_str());

        // Build args for each frame, forwarding the optional file filter.
        let mut args_a = json!({
            "session_id": session_id,
            "frame_id": frame_a,
        });
        let mut args_b = json!({
            "session_id": session_id,
            "frame_id": frame_b,
        });
        if let Some(f) = file_filter {
            args_a["file"] = json!(f);
            args_b["file"] = json!(f);
        }

        let frame_a_result = self.handle_get_frame(&args_a)?;
        let frame_b_result = self.handle_get_frame(&args_b)?;

        // Build path -> content maps.
        let map_a = Self::files_to_map(&frame_a_result);
        let map_b = Self::files_to_map(&frame_b_result);

        // Collect all paths from both frames.
        let mut all_paths: Vec<&str> = map_a
            .keys()
            .chain(map_b.keys())
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        all_paths.sort();

        let mut diffs: Vec<Value> = Vec::new();
        for path in &all_paths {
            let old = map_a.get(path).copied().unwrap_or("");
            let new = map_b.get(path).copied().unwrap_or("");
            if old == new {
                continue;
            }
            let diff_result = compute_diff(old, new, path);
            diffs.push(json!({
                "path": path,
                "diff": diff_result.patch,
            }));
        }

        Ok(json!({ "diffs": diffs }))
    }

    /// `search_edits` — regex or substring search over edit patches, files,
    /// and intents.
    pub fn handle_search_edits(&self, args: &Value) -> Result<Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("session_id is required")?;
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("query is required")?;

        let edits_path = self.edits_path(session_id)?;
        let params = Self::page_params(args);

        // Try to compile as regex; fall back to literal substring match.
        let re = regex::Regex::new(query).ok();
        let query_owned = query.to_string();

        let filter_fn = move |e: &EditEvent| -> bool {
            let fields = [
                e.patch.as_str(),
                e.file.as_str(),
                e.intent.as_deref().unwrap_or(""),
            ];
            if let Some(ref re) = re {
                fields.iter().any(|f| re.is_match(f))
            } else {
                fields.iter().any(|f| f.contains(query_owned.as_str()))
            }
        };

        let result = read_edits_paged(&edits_path, &params, Some(&filter_fn))?;

        let edits: Vec<Value> = result
            .events
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "ts": e.ts,
                    "file": e.file,
                    "kind": e.kind,
                    "patch": e.patch,
                    "intent": e.intent,
                })
            })
            .collect();

        Ok(json!({
            "edits": edits,
            "total_count": result.total_count,
        }))
    }

    /// `get_regression_window` — return frames in a range, optionally filtered
    /// by file.
    pub fn handle_get_regression_window(&self, args: &Value) -> Result<Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("session_id is required")?;

        let edits_path = self.edits_path(session_id)?;
        let all_edits = EditLog::read_all(&edits_path)?;

        let file_filter = args.get("file").and_then(|v| v.as_str());
        let start_frame = args.get("start_frame").and_then(|v| v.as_u64());
        let end_frame = args.get("end_frame").and_then(|v| v.as_u64());

        let frames: Vec<Value> = all_edits
            .iter()
            .filter(|e| {
                if let Some(f) = file_filter {
                    if e.file != f {
                        return false;
                    }
                }
                if let Some(start) = start_frame {
                    if e.id < start {
                        return false;
                    }
                }
                if let Some(end) = end_frame {
                    if e.id > end {
                        return false;
                    }
                }
                true
            })
            .map(|e| {
                json!({
                    "frame_id": e.id,
                    "file": e.file,
                    "patch": e.patch,
                    "before_hash": e.before_hash,
                    "after_hash": e.after_hash,
                })
            })
            .collect();

        Ok(json!({ "frames": frames }))
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /// Build a path -> content map from a `get_frame` result value.
    fn files_to_map(frame_result: &Value) -> HashMap<&str, &str> {
        let mut map = HashMap::new();
        if let Some(files) = frame_result.get("files").and_then(|v| v.as_array()) {
            for f in files {
                if let (Some(p), Some(c)) = (
                    f.get("path").and_then(|v| v.as_str()),
                    f.get("content").and_then(|v| v.as_str()),
                ) {
                    map.insert(p, c);
                }
            }
        }
        map
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EditKind;
    use crate::session::{SessionMeta, SessionMode};
    use serde_json::json;
    use tempfile::tempdir;

    /// Create a test session with `edit_count` edits. Files cycle through
    /// `file_0.rs`, `file_1.rs`, `file_2.rs`. Patches use the format
    /// `@@ -1 +1 @@\n-old{i}\n+new{i}`.
    fn create_test_session(sessions_dir: &std::path::Path, session_id: &str, edit_count: u32) {
        let dir = sessions_dir.join(session_id);
        std::fs::create_dir_all(dir.join("snapshots")).unwrap();

        let meta = SessionMeta {
            id: session_id.to_string(),
            project_path: "/tmp/project".to_string(),
            started_at: 1_700_000_000,
            mode: SessionMode::Enriched,
            agents: Vec::new(),
        };
        std::fs::write(
            dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();

        let log = EditLog::new(dir.join("edits.jsonl"));
        for i in 1..=edit_count {
            let event = EditEvent {
                id: i as u64,
                ts: 1_700_000_000_000 + i as i64,
                file: format!("file_{}.rs", i % 3),
                kind: EditKind::Modify,
                patch: format!("@@ -1 +1 @@\n-old{}\n+new{}", i, i),
                before_hash: Some(format!("before_{}", i)),
                after_hash: format!("after_{}", i),
                intent: Some(format!("intent for edit {}", i)),
                tool: None,
                lines_added: 1,
                lines_removed: 1,
                agent_id: None,
                agent_label: None,
                operation_id: None,
                operation_intent: None,
                tool_name: None,
                restore_id: None,
            };
            log.append(&event).unwrap();
        }
    }

    // ── test_handle_list_sessions ────────────────────────────────────────

    #[test]
    fn test_handle_list_sessions() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();

        create_test_session(&sessions_dir, "session-a", 3);
        create_test_session(&sessions_dir, "session-b", 5);

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx.handle_list_sessions(&json!({})).unwrap();

        let sessions = result["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(result["total_count"], 2);

        // Both session IDs should be present.
        let ids: Vec<&str> = sessions.iter().map(|s| s["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"session-a"));
        assert!(ids.contains(&"session-b"));

        // Verify edit counts.
        for s in sessions {
            let id = s["id"].as_str().unwrap();
            match id {
                "session-a" => assert_eq!(s["edit_count"], 3),
                "session-b" => assert_eq!(s["edit_count"], 5),
                _ => panic!("unexpected session id: {}", id),
            }
        }
    }

    // ── test_handle_get_timeline ─────────────────────────────────────────

    #[test]
    fn test_handle_get_timeline() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        create_test_session(&sessions_dir, "sess-1", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx
            .handle_get_timeline(&json!({
                "session_id": "sess-1",
                "offset": 2,
                "limit": 3,
            }))
            .unwrap();

        let edits = result["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 3);
        assert_eq!(edits[0]["id"], 3);
        assert_eq!(edits[1]["id"], 4);
        assert_eq!(edits[2]["id"], 5);
        assert_eq!(result["total_count"], 10);
    }

    // ── test_handle_get_timeline_with_file_filter ────────────────────────

    #[test]
    fn test_handle_get_timeline_with_file_filter() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        // 9 edits: files cycle file_1.rs, file_2.rs, file_0.rs, ...
        create_test_session(&sessions_dir, "sess-2", 9);

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx
            .handle_get_timeline(&json!({
                "session_id": "sess-2",
                "file_filter": "file_1.rs",
            }))
            .unwrap();

        let edits = result["edits"].as_array().unwrap();
        // Edits whose i%3==1: i=1,4,7 -> 3 matches
        assert_eq!(edits.len(), 3);
        assert_eq!(result["total_count"], 3);
        for e in edits {
            assert_eq!(e["file"].as_str().unwrap(), "file_1.rs");
        }
    }

    // ── test_handle_search_edits ─────────────────────────────────────────

    #[test]
    fn test_handle_search_edits() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        create_test_session(&sessions_dir, "sess-3", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx
            .handle_search_edits(&json!({
                "session_id": "sess-3",
                "query": "new5",
            }))
            .unwrap();

        let edits = result["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["id"], 5);
        assert_eq!(result["total_count"], 1);
    }

    // ── test_handle_get_regression_window ────────────────────────────────

    #[test]
    fn test_handle_get_regression_window() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        create_test_session(&sessions_dir, "sess-4", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx
            .handle_get_regression_window(&json!({
                "session_id": "sess-4",
                "file": "file_1.rs",
                "start_frame": 2,
                "end_frame": 8,
            }))
            .unwrap();

        let frames = result["frames"].as_array().unwrap();
        // file_1.rs edits: i%3==1 -> 1,4,7,10. In range 2..=8: 4 and 7.
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["frame_id"], 4);
        assert_eq!(frames[1]["frame_id"], 7);
    }

    // ── test_handle_get_frame (with real snapshots) ──────────────────────

    #[test]
    fn test_handle_get_frame() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        let session_id = "snap-sess";
        let session_dir = sessions_dir.join(session_id);
        std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();

        // Write meta.json
        let meta = SessionMeta {
            id: session_id.to_string(),
            project_path: "/tmp".to_string(),
            started_at: 1_700_000_000,
            mode: SessionMode::Enriched,
            agents: Vec::new(),
        };
        std::fs::write(
            session_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();

        // Store real snapshots.
        let store = SnapshotStore::new(session_dir.join("snapshots"));
        let hash1 = store.store(b"content version 1").unwrap();
        let hash2 = store.store(b"content version 2").unwrap();

        // Write edits referencing the stored hashes.
        let log = EditLog::new(session_dir.join("edits.jsonl"));
        let e1 = EditEvent {
            id: 1,
            ts: 1000,
            file: "main.rs".to_string(),
            kind: EditKind::Create,
            patch: String::new(),
            before_hash: None,
            after_hash: hash1.clone(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };
        let e2 = EditEvent {
            id: 2,
            ts: 2000,
            file: "main.rs".to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: Some(hash1.clone()),
            after_hash: hash2.clone(),
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
        };
        log.append(&e1).unwrap();
        log.append(&e2).unwrap();

        let ctx = HandlerContext::new(sessions_dir);

        // Frame 1: should see version 1 content
        let result = ctx
            .handle_get_frame(&json!({
                "session_id": session_id,
                "frame_id": 1,
            }))
            .unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "main.rs");
        assert_eq!(files[0]["content"], "content version 1");
        assert_eq!(files[0]["hash"], hash1);

        // Frame 2: should see version 2 content
        let result = ctx
            .handle_get_frame(&json!({
                "session_id": session_id,
                "frame_id": 2,
            }))
            .unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["content"], "content version 2");
        assert_eq!(files[0]["hash"], hash2);
    }

    // ── test_handle_diff_frames (with real snapshots) ────────────────────

    #[test]
    fn test_handle_diff_frames() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        let session_id = "diff-sess";
        let session_dir = sessions_dir.join(session_id);
        std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();

        let meta = SessionMeta {
            id: session_id.to_string(),
            project_path: "/tmp".to_string(),
            started_at: 1_700_000_000,
            mode: SessionMode::Enriched,
            agents: Vec::new(),
        };
        std::fs::write(
            session_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();

        let store = SnapshotStore::new(session_dir.join("snapshots"));
        let hash1 = store.store(b"hello world\n").unwrap();
        let hash2 = store.store(b"hello rust\n").unwrap();

        let log = EditLog::new(session_dir.join("edits.jsonl"));
        let e1 = EditEvent {
            id: 1,
            ts: 1000,
            file: "greet.txt".to_string(),
            kind: EditKind::Create,
            patch: String::new(),
            before_hash: None,
            after_hash: hash1.clone(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };
        let e2 = EditEvent {
            id: 2,
            ts: 2000,
            file: "greet.txt".to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: Some(hash1),
            after_hash: hash2,
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
        };
        log.append(&e1).unwrap();
        log.append(&e2).unwrap();

        let ctx = HandlerContext::new(sessions_dir);
        let result = ctx
            .handle_diff_frames(&json!({
                "session_id": session_id,
                "frame_a": 1,
                "frame_b": 2,
            }))
            .unwrap();

        let diffs = result["diffs"].as_array().unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["path"], "greet.txt");
        let diff_text = diffs[0]["diff"].as_str().unwrap();
        assert!(diff_text.contains("-hello world"));
        assert!(diff_text.contains("+hello rust"));
    }

    // ── test_session_not_found ───────────────────────────────────────────

    #[test]
    fn test_session_not_found() {
        let dir = tempdir().unwrap();
        let ctx = HandlerContext::new(dir.path().to_path_buf());
        let err = ctx
            .handle_get_timeline(&json!({
                "session_id": "nonexistent-session",
            }))
            .unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("session not found"),
            "error should mention 'session not found', got: {}",
            msg
        );
    }

    // ── test_frame_out_of_range ──────────────────────────────────────────

    #[test]
    fn test_frame_out_of_range() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().to_path_buf();
        create_test_session(&sessions_dir, "small-sess", 5);

        let ctx = HandlerContext::new(sessions_dir);
        let err = ctx
            .handle_get_frame(&json!({
                "session_id": "small-sess",
                "frame_id": 99,
            }))
            .unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("5"),
            "error should mention the edit count '5', got: {}",
            msg
        );
        assert!(
            msg.contains("out of range"),
            "error should mention 'out of range', got: {}",
            msg
        );
    }
}
