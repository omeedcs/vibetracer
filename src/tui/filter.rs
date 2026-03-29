//! Search and filter system for narrowing edits across the entire UI.
//!
//! Supports a composable filter syntax:
//!   file:auth agent:claude-1 after:14:30 kind:modify lines>20 content:token
//!   bare text → fuzzy match across all fields

use crate::event::{EditEvent, EditKind};

/// A parsed filter predicate.
#[derive(Debug, Clone)]
pub enum FilterPredicate {
    /// Match file path substring.
    File(String),
    /// Match agent ID or label substring.
    Agent(String),
    /// Match edit kind.
    Kind(EditKind),
    /// Match tool name substring.
    Tool(String),
    /// Edits after this timestamp offset (HH:MM from session start, or edit ID).
    After(i64),
    /// Edits before this timestamp offset.
    Before(i64),
    /// Lines added+removed greater than threshold.
    LinesGreater(u32),
    /// Lines added+removed less than threshold.
    LinesLess(u32),
    /// Match operation intent substring.
    Operation(String),
    /// Grep through diff content.
    Content(String),
    /// Fuzzy match across all fields.
    Fuzzy(String),
}

/// A composed filter (all predicates must match = AND logic).
#[derive(Debug, Clone)]
pub struct Filter {
    pub predicates: Vec<FilterPredicate>,
    pub raw_input: String,
}

impl Filter {
    /// Parse a filter string into predicates.
    pub fn parse(input: &str, session_start_ms: i64) -> Self {
        let mut predicates = Vec::new();
        let raw_input = input.to_string();

        for token in input.split_whitespace() {
            if let Some(val) = token.strip_prefix("file:") {
                predicates.push(FilterPredicate::File(val.to_lowercase()));
            } else if let Some(val) = token.strip_prefix("agent:") {
                predicates.push(FilterPredicate::Agent(val.to_lowercase()));
            } else if let Some(val) = token.strip_prefix("kind:") {
                match val.to_lowercase().as_str() {
                    "create" => predicates.push(FilterPredicate::Kind(EditKind::Create)),
                    "modify" => predicates.push(FilterPredicate::Kind(EditKind::Modify)),
                    "delete" => predicates.push(FilterPredicate::Kind(EditKind::Delete)),
                    _ => predicates.push(FilterPredicate::Fuzzy(token.to_lowercase())),
                }
            } else if let Some(val) = token.strip_prefix("tool:") {
                predicates.push(FilterPredicate::Tool(val.to_lowercase()));
            } else if let Some(val) = token.strip_prefix("after:") {
                if let Some(ts) = parse_time_offset(val, session_start_ms) {
                    predicates.push(FilterPredicate::After(ts));
                }
            } else if let Some(val) = token.strip_prefix("before:") {
                if let Some(ts) = parse_time_offset(val, session_start_ms) {
                    predicates.push(FilterPredicate::Before(ts));
                }
            } else if let Some(val) = token.strip_prefix("lines>") {
                if let Ok(n) = val.parse::<u32>() {
                    predicates.push(FilterPredicate::LinesGreater(n));
                }
            } else if let Some(val) = token.strip_prefix("lines<") {
                if let Ok(n) = val.parse::<u32>() {
                    predicates.push(FilterPredicate::LinesLess(n));
                }
            } else if let Some(val) = token.strip_prefix("op:") {
                predicates.push(FilterPredicate::Operation(val.to_lowercase()));
            } else if let Some(val) = token.strip_prefix("content:") {
                predicates.push(FilterPredicate::Content(val.to_lowercase()));
            } else {
                predicates.push(FilterPredicate::Fuzzy(token.to_lowercase()));
            }
        }

        Filter {
            predicates,
            raw_input,
        }
    }

    /// Test whether an edit matches all predicates.
    pub fn matches(&self, edit: &EditEvent) -> bool {
        self.predicates.iter().all(|p| predicate_matches(p, edit))
    }

