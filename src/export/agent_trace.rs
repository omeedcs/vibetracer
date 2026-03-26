use anyhow::Result;
use std::path::Path;

use crate::event::EditEvent;

/// Export edit events as Agent Trace JSON string.
pub fn export_agent_trace(events: &[EditEvent], _session_id: &str) -> Result<String> {
    let mut contributions = Vec::new();
    for event in events {
        let contrib = serde_json::json!({
            "agent": event.agent_id,
            "model": event.tool_name,
            "timestamp": chrono::DateTime::from_timestamp_millis(event.ts)
                .map(|dt| dt.to_rfc3339()),
            "file": event.file,
            "diff": if event.patch.is_empty() { None } else { Some(&event.patch) },
            "reasoning": event.intent,
            "operation_id": event.operation_id,
        });
        contributions.push(contrib);
    }
    let output = serde_json::json!({
        "version": "0.1",
        "generator": "vibetracer",
        "contributions": contributions,
    });
    serde_json::to_string_pretty(&output).map_err(Into::into)
}

/// Export and write to a file path or stdout.
pub fn export_agent_trace_to_path(
    events: &[EditEvent],
    session_id: &str,
    output: Option<&Path>,
) -> Result<()> {
    let json = export_agent_trace(events, session_id)?;
    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            let missing_reasoning = events.iter().filter(|e| e.intent.is_none()).count();
            eprintln!(
                "Exported {} events to {} ({} missing reasoning context)",
                events.len(),
                path.display(),
                missing_reasoning
            );
        }
        None => {
            println!("{}", json);
        }
    }
    Ok(())
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EditEvent, EditKind};

    fn sample_event(id: u64) -> EditEvent {
        EditEvent {
            id,
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
            agent_id: Some("agent-1".to_string()),
            agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-1".to_string()),
            operation_intent: Some("refactor auth".to_string()),
            tool_name: Some("Edit".to_string()),
            restore_id: None,
        }
    }

    #[test]
    fn export_produces_valid_json() {
        let events = vec![sample_event(1), sample_event(2)];
        let json_str = export_agent_trace(&events, "test-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(value["version"], "0.1");
        assert_eq!(value["generator"], "vibetracer");

        let contributions = value["contributions"].as_array().unwrap();
        assert_eq!(contributions.len(), 2);

        let first = &contributions[0];
        assert_eq!(first["file"], "src/main.rs");
        assert_eq!(first["agent"], "agent-1");
        assert_eq!(first["model"], "Edit");
        assert_eq!(first["reasoning"], "fix bug");
        assert!(first["timestamp"].as_str().is_some());
        assert!(first["diff"].as_str().is_some());
    }

    #[test]
    fn export_handles_missing_fields() {
        let event = EditEvent {
            id: 1,
            ts: 1_700_000_000_000,
            file: "src/lib.rs".to_string(),
            kind: EditKind::Create,
            patch: String::new(),
            before_hash: None,
            after_hash: "abc".to_string(),
            intent: None,
            tool: None,
            lines_added: 5,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };
        let json_str = export_agent_trace(&[event], "test-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let contributions = value["contributions"].as_array().unwrap();
        assert_eq!(contributions.len(), 1);

        let first = &contributions[0];
        assert!(first["agent"].is_null());
        assert!(first["model"].is_null());
        assert!(first["reasoning"].is_null());
        assert!(first["diff"].is_null());
    }
}
