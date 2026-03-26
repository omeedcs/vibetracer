use anyhow::{Context, Result};
use chrono::DateTime;
use serde::Deserialize;
use std::path::Path;

use crate::event::{EditEvent, EditKind};
use crate::import::traits::AgentImporter;
use crate::watcher::differ::compute_diff;

// ─── Serde structs ────────────────────────────────────────────────────────────

/// A single contribution entry in an agent trace file.
#[derive(Debug, Deserialize)]
pub struct TraceContribution {
    /// Agent identifier (e.g. "cursor", "codex").
    pub agent: Option<String>,
    /// Model name used for this contribution.
    pub model: Option<String>,
    /// RFC 3339 timestamp string.
    pub timestamp: Option<String>,
    /// Relative or absolute path of the file affected.
    pub file: Option<String>,
    /// File content before the edit.
    pub before: Option<String>,
    /// File content after the edit.
    pub after: Option<String>,
    /// Pre-computed unified diff patch. Used when before/after are not provided.
    pub diff: Option<String>,
    /// Free-form text describing the reasoning behind this contribution.
    pub reasoning: Option<String>,
    /// Identifier grouping multiple contributions under one logical operation.
    pub operation_id: Option<String>,
}

/// Top-level structure of an agent trace JSON file.
#[derive(Debug, Deserialize)]
pub struct TraceFile {
    /// Optional format version string.
    pub version: Option<String>,
    /// List of contributions recorded in this trace.
    pub contributions: Vec<TraceContribution>,
}

// ─── Importer ────────────────────────────────────────────────────────────────

/// Batch importer for the Agent Trace JSON format used by Cursor and Codex CLI.
pub struct AgentTraceImporter {
    pub agent_name_override: Option<String>,
}

impl AgentTraceImporter {
    /// Create a new importer with no agent name override.
    pub fn new() -> Self {
        Self {
            agent_name_override: None,
        }
    }

    /// Create a new importer that overrides the agent name in all imported events.
    pub fn with_agent_name(name: &str) -> Self {
        Self {
            agent_name_override: Some(name.to_string()),
        }
    }

    /// Parse an RFC 3339 timestamp string to Unix milliseconds. Returns 0 on failure.
    fn parse_ts(ts_str: &str) -> i64 {
        DateTime::parse_from_rfc3339(ts_str)
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0)
    }

    /// Process a single `TraceFile` and append `EditEvent`s to `out`.
    fn collect_events(
        &self,
        trace: TraceFile,
        project_root: &Path,
        id_counter: &mut u64,
        out: &mut Vec<EditEvent>,
    ) {
        for contrib in trace.contributions {
            // Skip entries without a file path.
            let file_str = match &contrib.file {
                Some(f) if !f.is_empty() => f.clone(),
                _ => continue,
            };

            // Compute relative file path.
            let relative_file = {
                let p = std::path::PathBuf::from(&file_str);
                if let Ok(rel) = p.strip_prefix(project_root) {
                    rel.to_string_lossy().to_string()
                } else {
                    file_str.clone()
                }
            };

            let ts = contrib
                .timestamp
                .as_deref()
                .map(Self::parse_ts)
                .unwrap_or(0);

            // Determine diff / line counts.
            let (patch, lines_added, lines_removed, kind) = match (&contrib.before, &contrib.after)
            {
                (_, Some(after)) => {
                    let before_str = contrib.before.as_deref().unwrap_or("");
                    let diff = compute_diff(before_str, after, &relative_file);
                    let kind = if contrib.before.is_none() || contrib.before.as_deref() == Some("")
                    {
                        EditKind::Create
                    } else {
                        EditKind::Modify
                    };
                    (diff.patch, diff.lines_added, diff.lines_removed, kind)
                }
                _ => {
                    // Fall back to the provided diff field.
                    let patch = contrib.diff.clone().unwrap_or_default();
                    let (la, lr) = count_diff_lines(&patch);
                    (patch, la, lr, EditKind::Modify)
                }
            };

            // Determine agent_id: prefer override, then contribution field, then default.
            let agent_id = self
                .agent_name_override
                .clone()
                .or_else(|| contrib.agent.clone())
                .or_else(|| Some("agent-trace".to_string()));

            let event = EditEvent {
                id: *id_counter,
                ts,
                file: relative_file,
                kind,
                patch,
                before_hash: None,
                after_hash: String::new(),
                intent: contrib.reasoning.clone(),
                tool: agent_id.clone(),
                lines_added,
                lines_removed,
                agent_id,
                agent_label: contrib.agent.clone(),
                operation_id: contrib.operation_id.clone(),
                operation_intent: contrib.reasoning.clone(),
                tool_name: None,
                restore_id: None,
            };

            out.push(event);
            *id_counter += 1;
        }
    }
}