    /// Return true if this filter has no predicates (matches everything).
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

/// Test a single predicate against an edit.
fn predicate_matches(pred: &FilterPredicate, edit: &EditEvent) -> bool {
    match pred {
        FilterPredicate::File(s) => edit.file.to_lowercase().contains(s),

        FilterPredicate::Agent(s) => {
            edit.agent_id
                .as_ref()
                .map(|a| a.to_lowercase().contains(s))
                .unwrap_or(false)
                || edit
                    .agent_label
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(s))
                    .unwrap_or(false)
        }

        FilterPredicate::Kind(k) => edit.kind == *k,

        FilterPredicate::Tool(s) => edit
            .tool_name
            .as_ref()
            .map(|t| t.to_lowercase().contains(s))
            .unwrap_or(false),

        FilterPredicate::After(ts) => edit.ts >= *ts,
        FilterPredicate::Before(ts) => edit.ts <= *ts,

        FilterPredicate::LinesGreater(n) => (edit.lines_added + edit.lines_removed) > *n,
        FilterPredicate::LinesLess(n) => (edit.lines_added + edit.lines_removed) < *n,

        FilterPredicate::Operation(s) => edit
            .operation_intent
            .as_ref()
            .map(|o| o.to_lowercase().contains(s))
            .unwrap_or(false),

        FilterPredicate::Content(s) => edit.patch.to_lowercase().contains(s),

        FilterPredicate::Fuzzy(s) => {
            // Search across all text fields
            edit.file.to_lowercase().contains(s)
                || edit.patch.to_lowercase().contains(s)
                || edit
                    .intent
                    .as_ref()
                    .map(|i| i.to_lowercase().contains(s))
                    .unwrap_or(false)
                || edit
                    .operation_intent
                    .as_ref()
                    .map(|o| o.to_lowercase().contains(s))
                    .unwrap_or(false)
                || edit
                    .agent_label
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(s))
                    .unwrap_or(false)
                || edit
                    .tool_name
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(s))
                    .unwrap_or(false)
        }
    }
}

/// Parse a time offset string "HH:MM" or "MM:SS" or a raw edit ID into a timestamp.
fn parse_time_offset(val: &str, session_start_ms: i64) -> Option<i64> {
    // Try as edit ID first
    if let Ok(id) = val.parse::<u64>() {
        // Treat as edit ID — caller will need to resolve this
        return Some(id as i64);
    }

    // Try as HH:MM
    let parts: Vec<&str> = val.split(':').collect();
    if parts.len() == 2 {
        if let (Ok(h), Ok(m)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
            let offset_ms = (h * 3600 + m * 60) * 1000;
            return Some(session_start_ms + offset_ms);
        }
    }

    None
}

/// Compute which edit indices match the current filter.
pub fn compute_matching_indices(edits: &[EditEvent], filter: &Filter) -> Vec<bool> {
    edits.iter().map(|e| filter.matches(e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edit(file: &str, kind: EditKind) -> EditEvent {
        EditEvent {
            id: 1,
            ts: 1000,
            file: file.to_string(),
            kind,
            patch: String::new(),
            before_hash: None,
            after_hash: String::new(),
            intent: None,
            tool: None,
            lines_added: 5,
            lines_removed: 2,
            agent_id: Some("claude-1".to_string()),
            agent_label: Some("claude".to_string()),
            operation_id: None,
            operation_intent: Some("fix auth".to_string()),
            tool_name: Some("Edit".to_string()),
            restore_id: None,
        }
    }

    #[test]
    fn filter_file() {
        let f = Filter::parse("file:auth", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Modify)));
        assert!(!f.matches(&make_edit("src/main.rs", EditKind::Modify)));
    }

    #[test]
    fn filter_agent() {
        let f = Filter::parse("agent:claude", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Modify)));
    }

    #[test]
    fn filter_kind() {
        let f = Filter::parse("kind:create", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Create)));
        assert!(!f.matches(&make_edit("src/auth.rs", EditKind::Modify)));
    }

    #[test]
    fn filter_lines() {
        let f = Filter::parse("lines>3", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Modify))); // 5+2=7 > 3
    }

    #[test]
    fn filter_fuzzy() {
        let f = Filter::parse("auth", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Modify)));
    }

    #[test]
    fn filter_compose() {
        let f = Filter::parse("file:auth agent:claude", 0);
        assert!(f.matches(&make_edit("src/auth.rs", EditKind::Modify)));
        assert!(!f.matches(&make_edit("src/main.rs", EditKind::Modify)));
    }

    #[test]
    fn filter_empty() {
        let f = Filter::parse("", 0);
        assert!(f.is_empty());
        assert!(f.matches(&make_edit("anything", EditKind::Modify)));
    }
}
