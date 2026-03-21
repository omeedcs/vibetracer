use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

const VIBETRACER_DESCRIPTION: &str = "vibetracer edit tracking";

/// Register a vibetracer hook in `.claude/settings.local.json`.
///
/// Reads or creates the settings file, removes any existing vibetracer hook,
/// then appends a new `PostToolUse` hook entry that forwards tool events to
/// `socket_path` via `nc -U`.
pub fn register_hook(claude_dir: &Path, socket_path: &str) -> Result<()> {
    let settings_path = claude_dir.join("settings.local.json");

    // Read existing settings or start with an empty object.
    let mut settings: Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path)
            .with_context(|| format!("read {:?}", settings_path))?;
        serde_json::from_str(&raw).with_context(|| format!("parse {:?}", settings_path))?
    } else {
        json!({})
    };

    // Ensure the "hooks" key exists and is an array.
    if !settings.get("hooks").map(|v| v.is_array()).unwrap_or(false) {
        settings["hooks"] = json!([]);
    }

    // Remove any existing vibetracer entries.
    let hooks = settings["hooks"].as_array_mut().unwrap();
    hooks.retain(|entry| !entry_has_vibetracer_description(entry));

    // Build the new hook entry.
    let new_entry = json!({
        "matcher": "PostToolUse",
        "hooks": [
            {
                "type": "command",
                "command": format!("echo '$TOOL_NAME $TOOL_INPUT' | nc -U {socket_path}"),
                "description": VIBETRACER_DESCRIPTION,
            }
        ]
    });
    hooks.push(new_entry);

    // Write back.
    std::fs::create_dir_all(claude_dir).with_context(|| format!("create dir {:?}", claude_dir))?;
    let json_str = serde_json::to_string_pretty(&settings).context("serialize settings")?;
    std::fs::write(&settings_path, json_str)
        .with_context(|| format!("write {:?}", settings_path))?;

    Ok(())
}

/// Remove all vibetracer hooks from `.claude/settings.local.json`.
pub fn unregister_hook(claude_dir: &Path) -> Result<()> {
    let settings_path = claude_dir.join("settings.local.json");

    if !settings_path.exists() {
        return Ok(());
    }

    let raw = std::fs::read_to_string(&settings_path)
        .with_context(|| format!("read {:?}", settings_path))?;
    let mut settings: Value =
        serde_json::from_str(&raw).with_context(|| format!("parse {:?}", settings_path))?;

    if let Some(hooks) = settings.get_mut("hooks").and_then(|v| v.as_array_mut()) {
        hooks.retain(|entry| !entry_has_vibetracer_description(entry));
    }

    let json_str = serde_json::to_string_pretty(&settings).context("serialize settings")?;
    std::fs::write(&settings_path, json_str)
        .with_context(|| format!("write {:?}", settings_path))?;

    Ok(())
}

/// Return `true` if any nested hook in the entry has a description containing "vibetracer".
fn entry_has_vibetracer_description(entry: &Value) -> bool {
    if let Some(inner_hooks) = entry.get("hooks").and_then(|v| v.as_array()) {
        for hook in inner_hooks {
            if let Some(desc) = hook.get("description").and_then(|v| v.as_str()) {
                if desc.contains("vibetracer") {
                    return true;
                }
            }
        }
    }
    false
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_register_creates_settings_file() {
        let tmp = tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        register_hook(&claude_dir, "/tmp/vibetracer.sock").unwrap();
        assert!(claude_dir.join("settings.local.json").exists());
    }

    #[test]
    fn test_unregister_removes_vibetracer_entry() {
        let tmp = tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        register_hook(&claude_dir, "/tmp/vibetracer.sock").unwrap();
        unregister_hook(&claude_dir).unwrap();

        let raw = std::fs::read_to_string(claude_dir.join("settings.local.json")).unwrap();
        assert!(!raw.contains("vibetracer"));
    }
}
