use serde::{Deserialize, Serialize};

/// The kind of filesystem edit that occurred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    Create,
    Modify,
    Delete,
}

/// A single edit event captured by the watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditEvent {
    /// Monotonically increasing event ID within a session.
    pub id: u64,
    /// Unix timestamp (milliseconds) when the event was recorded.
    pub ts: i64,
    /// Relative path of the file that was edited.
    pub file: String,
    /// Whether the file was created, modified, or deleted.
    pub kind: EditKind,
    /// Unified diff patch string.
    pub patch: String,
    /// SHA-256 hex digest of the file content before the edit, if available.
    pub before_hash: Option<String>,
    /// SHA-256 hex digest of the file content after the edit.
    pub after_hash: String,
    /// Free-form intent description (e.g. from an AI hook).
    #[serde(default)]
    pub intent: Option<String>,
    /// Name of the tool that triggered the edit (e.g. "cursor", "claude").
    #[serde(default)]
    pub tool: Option<String>,
    /// Number of lines added by this edit.
    pub lines_added: u32,
    /// Number of lines removed by this edit.
    pub lines_removed: u32,
    /// Identifier of the agent that produced this edit (e.g. Claude instance ID).
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Human-readable label for the agent (e.g. "claude-1").
    #[serde(default)]
    pub agent_label: Option<String>,
    /// Identifier grouping multiple edits under one logical operation.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// Human-readable description of the operation's intent.
    #[serde(default)]
    pub operation_intent: Option<String>,
    /// Name of the tool invocation that produced this edit (e.g. "Edit", "Write").
    #[serde(default)]
    pub tool_name: Option<String>,
    /// ID of a snapshot to restore from, used by the restore workflow.
    #[serde(default)]
    pub restore_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreEvent {
    pub id: u64,
    pub ts: i64,
    pub scope: RestoreScope,
    pub files_restored: Vec<RestoreFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreFileEntry {
    pub path: String,
    pub from_hash: String,
    pub to_hash: String, // empty string = file was deleted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RestoreScope {
    File { path: String, target_edit_id: u64 },
    Operation { operation_id: String },
    AgentRange { agent_id: String, from_ts: i64, to_ts: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub agent_label: String,
    pub tool_type: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub edit_count: u64,
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_event_serialization() {
        let event = EditEvent {
            id: 1,
            ts: 1_700_000_000_000,
            file: "src/main.rs".to_string(),
            kind: EditKind::Modify,
            patch: "@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
            before_hash: Some("abc123".to_string()),
            after_hash: "def456".to_string(),
            intent: Some("fix bug".to_string()),
            tool: Some("cursor".to_string()),
            lines_added: 1,
            lines_removed: 1,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };

        let json = serde_json::to_string(&event).expect("serialize");
        let restored: EditEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.id, 1);
        assert_eq!(restored.ts, 1_700_000_000_000);
        assert_eq!(restored.file, "src/main.rs");
        assert_eq!(restored.kind, EditKind::Modify);
        assert_eq!(restored.patch, "@@ -1,1 +1,1 @@\n-old\n+new");
        assert_eq!(restored.before_hash, Some("abc123".to_string()));
        assert_eq!(restored.after_hash, "def456");
        assert_eq!(restored.intent, Some("fix bug".to_string()));
        assert_eq!(restored.tool, Some("cursor".to_string()));
        assert_eq!(restored.lines_added, 1);
        assert_eq!(restored.lines_removed, 1);
    }

    #[test]
    fn test_edit_event_v2_fields_serialize() {
        let event = EditEvent {
            id: 1,
            ts: 1_700_000_000_000,
            file: "src/main.rs".to_string(),
            kind: EditKind::Modify,
            patch: "@@ -1 +1 @@\n-old\n+new".to_string(),
            before_hash: Some("abc".to_string()),
            after_hash: "def".to_string(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 1,
            agent_id: Some("12345".to_string()),
            agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-7".to_string()),
            operation_intent: Some("refactor auth".to_string()),
            tool_name: Some("Edit".to_string()),
            restore_id: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: EditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.agent_id, Some("12345".to_string()));
        assert_eq!(restored.operation_id, Some("op-7".to_string()));
        assert_eq!(restored.restore_id, None);
    }

    #[test]
    fn test_v1_json_deserializes_with_defaults() {
        let v1_json = r#"{"id":1,"ts":1700000000000,"file":"src/main.rs","kind":"modify","patch":"","before_hash":"abc","after_hash":"def","intent":"fix bug","tool":"cursor","lines_added":1,"lines_removed":1}"#;
        let event: EditEvent = serde_json::from_str(v1_json).unwrap();
        assert_eq!(event.agent_id, None);
        assert_eq!(event.operation_id, None);
        assert_eq!(event.restore_id, None);
    }

    #[test]
    fn test_edit_kind_variants() {
        let create = EditKind::Create;
        let modify = EditKind::Modify;
        let delete = EditKind::Delete;

        // Each variant has a distinct Debug output
        let create_dbg = format!("{:?}", create);
        let modify_dbg = format!("{:?}", modify);
        let delete_dbg = format!("{:?}", delete);

        assert_ne!(create_dbg, modify_dbg);
        assert_ne!(modify_dbg, delete_dbg);
        assert_ne!(create_dbg, delete_dbg);

        // Confirm the names are as expected
        assert_eq!(create_dbg, "Create");
        assert_eq!(modify_dbg, "Modify");
        assert_eq!(delete_dbg, "Delete");

        // Confirm serde rename_all = "snake_case"
        assert_eq!(
            serde_json::to_string(&EditKind::Create).unwrap(),
            "\"create\""
        );
        assert_eq!(
            serde_json::to_string(&EditKind::Modify).unwrap(),
            "\"modify\""
        );
        assert_eq!(
            serde_json::to_string(&EditKind::Delete).unwrap(),
            "\"delete\""
        );
    }

    #[test]
    fn test_restore_event_serialization() {
        let event = RestoreEvent {
            id: 1,
            ts: 1_700_000_000_000,
            scope: RestoreScope::File {
                path: "src/main.rs".to_string(),
                target_edit_id: 42,
            },
            files_restored: vec![RestoreFileEntry {
                path: "src/main.rs".to_string(),
                from_hash: "abc".to_string(),
                to_hash: "def".to_string(),
            }],
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: RestoreEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, 1);
        match restored.scope {
            RestoreScope::File { target_edit_id, .. } => assert_eq!(target_edit_id, 42),
            _ => panic!("wrong scope"),
        }
    }

    #[test]
    fn test_restore_scope_variants() {
        let op = RestoreScope::Operation { operation_id: "op-1".to_string() };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"type\":\"operation\""));

        let range = RestoreScope::AgentRange {
            agent_id: "a1".to_string(),
            from_ts: 1000,
            to_ts: 2000,
        };
        let json = serde_json::to_string(&range).unwrap();
        assert!(json.contains("\"type\":\"agent_range\""));
    }

    #[test]
    fn test_agent_info_serialization() {
        let info = AgentInfo {
            agent_id: "pid-123".to_string(),
            agent_label: "claude-1".to_string(),
            tool_type: "claude-code".to_string(),
            first_seen: 1000,
            last_seen: 2000,
            edit_count: 5,
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: AgentInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.agent_label, "claude-1");
        assert_eq!(restored.edit_count, 5);
    }
}