impl Default for AgentTraceImporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Count `+` and `-` lines in a unified diff patch.
fn count_diff_lines(patch: &str) -> (u32, u32) {
    let mut added: u32 = 0;
    let mut removed: u32 = 0;
    for line in patch.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
        }
    }
    (added, removed)
}

impl AgentImporter for AgentTraceImporter {
    fn agent_name(&self) -> &str {
        self.agent_name_override.as_deref().unwrap_or("agent-trace")
    }

    fn format_version(&self) -> Option<&str> {
        Some("0.1")
    }

    fn can_import(&self, path: &Path) -> bool {
        // Accept a directory named ".agent-trace"
        if path.is_dir() {
            return path.file_name().and_then(|n| n.to_str()) == Some(".agent-trace");
        }
        // Accept any file with a .json extension
        path.extension().and_then(|e| e.to_str()) == Some("json")
    }

    fn import_edits(&self, path: &Path, project_root: &Path) -> Result<Vec<EditEvent>> {
        let json_files: Vec<std::path::PathBuf> = if path.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(path)
                .with_context(|| format!("read directory {:?}", path))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
                .collect();
            entries.sort();
            entries
        } else {
            vec![path.to_path_buf()]
        };

        let mut events: Vec<EditEvent> = Vec::new();
        let mut id_counter: u64 = 1;

