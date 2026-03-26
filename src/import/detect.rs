use std::path::{Path, PathBuf};

/// Detected agent presence in a project directory.
#[derive(Debug, Clone)]
pub struct DetectedAgent {
    /// Agent name (e.g. "claude-code", "cursor")
    pub name: String,
    /// Path to the log file or directory
    pub log_path: PathBuf,
}

/// Scan a project directory for known agent log patterns.
///
/// Checks for:
/// - `.agent-trace/` directory (Cursor, Codex CLI)
/// - `~/.claude/projects/{converted}/` with .jsonl files (Claude Code)
pub fn detect_agents(project_path: &Path) -> Vec<DetectedAgent> {
    let mut detected = Vec::new();

    // Check for .agent-trace/ directory (Cursor/Codex)
    let agent_trace_dir = project_path.join(".agent-trace");
    if agent_trace_dir.is_dir() {
        detected.push(DetectedAgent {
            name: "cursor".to_string(),
            log_path: agent_trace_dir,
        });
    }

    // Check for Claude Code session files
    if let Some(home) = dirs::home_dir() {
        let converted = project_path.to_string_lossy().replace('/', "-");
        let claude_sessions_dir = home.join(".claude").join("projects").join(&converted);
        if claude_sessions_dir.is_dir() {
            let has_jsonl = std::fs::read_dir(&claude_sessions_dir)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .any(|e| {
                            e.path()
                                .extension()
                                .and_then(|ext| ext.to_str())
                                == Some("jsonl")
                        })
                })
                .unwrap_or(false);

            if has_jsonl {
                detected.push(DetectedAgent {
                    name: "claude-code".to_string(),
                    log_path: claude_sessions_dir,
                });
            }
        }
    }

    detected
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_agent_trace_dir() {
        let tmp = tempdir().unwrap();
        let trace_dir = tmp.path().join(".agent-trace");
        std::fs::create_dir(&trace_dir).unwrap();
        std::fs::write(trace_dir.join("session.json"), "{}").unwrap();

        let agents = detect_agents(tmp.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "cursor");
    }

    #[test]
    fn returns_empty_for_clean_dir() {
        let tmp = tempdir().unwrap();
        let agents = detect_agents(tmp.path());
        assert!(agents.is_empty());
    }
}
