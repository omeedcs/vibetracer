use std::collections::HashMap;

use crate::event::EditEvent;

/// A group of edits that belong to the same logical operation.
#[derive(Debug, Clone)]
pub struct OperationGroup {
    pub operation_id: String,
    pub agent_id: Option<String>,
    pub agent_label: Option<String>,
    pub intent: Option<String>,
    pub edits: Vec<usize>,
    pub files_touched: Vec<String>,
    pub ts_start: i64,
    pub ts_end: i64,
}

/// Manages grouping of edits into operations.
pub struct OperationManager {
    groups: HashMap<String, OperationGroup>,
}

impl OperationManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Add an edit to the manager. If the edit has an `operation_id`, it will
    /// be grouped with other edits sharing that ID. Edits without an
    /// `operation_id` get a synthetic ID of the form `"ungrouped-{edit_index}"`.
    pub fn add_edit(&mut self, edit_index: usize, edit: &EditEvent) {
        let op_id = edit
            .operation_id
            .clone()
            .unwrap_or_else(|| format!("ungrouped-{edit_index}"));

        let group = self.groups.entry(op_id.clone()).or_insert_with(|| {
            OperationGroup {
                operation_id: op_id,
                agent_id: edit.agent_id.clone(),
                agent_label: edit.agent_label.clone(),
                intent: edit.operation_intent.clone(),
                edits: Vec::new(),
                files_touched: Vec::new(),
                ts_start: edit.ts,
                ts_end: edit.ts,
            }
        });

        group.edits.push(edit_index);

        if !group.files_touched.contains(&edit.file) {
            group.files_touched.push(edit.file.clone());
        }

        if edit.ts < group.ts_start {
            group.ts_start = edit.ts;
        }
        if edit.ts > group.ts_end {
            group.ts_end = edit.ts;
        }

        // Update agent info if the group didn't have it yet
        if group.agent_id.is_none() {
            group.agent_id = edit.agent_id.clone();
        }
        if group.agent_label.is_none() {
            group.agent_label = edit.agent_label.clone();
        }
        if group.intent.is_none() {
            group.intent = edit.operation_intent.clone();
        }
    }

    /// Return all groups sorted by `ts_start` (ascending).
    pub fn groups_ordered(&self) -> Vec<&OperationGroup> {
        let mut groups: Vec<&OperationGroup> = self.groups.values().collect();
        groups.sort_by_key(|g| g.ts_start);
        groups
    }

    /// Look up a group by its operation ID.
    pub fn get(&self, operation_id: &str) -> Option<&OperationGroup> {
        self.groups.get(operation_id)
    }

    /// Return the total number of groups.
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Whether there are no groups.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl Default for OperationManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EditEvent, EditKind};

    fn make_edit(id: u64, ts: i64, file: &str, op_id: Option<&str>) -> EditEvent {
        EditEvent {
            id,
            ts,
            file: file.to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: None,
            after_hash: "hash".to_string(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 0,
            agent_id: Some("agent-1".to_string()),
            agent_label: Some("claude-1".to_string()),
            operation_id: op_id.map(|s| s.to_string()),
            operation_intent: op_id.map(|_| "test intent".to_string()),
            tool_name: None,
            restore_id: None,
        }
    }

    #[test]
    fn grouped_edits_share_operation() {
        let mut mgr = OperationManager::new();
        let e0 = make_edit(1, 1000, "a.rs", Some("op-1"));
        let e1 = make_edit(2, 1500, "b.rs", Some("op-1"));
        let e2 = make_edit(3, 2000, "a.rs", Some("op-1"));

        mgr.add_edit(0, &e0);
        mgr.add_edit(1, &e1);
        mgr.add_edit(2, &e2);

        assert_eq!(mgr.len(), 1);

        let group = mgr.get("op-1").unwrap();
        assert_eq!(group.edits, vec![0, 1, 2]);
        assert_eq!(group.files_touched, vec!["a.rs", "b.rs"]);
        assert_eq!(group.ts_start, 1000);
        assert_eq!(group.ts_end, 2000);
        assert_eq!(group.agent_id, Some("agent-1".to_string()));
        assert_eq!(group.intent, Some("test intent".to_string()));
    }

    #[test]
    fn ungrouped_edits_get_synthetic_ids() {
        let mut mgr = OperationManager::new();
        let e0 = make_edit(1, 1000, "a.rs", None);
        let e1 = make_edit(2, 2000, "b.rs", None);

        mgr.add_edit(0, &e0);
        mgr.add_edit(1, &e1);

        assert_eq!(mgr.len(), 2);
        assert!(mgr.get("ungrouped-0").is_some());
        assert!(mgr.get("ungrouped-1").is_some());
    }

    #[test]
    fn groups_ordered_by_ts_start() {
        let mut mgr = OperationManager::new();
        let e0 = make_edit(1, 3000, "a.rs", Some("op-b"));
        let e1 = make_edit(2, 1000, "b.rs", Some("op-a"));
        let e2 = make_edit(3, 2000, "c.rs", None);

        mgr.add_edit(0, &e0);
        mgr.add_edit(1, &e1);
        mgr.add_edit(2, &e2);

        let ordered = mgr.groups_ordered();
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].operation_id, "op-a");
        assert_eq!(ordered[1].operation_id, "ungrouped-2");
        assert_eq!(ordered[2].operation_id, "op-b");
    }

    #[test]
    fn mixed_grouped_and_ungrouped() {
        let mut mgr = OperationManager::new();
        let e0 = make_edit(1, 1000, "a.rs", Some("op-1"));
        let e1 = make_edit(2, 1100, "a.rs", None);
        let e2 = make_edit(3, 1200, "b.rs", Some("op-1"));
        let e3 = make_edit(4, 1300, "c.rs", None);

        mgr.add_edit(0, &e0);
        mgr.add_edit(1, &e1);
        mgr.add_edit(2, &e2);
        mgr.add_edit(3, &e3);

        // op-1 has 2 edits, plus 2 ungrouped
        assert_eq!(mgr.len(), 3);

        let op1 = mgr.get("op-1").unwrap();
        assert_eq!(op1.edits, vec![0, 2]);
        assert_eq!(op1.files_touched, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn empty_manager() {
        let mgr = OperationManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
        assert!(mgr.groups_ordered().is_empty());
    }

    #[test]
    fn ts_range_tracks_min_max() {
        let mut mgr = OperationManager::new();
        // Add edits out of order
        let e0 = make_edit(1, 5000, "a.rs", Some("op-1"));
        let e1 = make_edit(2, 2000, "b.rs", Some("op-1"));
        let e2 = make_edit(3, 8000, "a.rs", Some("op-1"));

        mgr.add_edit(0, &e0);
        mgr.add_edit(1, &e1);
        mgr.add_edit(2, &e2);

        let group = mgr.get("op-1").unwrap();
        assert_eq!(group.ts_start, 2000);
        assert_eq!(group.ts_end, 8000);
    }
}
