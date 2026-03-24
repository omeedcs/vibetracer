use anyhow::{Context, Result};
use chrono::DateTime;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::event::{EditEvent, EditKind};
use crate::watcher::differ::compute_diff;

/// Metadata about a Claude Code session discovered on disk.
#[derive(Debug, Clone)]
pub struct ClaudeSession {
    pub id: String,
    pub project_path: String,
    pub started_at: i64,
    pub edit_count: usize,
}

/// Convert a filesystem path to Claude's hyphen-separated directory name.
/// e.g. `/Users/foo/bar` -> `-Users-foo-bar`
fn path_to_claude_dir(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Replace path separators with hyphens, leading `/` becomes leading `-`
    s.replace('/', "-")
}

/// List all Claude Code sessions for the given project path.
///
/// Looks in `~/.claude/projects/{converted_path}/` for `*.jsonl` files,
/// counts Edit/Write tool uses in each, and returns them sorted newest-first.
pub fn list_sessions(project_path: &Path) -> Result<Vec<ClaudeSession>> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let claude_dir = home.join(".claude").join("projects");
    let converted = path_to_claude_dir(project_path);
    let sessions_dir = claude_dir.join(&converted);

    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    let entries = std::fs::read_dir(&sessions_dir)
        .with_context(|| format!("read directory {:?}", sessions_dir))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let (started_at, edit_count) = quick_scan_session(&path).unwrap_or((0, 0));

        sessions.push(ClaudeSession {
            id: file_stem,
            project_path: project_path.to_string_lossy().to_string(),
            started_at,
            edit_count,
        });
    }

    // Sort newest-first
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(sessions)
}

/// Quickly scan a JSONL session file to count Edit/Write tool uses and get first timestamp.
fn quick_scan_session(path: &Path) -> Result<(i64, usize)> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    let mut started_at: i64 = 0;
    let mut edit_count: usize = 0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(val): Result<Value, _> = serde_json::from_str(trimmed) else {
            continue;
        };

        // Extract first timestamp
        if started_at == 0 {
            if let Some(ts_str) = val.get("timestamp").and_then(|v| v.as_str()) {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                    started_at = dt.timestamp_millis();
                }
            }
        }

        // Count Edit/Write tool uses
        if val.get("type").and_then(|v| v.as_str()) == Some("assistant") {
            if let Some(content) = val
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for item in content {
                    if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        if name == "Edit" || name == "Write" {
                            edit_count += 1;
                        }
                    }
                }
            }
        }
    }

    Ok((started_at, edit_count))
}

/// Parse an ISO 8601 timestamp string to unix timestamp in milliseconds.
fn parse_ts(ts_str: &str) -> i64 {
    DateTime::parse_from_rfc3339(ts_str)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

/// Extract intent text from an assistant message's text content blocks.
fn extract_intent(message: &Value) -> Option<String> {
    let content = message.get("content")?.as_array()?;
    for item in content {
        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    // Return first non-empty text block, truncated for intent
                    let intent = trimmed.lines().next().unwrap_or(trimmed);
                    return Some(intent.to_string());
                }
            }
        }
    }
    None
}

