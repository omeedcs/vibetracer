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
    pub intent: Option<String>,
    /// Name of the tool that triggered the edit (e.g. "cursor", "claude").
    pub tool: Option<String>,
    /// Number of lines added by this edit.
    pub lines_added: u32,
    /// Number of lines removed by this edit.
    pub lines_removed: u32,
}

/// Internal bus events routed between vibetracer components.
///
/// Not serializable — these exist only in-process.
pub enum BusEvent {
    /// A filesystem edit was detected and recorded.
    Edit(EditEvent),
    /// An AI hook enriched a pending edit with tool/intent metadata.
    HookEnrichment {
        file: String,
        tool: String,
        intent: Option<String>,
    },
    /// A checkpoint should be created now.
    Checkpoint,
    /// Advance one tick during session playback.
    PlaybackTick,
    /// A terminal input event forwarded from crossterm.
    Input(crossterm::event::Event),
    /// Signal all components to shut down.
    Quit,
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
}