        for file_path in &json_files {
            let raw = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!("agent_trace: could not read {:?}: {}", file_path, err);
                    continue;
                }
            };

            let trace: TraceFile = match serde_json::from_str(&raw) {
                Ok(t) => t,
                Err(err) => {
                    tracing::warn!(
                        "agent_trace: skipping malformed file {:?}: {}",
                        file_path,
                        err
                    );
                    continue;
                }
            };

            self.collect_events(trace, project_root, &mut id_counter, &mut events);
        }

        events.sort_by_key(|e| e.ts);
        Ok(events)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_trace_json(contributions: &str) -> String {
        format!(r#"{{"version":"0.1","contributions":[{}]}}"#, contributions)
    }

    #[test]
    fn can_import_agent_trace_dir() {
        let tmp = tempdir().unwrap();
        let trace_dir = tmp.path().join(".agent-trace");
        std::fs::create_dir_all(&trace_dir).unwrap();

        let importer = AgentTraceImporter::new();
        assert!(importer.can_import(&trace_dir));
        // A directory with a different name should not match.
        assert!(!importer.can_import(tmp.path()));
    }

    #[test]
    fn can_import_json_file() {
        let tmp = tempdir().unwrap();
        let json_file = tmp.path().join("trace.json");
        std::fs::write(&json_file, "{}").unwrap();

        let importer = AgentTraceImporter::new();
        assert!(importer.can_import(&json_file));

        // Non-json extensions should not match.
        let txt_file = tmp.path().join("trace.txt");
        std::fs::write(&txt_file, "").unwrap();
        assert!(!importer.can_import(&txt_file));
    }

    #[test]
    fn import_single_trace_file() {
        let tmp = tempdir().unwrap();
        let project_root = tmp.path();

        let json = make_trace_json(
            r#"{"agent":"cursor","model":"gpt-4o","timestamp":"2024-01-15T10:00:00Z","file":"src/main.rs","before":"fn main() {}\n","after":"fn main() {\n    println!(\"hello\");\n}\n","reasoning":"Add hello world"}"#,
        );

        let trace_file = tmp.path().join("trace.json");
        std::fs::write(&trace_file, &json).unwrap();

        let importer = AgentTraceImporter::new();
        let events = importer.import_edits(&trace_file, project_root).unwrap();

        assert_eq!(events.len(), 1);
        let ev = &events[0];
        assert_eq!(ev.file, "src/main.rs");
        assert_eq!(ev.kind, EditKind::Modify);
        assert!(ev.lines_added > 0);
        assert_eq!(ev.intent.as_deref(), Some("Add hello world"));
        assert!(ev.ts > 0);
    }

    #[test]
    fn import_directory_of_trace_files() {
        let tmp = tempdir().unwrap();
        let trace_dir = tmp.path().join(".agent-trace");
        std::fs::create_dir_all(&trace_dir).unwrap();
        let project_root = tmp.path();

        // Write two files; they'll be read in alphabetical order.
        let json_a = make_trace_json(
            r#"{"file":"a.rs","before":"","after":"fn a() {}\n","timestamp":"2024-01-15T10:00:00Z"}"#,
        );
        let json_b = make_trace_json(
            r#"{"file":"b.rs","before":"","after":"fn b() {}\n","timestamp":"2024-01-15T11:00:00Z"}"#,
        );
        std::fs::write(trace_dir.join("a.json"), &json_a).unwrap();
        std::fs::write(trace_dir.join("b.json"), &json_b).unwrap();

        let importer = AgentTraceImporter::new();
        let events = importer.import_edits(&trace_dir, project_root).unwrap();

        assert_eq!(events.len(), 2);
        // Result sorted by timestamp: a.rs before b.rs.
        assert_eq!(events[0].file, "a.rs");
        assert_eq!(events[1].file, "b.rs");
    }

    #[test]
    fn agent_name_override() {
        let tmp = tempdir().unwrap();
        let project_root = tmp.path();

        let json = make_trace_json(
            r#"{"agent":"cursor","file":"foo.rs","before":"","after":"// hi\n","timestamp":"2024-01-15T10:00:00Z"}"#,
        );
        let trace_file = tmp.path().join("trace.json");
        std::fs::write(&trace_file, &json).unwrap();

        let importer = AgentTraceImporter::with_agent_name("my-custom-agent");
        assert_eq!(importer.agent_name(), "my-custom-agent");

        let events = importer.import_edits(&trace_file, project_root).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].agent_id.as_deref(), Some("my-custom-agent"));
    }

    #[test]
    fn skips_malformed_files_gracefully() {
        let tmp = tempdir().unwrap();
        let trace_dir = tmp.path().join(".agent-trace");
        std::fs::create_dir_all(&trace_dir).unwrap();
        let project_root = tmp.path();

        // Bad JSON — should be skipped without panicking.
        std::fs::write(trace_dir.join("bad.json"), "not json at all {{{{").unwrap();

        // Good JSON — should be imported.
        let good = make_trace_json(
            r#"{"file":"good.rs","before":"","after":"// ok\n","timestamp":"2024-01-15T10:00:00Z"}"#,
        );
        std::fs::write(trace_dir.join("good.json"), &good).unwrap();

        let importer = AgentTraceImporter::new();
        let events = importer.import_edits(&trace_dir, project_root).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].file, "good.rs");
    }

    #[test]
    fn handles_missing_optional_fields() {
        let tmp = tempdir().unwrap();
        let project_root = tmp.path();

        // Minimal contribution: only `file` and `diff`.
        let json = r#"{
            "contributions": [
                {
                    "file": "minimal.rs",
                    "diff": "@@ -1 +1 @@\n-old\n+new\n"
                }
            ]
        }"#;

        let trace_file = tmp.path().join("minimal.json");
        std::fs::write(&trace_file, json).unwrap();

        let importer = AgentTraceImporter::new();
        let events = importer.import_edits(&trace_file, project_root).unwrap();

        assert_eq!(events.len(), 1);
        let ev = &events[0];
        assert_eq!(ev.file, "minimal.rs");
        assert_eq!(ev.ts, 0); // no timestamp → defaults to 0
        assert!(!ev.patch.is_empty());
        assert_eq!(ev.lines_added, 1);
        assert_eq!(ev.lines_removed, 1);
    }
}