/// Import all Edit/Write events from a Claude Code JSONL session file.
///
/// Returns a `Vec<EditEvent>` sorted by timestamp.
pub fn import_session(jsonl_path: &Path, project_root: &Path) -> Result<Vec<EditEvent>> {
    let file = std::fs::File::open(jsonl_path)
        .with_context(|| format!("open session file {:?}", jsonl_path))?;
    let reader = std::io::BufReader::new(file);

    let lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .collect();

    let mut events: Vec<EditEvent> = Vec::new();
    let mut id_counter: u64 = 1;

    for (i, line) in lines.iter().enumerate() {
        let Ok(val): Result<Value, _> = serde_json::from_str(line.trim()) else {
            continue;
        };

        // Only process assistant messages
        if val.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }

        let ts_str = val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let ts = parse_ts(ts_str);

        let message = match val.get("message") {
            Some(m) => m,
            None => continue,
        };

        // Extract intent from text blocks in this message
        let intent = extract_intent(message);

        let content = match message.get("content").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => continue,
        };

        for tool_item in content {
            if tool_item.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                continue;
            }

            let tool_name = tool_item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if tool_name != "Edit" && tool_name != "Write" {
                continue;
            }

            let input = match tool_item.get("input") {
                Some(inp) => inp,
                None => continue,
            };

            let file_path_str = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if file_path_str.is_empty() {
                continue;
            }

            // Look ahead for the tool result in subsequent lines
            let tool_result = find_tool_result(&lines, i + 1, file_path_str);

            // Determine before/after content
            let (before_content, after_content, kind) = match &tool_result {
                Some(result) => {
                    let kind_str = result.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = if kind_str == "create" {
                        EditKind::Create
                    } else {
                        EditKind::Modify
                    };
                    let after = result
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let before = result
                        .get("originalFile")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (before, after, kind)
                }
                None => {
                    // Fall back to extracting from tool input for Edit/Write
                    if tool_name == "Edit" {
                        let old = input
                            .get("old_string")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let new = input
                            .get("new_string")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        (Some(old), new, EditKind::Modify)
                    } else {
                        // Write tool: no before content
                        let new = input
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        (None, new, EditKind::Create)
                    }
                }
            };

            // Compute relative path
            let file_path = PathBuf::from(file_path_str);
            let relative_file = if let Ok(rel) = file_path.strip_prefix(project_root) {
                rel.to_string_lossy().to_string()
            } else {
                // If stripping fails, use the full path
                file_path_str.to_string()
            };

            // Compute diff
            let before_str = before_content.as_deref().unwrap_or("");
            let diff_result = compute_diff(before_str, &after_content, &relative_file);

            let event = EditEvent {
                id: id_counter,
                ts,
                file: relative_file,
                kind,
                patch: diff_result.patch,
                before_hash: None,
                after_hash: String::new(),
                intent: intent.clone(),
                tool: Some(tool_name.to_string()),
                lines_added: diff_result.lines_added,
                lines_removed: diff_result.lines_removed,
                agent_id: None,
                agent_label: None,
                operation_id: None,
                operation_intent: None,
                tool_name: None,
                restore_id: None,
            };

            events.push(event);
            id_counter += 1;
        }
    }

    events.sort_by_key(|e| e.ts);
    Ok(events)
}

/// Search forward from `start_idx` in `lines` to find a tool result referencing `file_path`.
fn find_tool_result(lines: &[String], start_idx: usize, file_path: &str) -> Option<Value> {
    // Search up to 5 lines ahead for the corresponding tool result
    let end = (start_idx + 5).min(lines.len());
    for line in &lines[start_idx..end] {
        let Ok(val): Result<Value, _> = serde_json::from_str(line.trim()) else {
            continue;
        };

        if val.get("type").and_then(|v| v.as_str()) != Some("user") {
            continue;
        }

        // Check toolUseResult
        if let Some(result) = val.get("toolUseResult") {
            let result_path = result
                .get("filePath")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if result_path == file_path || result_path.is_empty() {
                return Some(result.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_to_claude_dir() {
        assert_eq!(
            path_to_claude_dir(Path::new("/Users/foo/bar")),
            "-Users-foo-bar"
        );
        assert_eq!(
            path_to_claude_dir(Path::new("/Users/foo/my-project")),
            "-Users-foo-my-project"
        );
    }

    #[test]
    fn test_list_sessions_empty_dir() {
        let tmp = tempdir().unwrap();
        // Point at a nonexistent path — should return empty vec
        let result = list_sessions(tmp.path().join("nonexistent").as_path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_ts_valid() {
        let ts = parse_ts("2026-03-20T21:51:29.334Z");
        assert!(ts > 0, "Expected positive timestamp");
    }

    #[test]
    fn test_parse_ts_invalid() {
        let ts = parse_ts("not-a-timestamp");
        assert_eq!(ts, 0);
    }
}
