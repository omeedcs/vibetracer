use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::event::EditEvent;

/// Format edit events as a git-ai compatible authorship log.
pub fn format_authorship_log(events: &[EditEvent]) -> String {
    let mut lines = Vec::new();
    for event in events {
        let agent = event.agent_id.as_deref().unwrap_or("unknown");
        let model = event.tool_name.as_deref().unwrap_or("unknown");
        let ts = chrono::DateTime::from_timestamp_millis(event.ts)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!(
            "{}  agent={}  model={}  ts={}  lines=+{}-{}",
            event.file, agent, model, ts, event.lines_added, event.lines_removed
        ));
    }
    lines.join("\n")
}

/// Check if git notes.rewriteRef is configured.
pub fn check_notes_config(project_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["config", "--get", "notes.rewriteRef"])
        .current_dir(project_path)
        .output()
        .context("run git config")?;
    Ok(output.status.success())
}

/// Attach authorship log as a git note to the specified commit.
pub fn attach_git_note(project_path: &Path, commit: &str, note_content: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["notes", "add", "-f", "-m", note_content, commit])
        .current_dir(project_path)
        .output()
        .context("run git notes add")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git notes add failed: {}", stderr);
    }
    Ok(())
}

/// Export edit events as git notes attached to a commit (default HEAD).
pub fn export_git_notes(
    events: &[EditEvent],
    project_path: &Path,
    commit: Option<&str>,
) -> Result<()> {
    if !check_notes_config(project_path)? {
        eprintln!(
            "warning: git notes.rewriteRef is not set. Notes will be orphaned on rebase.\n\
             \x20        Run: git config notes.rewriteRef refs/notes/commits"
        );
    }
    let log = format_authorship_log(events);
    let target = commit.unwrap_or("HEAD");
    attach_git_note(project_path, target, &log)?;
    let missing_reasoning = events.iter().filter(|e| e.intent.is_none()).count();
    eprintln!(
        "Attached {} events as git note to {} ({} missing reasoning context)",
        events.len(),
        target,
        missing_reasoning
    );
    Ok(())
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EditEvent, EditKind};

    fn sample_event(id: u64, file: &str, agent_id: Option<&str>) -> EditEvent {
        EditEvent {
            id,
            ts: 1_700_000_000_000,
            file: file.to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: None,
            after_hash: "abc".to_string(),
            intent: Some("fix bug".to_string()),
            tool: None,
            lines_added: 10,
            lines_removed: 3,
            agent_id: agent_id.map(|s| s.to_string()),
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: Some("Edit".to_string()),
            restore_id: None,
        }
    }

    #[test]
    fn format_authorship_log_basic() {
        let events = vec![
            sample_event(1, "src/main.rs", Some("agent-1")),
            sample_event(2, "src/lib.rs", Some("agent-2")),
        ];
        let log = format_authorship_log(&events);
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), 2);

        assert!(lines[0].starts_with("src/main.rs"));
        assert!(lines[0].contains("agent=agent-1"));
        assert!(lines[0].contains("model=Edit"));
        assert!(lines[0].contains("lines=+10-3"));

        assert!(lines[1].starts_with("src/lib.rs"));
        assert!(lines[1].contains("agent=agent-2"));
    }

    #[test]
    fn format_handles_missing_agent() {
        let events = vec![sample_event(1, "src/main.rs", None)];
        let log = format_authorship_log(&events);
        assert!(log.contains("agent=unknown"));
    }
}
