use crate::event::EditEvent;
use std::collections::HashMap;

/// Summary of changes between two points in a session.
#[derive(Debug, Clone)]
pub struct SessionDiff {
    /// Starting edit index (inclusive).
    pub from_edit: usize,
    /// Ending edit index (inclusive).
    pub to_edit: usize,
    /// Per-file change summaries, sorted by edit count descending.
    pub file_changes: Vec<FileDiffSummary>,
    /// Total lines added across all files.
    pub total_added: u32,
    /// Total lines removed across all files.
    pub total_removed: u32,
    /// Number of edits in the range.
    pub edit_count: usize,
}

/// Per-file summary within a session diff.
#[derive(Debug, Clone)]
pub struct FileDiffSummary {
    /// Relative file path.
    pub file: String,
    /// Number of edits touching this file.
    pub edits: usize,
    /// Total lines added in this file.
    pub lines_added: u32,
    /// Total lines removed in this file.
    pub lines_removed: u32,
    /// Unique agent labels that touched this file.
    pub agents: Vec<String>,
}

impl SessionDiff {
    /// Compute a session diff between two edit indices (inclusive on both ends).
    ///
    /// `from` and `to` are clamped to valid indices. If `from > to` they are
    /// swapped so the range always goes forward.
    pub fn compute(edits: &[EditEvent], from: usize, to: usize) -> Self {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        let hi = hi.min(edits.len().saturating_sub(1));
        let lo = lo.min(hi);

        if edits.is_empty() {
            return SessionDiff {
                from_edit: lo,
                to_edit: hi,
                file_changes: Vec::new(),
                total_added: 0,
                total_removed: 0,
                edit_count: 0,
            };
        }

        let slice = &edits[lo..=hi];

        // Accumulate per-file stats.
        struct FileAccum {
            edits: usize,
            lines_added: u32,
            lines_removed: u32,
            agents: Vec<String>,
        }

        let mut per_file: HashMap<String, FileAccum> = HashMap::new();
        let mut total_added: u32 = 0;
        let mut total_removed: u32 = 0;

        for edit in slice {
            total_added += edit.lines_added;
            total_removed += edit.lines_removed;

            let accum = per_file.entry(edit.file.clone()).or_insert_with(|| FileAccum {
                edits: 0,
                lines_added: 0,
                lines_removed: 0,
                agents: Vec::new(),
            });
            accum.edits += 1;
            accum.lines_added += edit.lines_added;
            accum.lines_removed += edit.lines_removed;

            if let Some(ref label) = edit.agent_label {
                if !accum.agents.contains(label) {
                    accum.agents.push(label.clone());
                }
            }
        }

        // Sort by edit count descending, then alphabetically for ties.
        let mut file_changes: Vec<FileDiffSummary> = per_file
            .into_iter()
            .map(|(file, acc)| FileDiffSummary {
                file,
                edits: acc.edits,
                lines_added: acc.lines_added,
                lines_removed: acc.lines_removed,
                agents: acc.agents,
            })
            .collect();
        file_changes.sort_by(|a, b| b.edits.cmp(&a.edits).then_with(|| a.file.cmp(&b.file)));

        SessionDiff {
            from_edit: lo,
            to_edit: hi,
            file_changes,
            total_added,
            total_removed,
            edit_count: slice.len(),
        }
    }

    /// Collect all unique agent labels and their total edit counts across the diff.
    pub fn agent_summary(&self) -> Vec<(String, usize)> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for fc in &self.file_changes {
            for agent in &fc.agents {
                // Count the file's edits attributed to this agent.
                // Since we only tracked presence per file (not exact count per agent),
                // we use the file edit count as a rough approximation.
                *counts.entry(agent.clone()).or_insert(0) += fc.edits;
            }
        }
        let mut result: Vec<(String, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EditEvent, EditKind};

    fn make_edit(id: u64, file: &str, added: u32, removed: u32, agent: Option<&str>) -> EditEvent {
        EditEvent {
            id,
            ts: 1_700_000_000_000 + (id as i64 * 1000),
            file: file.to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: Some("aaa".to_string()),
            after_hash: "bbb".to_string(),
            intent: None,
            tool: None,
            lines_added: added,
            lines_removed: removed,
            agent_id: agent.map(|a| a.to_string()),
            agent_label: agent.map(|a| a.to_string()),
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        }
    }

    #[test]
    fn compute_empty_edits() {
        let edits: Vec<EditEvent> = Vec::new();
        let diff = SessionDiff::compute(&edits, 0, 0);
        assert_eq!(diff.edit_count, 0);
        assert_eq!(diff.total_added, 0);
        assert_eq!(diff.total_removed, 0);
        assert!(diff.file_changes.is_empty());
    }

