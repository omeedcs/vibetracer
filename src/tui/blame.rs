//! Blame view: per-line attribution showing which operation/agent last touched each line.

use crate::event::EditEvent;
use std::collections::HashMap;

/// Attribution for a single line in a file.
#[derive(Debug, Clone)]
pub struct LineBlame {
    /// Edit ID that last modified this line.
    pub edit_id: u64,
    /// Agent label (if known).
    pub agent_label: Option<String>,
    /// Operation intent (if known).
    pub operation_intent: Option<String>,
    /// Timestamp of the edit.
    pub timestamp: i64,
}

/// Compute per-line blame for a file at a given playhead position.
///
/// Walks through all edits for the file up to `playhead` and tracks
/// which edit last touched each line by parsing the unified diffs.
pub fn compute_blame(
    edits: &[EditEvent],
    file: &str,
    playhead: usize,
) -> HashMap<usize, LineBlame> {
    let mut blame: HashMap<usize, LineBlame> = HashMap::new();

    // Collect edits for this file up to playhead
    for edit in edits.iter().take(playhead + 1) {
        if edit.file != file {
            continue;
        }

        let attribution = LineBlame {
            edit_id: edit.id,
            agent_label: edit.agent_label.clone(),
            operation_intent: edit.operation_intent.clone(),
            timestamp: edit.ts,
        };

        // Parse the unified diff to find which lines were added/modified
        let mut new_line: usize = 0;

        for diff_line in edit.patch.lines() {
            if diff_line.starts_with("@@") {
                // Parse +start from @@ -old,count +new,count @@
                if let Some(plus_pos) = diff_line.find('+') {
                    let after_plus = &diff_line[plus_pos + 1..];
                    let num_str: String = after_plus
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(start) = num_str.parse::<usize>() {
                        new_line = start;
                    }
                }
            } else if diff_line.starts_with('+') {
                // This line was added/modified by this edit
                blame.insert(new_line, attribution.clone());
                new_line += 1;
            } else if diff_line.starts_with('-') {
                // Removed line: don't advance new-file counter
            } else {
                // Context line: advance counter
                new_line += 1;
            }
        }
    }

    blame
}

/// Format a blame annotation for display in the gutter.
/// Returns a short string like "op#2 claude" or "claude-1".
pub fn format_blame(blame: &LineBlame, max_width: usize) -> String {
    let mut parts = Vec::new();

    if let Some(ref agent) = blame.agent_label {
        parts.push(agent.clone());
    }

    if let Some(ref intent) = blame.operation_intent {
        // Truncate intent to fit
        let max_intent = max_width.saturating_sub(
            parts.iter().map(|p| p.len()).sum::<usize>() + parts.len() + 1,
        );
        if max_intent > 3 {
            let truncated: String = intent.chars().take(max_intent).collect();
            parts.push(truncated);
        }
    }

    if parts.is_empty() {
        format!("#{}", blame.edit_id)
    } else {
        let result = parts.join(" ");
        if result.len() > max_width {
            result.chars().take(max_width).collect()
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EditKind;

    fn make_edit(id: u64, file: &str, patch: &str, agent: Option<&str>) -> EditEvent {
        EditEvent {
            id,
            ts: 1000 + id as i64,
            file: file.to_string(),
            kind: EditKind::Modify,
            patch: patch.to_string(),
            before_hash: Some("aaa".to_string()),
            after_hash: "bbb".to_string(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 0,
            agent_id: agent.map(|s| s.to_string()),
            agent_label: agent.map(|s| s.to_string()),
            operation_id: None,
            operation_intent: Some("fix auth".to_string()),
            tool_name: None,
            restore_id: None,
        }
    }

    #[test]
    fn blame_tracks_added_lines() {
        let edits = vec![make_edit(
            1,
            "src/auth.rs",
            "@@ -1,3 +1,4 @@\n context\n+added line\n context\n context",
            Some("claude"),
        )];

        let blame = compute_blame(&edits, "src/auth.rs", 0);
        assert!(blame.contains_key(&2)); // line 2 was added
        assert_eq!(blame[&2].agent_label.as_deref(), Some("claude"));
    }

    #[test]
    fn blame_ignores_other_files() {
        let edits = vec![make_edit(
            1,
            "src/other.rs",
            "@@ -1,1 +1,2 @@\n+new",
            Some("claude"),
        )];

        let blame = compute_blame(&edits, "src/auth.rs", 0);
        assert!(blame.is_empty());
    }

    #[test]
    fn blame_latest_edit_wins() {
        let edits = vec![
            make_edit(1, "src/auth.rs", "@@ -1,1 +1,2 @@\n+first", Some("claude-1")),
            make_edit(2, "src/auth.rs", "@@ -1,1 +1,2 @@\n+second", Some("claude-2")),
        ];

        let blame = compute_blame(&edits, "src/auth.rs", 1);
        // Line 1 (the added line) should be attributed to the latest edit
        assert_eq!(blame[&1].agent_label.as_deref(), Some("claude-2"));
    }

    #[test]
    fn format_blame_with_agent() {
        let b = LineBlame {
            edit_id: 1,
            agent_label: Some("claude".to_string()),
            operation_intent: Some("fix auth".to_string()),
            timestamp: 1000,
        };
        let formatted = format_blame(&b, 20);
        assert!(formatted.contains("claude"));
    }

    #[test]
    fn format_blame_no_agent() {
        let b = LineBlame {
            edit_id: 42,
            agent_label: None,
            operation_intent: None,
            timestamp: 1000,
        };
        assert_eq!(format_blame(&b, 20), "#42");
    }
}