    #[test]
    fn compute_single_edit() {
        let edits = vec![make_edit(0, "src/main.rs", 10, 3, Some("claude-1"))];
        let diff = SessionDiff::compute(&edits, 0, 0);
        assert_eq!(diff.edit_count, 1);
        assert_eq!(diff.total_added, 10);
        assert_eq!(diff.total_removed, 3);
        assert_eq!(diff.file_changes.len(), 1);
        assert_eq!(diff.file_changes[0].file, "src/main.rs");
        assert_eq!(diff.file_changes[0].agents, vec!["claude-1"]);
    }

    #[test]
    fn compute_range_aggregates_per_file() {
        let edits = vec![
            make_edit(0, "src/main.rs", 5, 2, Some("claude-1")),
            make_edit(1, "src/auth.rs", 10, 4, Some("claude-1")),
            make_edit(2, "src/main.rs", 3, 1, Some("claude-2")),
            make_edit(3, "src/auth.rs", 7, 0, Some("claude-1")),
            make_edit(4, "src/config.rs", 2, 1, None),
        ];
        let diff = SessionDiff::compute(&edits, 0, 4);

        assert_eq!(diff.edit_count, 5);
        assert_eq!(diff.from_edit, 0);
        assert_eq!(diff.to_edit, 4);
        assert_eq!(diff.total_added, 27);
        assert_eq!(diff.total_removed, 8);

        // auth.rs has 2 edits, main.rs has 2 edits, config.rs has 1 edit.
        assert_eq!(diff.file_changes.len(), 3);

        // Sorted by edit count descending, then alphabetically.
        // auth.rs (2 edits) and main.rs (2 edits) tied -> sorted alphabetically.
        assert_eq!(diff.file_changes[0].file, "src/auth.rs");
        assert_eq!(diff.file_changes[0].edits, 2);
        assert_eq!(diff.file_changes[0].lines_added, 17);
        assert_eq!(diff.file_changes[0].lines_removed, 4);

        assert_eq!(diff.file_changes[1].file, "src/main.rs");
        assert_eq!(diff.file_changes[1].edits, 2);
        assert_eq!(diff.file_changes[1].lines_added, 8);
        assert_eq!(diff.file_changes[1].lines_removed, 3);

        assert_eq!(diff.file_changes[2].file, "src/config.rs");
        assert_eq!(diff.file_changes[2].edits, 1);
    }

    #[test]
    fn compute_swaps_reversed_range() {
        let edits = vec![
            make_edit(0, "a.rs", 1, 0, None),
            make_edit(1, "b.rs", 2, 0, None),
            make_edit(2, "c.rs", 3, 0, None),
        ];
        let diff = SessionDiff::compute(&edits, 2, 0);
        assert_eq!(diff.from_edit, 0);
        assert_eq!(diff.to_edit, 2);
        assert_eq!(diff.edit_count, 3);
    }

    #[test]
    fn compute_clamps_out_of_bounds() {
        let edits = vec![
            make_edit(0, "a.rs", 1, 0, None),
            make_edit(1, "b.rs", 2, 0, None),
        ];
        let diff = SessionDiff::compute(&edits, 0, 100);
        assert_eq!(diff.to_edit, 1);
        assert_eq!(diff.edit_count, 2);
    }

    #[test]
    fn compute_partial_range() {
        let edits = vec![
            make_edit(0, "a.rs", 1, 0, None),
            make_edit(1, "b.rs", 2, 0, None),
            make_edit(2, "c.rs", 3, 0, None),
            make_edit(3, "d.rs", 4, 0, None),
        ];
        let diff = SessionDiff::compute(&edits, 1, 2);
        assert_eq!(diff.edit_count, 2);
        assert_eq!(diff.total_added, 5);
        assert_eq!(diff.file_changes.len(), 2);
    }

    #[test]
    fn agents_tracked_per_file() {
        let edits = vec![
            make_edit(0, "src/lib.rs", 1, 0, Some("claude-1")),
            make_edit(1, "src/lib.rs", 2, 0, Some("claude-2")),
            make_edit(2, "src/lib.rs", 1, 0, Some("claude-1")), // duplicate agent
        ];
        let diff = SessionDiff::compute(&edits, 0, 2);
        assert_eq!(diff.file_changes.len(), 1);
        let fc = &diff.file_changes[0];
        assert_eq!(fc.agents.len(), 2);
        assert!(fc.agents.contains(&"claude-1".to_string()));
        assert!(fc.agents.contains(&"claude-2".to_string()));
    }

    #[test]
    fn agent_summary_aggregates() {
        let edits = vec![
            make_edit(0, "a.rs", 1, 0, Some("claude-1")),
            make_edit(1, "a.rs", 1, 0, Some("claude-1")),
            make_edit(2, "b.rs", 1, 0, Some("claude-2")),
        ];
        let diff = SessionDiff::compute(&edits, 0, 2);
        let summary = diff.agent_summary();
        // claude-1 has 2 edits (from a.rs), claude-2 has 1 edit (from b.rs)
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].0, "claude-1");
        assert_eq!(summary[0].1, 2);
        assert_eq!(summary[1].0, "claude-2");
        assert_eq!(summary[1].1, 1);
    }
}
