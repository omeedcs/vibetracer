# vibetracer v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rearchitect vibetracer from a single-process TUI into a daemon + viewer model with per-file playheads, command-level operation grouping, multi-agent support, explicit restore system, and 19 color themes.

**Architecture:** Project-scoped daemon records edits to append-only JSONL. TUI reads the log file directly (no IPC for data). Shared `recorder/` module used by both daemon process and `--no-daemon` in-process mode. Restore system writes files to disk with daemon coordination via Unix socket.

**Tech Stack:** Rust (edition 2024), ratatui 0.29, crossterm 0.28, notify 7, similar 2, serde/serde_json, clap 4, sha2, anyhow

**Spec:** `docs/superpowers/specs/2026-03-24-vibetracer-v2-design.md`

---

## Phase 1: Strip & Clean (remove dead code, cut features)

### Task 1: Remove equation lens

**Files:**
- Delete: `src/equation/detect.rs`
- Delete: `src/equation/render.rs`
- Delete: `src/equation/mod.rs`
- Delete: `src/tui/widgets/equation_panel.rs`
- Modify: `src/lib.rs:7` (remove `pub mod equation;`)
- Modify: `src/tui/mod.rs:9` (remove equation import, `equations` field from App, equation scanning logic)
- Modify: `src/tui/input.rs` (remove `ToggleEquationLens` action)
- Modify: `src/tui/widgets/mod.rs` (remove `pub mod equation_panel;`)

- [ ] **Step 1: Delete equation module files**

```bash
rm src/equation/detect.rs src/equation/render.rs src/equation/mod.rs
rm src/tui/widgets/equation_panel.rs
```

- [ ] **Step 2: Remove equation module declaration from lib.rs**

In `src/lib.rs`, remove the line `pub mod equation;`

- [ ] **Step 3: Remove equation imports and fields from tui/mod.rs**

Remove the `use crate::equation::detect::{self as eq_detect, DetectedEquation};` import line. Remove the `pub equations: Vec<DetectedEquation>` field from the `App` struct and its initialization `equations: Vec::new()` in `App::new()`. Remove `Equations` from the `SidebarPanel` enum. Remove all equation-related logic in the event loop (equation lens toggle handling, equation scanning on scrub, equation detection on file change).

- [ ] **Step 4: Remove equation panel from widgets/mod.rs**

Remove `pub mod equation_panel;` from `src/tui/widgets/mod.rs`.

- [ ] **Step 5: Remove ToggleEquationLens from input.rs**

Remove `ToggleEquationLens` from the `Action` enum, its key mapping in `map_key`, and its handler in `apply_action`.

- [ ] **Step 6: Remove equation references from sidebar rendering in tui/mod.rs**

Remove the `SidebarPanel::Equations => { ... }` match arm from the sidebar rendering block.

- [ ] **Step 7: Remove equation key from keybindings bar in tui/mod.rs**

Remove the `"e"` / `" equations"` spans from the keybindings bar `Line::from(vec![...])`.

- [ ] **Step 8: Verify it compiles**

Run: `cargo build 2>&1`
Expected: Compiles with no errors (warnings OK for now)

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "refactor: remove equation lens module and all references"
```

---

### Task 2: Remove embedded terminal / PTY

**Files:**
- Delete: `src/pty/mod.rs`
- Modify: `src/lib.rs` (remove `pub mod pty;`)
- Modify: `src/tui/mod.rs` (remove terminal fields from App, EmbeddedTerminal import, terminal setup, terminal rendering, TerminalPane pane variant, sync_terminal_output)
- Modify: `src/tui/input.rs` (remove `ToggleTerminalFocus` action, terminal-related key handling)
- Modify: `src/tui/layout.rs` (remove terminal pane layout logic)
- Delete: `src/tui/widgets/terminal_pane.rs`
- Modify: `src/tui/widgets/mod.rs` (remove `pub mod terminal_pane;`)
- Modify: `src/main.rs` (remove `--embed`, `--cmd` flags)
- Modify: `Cargo.toml` (remove `portable-pty`, `vt100`)

- [ ] **Step 1: Delete PTY module and terminal pane widget**

```bash
rm src/pty/mod.rs src/tui/widgets/terminal_pane.rs
```

- [ ] **Step 2: Remove pty module from lib.rs**

Remove `pub mod pty;` from `src/lib.rs`.

- [ ] **Step 3: Remove terminal_pane from widgets/mod.rs**

Remove `pub mod terminal_pane;` from `src/tui/widgets/mod.rs`.

- [ ] **Step 4: Remove embed/cmd CLI flags from main.rs**

In the `Cli` struct, remove the `embed: bool` field and `cmd: String` field with their `#[arg]` attributes. In the `None` match arm of the main function, remove the `if cli.embed { ... }` block that creates `RunOptions` with `embed_command`.

- [ ] **Step 5: Remove terminal fields and logic from tui/mod.rs**

Remove: `use crate::pty::EmbeddedTerminal;` import. Remove `TerminalPane` from `Pane` enum. Remove these fields from `App` struct: `terminal`, `terminal_output`, `terminal_visible`, `last_vibetracer_pane`. Remove their initializations in `App::new()`. Remove `sync_terminal_output` method. Remove embedded terminal setup block in `run_tui_with_options`. Remove terminal rendering block. Remove `Ctrl+\` handling and `ToggleTerminalFocus` dispatch. Remove PTY forwarding block (`if app.focused_pane == Pane::TerminalPane`). Remove PTY resize handling in the `Event::Resize` handler.

- [ ] **Step 6: Simplify layout.rs — remove terminal pane**

In `src/tui/layout.rs`, remove the `terminal: Option<Rect>` field from the layout struct and all conditional logic that creates a terminal pane rect.

- [ ] **Step 7: Remove ToggleTerminalFocus from input.rs**

Remove `ToggleTerminalFocus` from the `Action` enum, its key mapping, and its handler in `apply_action`. Remove `TerminalPane` from pane cycling logic.

- [ ] **Step 8: Remove portable-pty and vt100 from Cargo.toml**

Remove the lines `portable-pty = "0.8"` and `vt100 = "0.15"` from `[dependencies]`.

- [ ] **Step 9: Verify it compiles**

Run: `cargo build 2>&1`
Expected: Compiles with no errors

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "refactor: remove embedded terminal/PTY and dependencies"
```

---

### Task 2b: Simplify layout.rs (remove terminal pane)

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Remove terminal pane logic**

The layout no longer needs to account for a terminal pane. Simplify to two modes: preview + sidebar (if visible), or preview only. Update the `compute_layout` function signature to remove the `terminal_visible` parameter.

- [ ] **Step 2: Update all callers**

Find all calls to `compute_layout` and remove the `terminal_visible` argument.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: simplify layout — remove terminal pane"
```

---

### Task 3: Remove splash, demo, schema diff, refactor tracker, rewind, BusEvent, tokio

**Files:**
- Delete: `src/splash.rs`, `src/demo.rs`, `src/rewind/mod.rs`
- Delete: `src/analysis/schema_diff.rs`, `src/analysis/refactor_tracker.rs`, `src/analysis/imports.rs`
- Delete: `src/hook/bridge.rs`
- Delete: `src/tui/widgets/refactor_panel.rs`
- Modify: `src/lib.rs` (remove `pub mod splash;`, `pub mod demo;`, `pub mod rewind;`)
- Modify: `src/analysis/mod.rs` (remove `pub mod schema_diff;`, `pub mod refactor_tracker;`, `pub mod imports;`)
- Modify: `src/hook/mod.rs` (remove `pub mod bridge;`)
- Modify: `src/event.rs` (remove `BusEvent` enum)
- Modify: `src/main.rs` (remove `Demo` subcommand, `--no-splash` flag, splash call)
- Modify: `src/session.rs` (replace `rand` usage with timestamp-based ID suffix)
- Modify: `src/tui/mod.rs` (remove schema_diff_mode from App, Refactor from SidebarPanel)
- Modify: `src/tui/input.rs` (remove schema diff and refactor toggle actions)
- Modify: `src/tui/widgets/mod.rs` (remove `pub mod refactor_panel;`)
- Modify: `Cargo.toml` (remove `tokio`, `rand`)

- [ ] **Step 1: Delete files**

```bash
rm src/splash.rs src/demo.rs src/rewind/mod.rs
rm src/analysis/schema_diff.rs src/analysis/refactor_tracker.rs src/analysis/imports.rs
rm src/hook/bridge.rs
rm src/tui/widgets/refactor_panel.rs
```

- [ ] **Step 2: Update lib.rs**

Remove: `pub mod splash;`, `pub mod demo;`, `pub mod rewind;`

- [ ] **Step 3: Update analysis/mod.rs**

Remove: `pub mod schema_diff;`, `pub mod refactor_tracker;`, `pub mod imports;`

- [ ] **Step 3b: Update hook/mod.rs**

Remove: `pub mod bridge;`

- [ ] **Step 4: Remove BusEvent from event.rs**

Delete the entire `pub enum BusEvent { ... }` block (lines 42-59) and the `use crossterm` import if it was only needed for `BusEvent::Input`.

- [ ] **Step 5: Remove Demo subcommand and splash from main.rs**

Remove `Demo` from `Commands` enum. Remove `no_splash: bool` field from `Cli`. Remove `Some(Commands::Demo) => { ... }` match arm. Remove `if !cli.no_splash { vibetracer::splash::play_splash()?; }` block.

- [ ] **Step 6: Remove schema_diff_mode, Refactor sidebar panel from tui/mod.rs**

Remove `schema_diff_mode: bool` from App struct and its init. Remove `Refactor` from `SidebarPanel` enum. Remove the `SidebarPanel::Refactor => { ... }` match arm from sidebar rendering.

- [ ] **Step 7: Remove schema diff and refactor actions from input.rs**

Remove `ToggleSchemaDiff` and any refactor-related actions from `Action` enum, `map_key`, and `apply_action`.

- [ ] **Step 8: Remove refactor_panel from widgets/mod.rs**

Remove `pub mod refactor_panel;`.

- [ ] **Step 9: Remove schema diff and refactor keys from keybindings bar**

Remove the `"d"` / `" schema diff"` and `"f"` / `" refactor"` spans from the keybindings bar in `tui/mod.rs`.

- [ ] **Step 9b: Replace rand usage in session.rs**

`src/session.rs` uses `rand::Rng` for session ID generation. Replace with a timestamp-based approach using microseconds for uniqueness:

```rust
// Replace: let mut rng = rand::rng(); ... hex suffix
// With: use the last 4 hex chars of the microsecond timestamp
let micros = chrono::Utc::now().timestamp_micros();
let hex_suffix = format!("{:04x}", (micros & 0xFFFF) as u16);
```

Remove the `use rand::Rng;` import.

- [ ] **Step 10: Remove tokio and rand from Cargo.toml**

Remove `tokio = { version = "1", features = ["full"] }` and `rand = "0.9"`.

- [ ] **Step 11: Verify it compiles and tests pass**

Run: `cargo build 2>&1 && cargo test 2>&1`
Expected: Compiles. Some tests may fail if they reference removed modules — fix any remaining references.

- [ ] **Step 12: Commit**

```bash
git add -A && git commit -m "refactor: strip splash, demo, rewind, schema diff, refactor tracker, BusEvent, tokio"
```

---

## Phase 2: Data Model v2

### Task 4: Update EditEvent with agent and operation fields

**Files:**
- Modify: `src/event.rs`
- Modify: `src/snapshot/edit_log.rs` (update read_all to skip malformed trailing lines)
- Test: existing tests in both files

- [ ] **Step 1: Write tests for new EditEvent fields**

Add to `src/event.rs` tests:

```rust
#[test]
fn test_edit_event_v2_fields_serialize() {
    let event = EditEvent {
        id: 1,
        ts: 1_700_000_000_000,
        file: "src/main.rs".to_string(),
        kind: EditKind::Modify,
        patch: "@@ -1 +1 @@\n-old\n+new".to_string(),
        before_hash: Some("abc".to_string()),
        after_hash: "def".to_string(),
        lines_added: 1,
        lines_removed: 1,
        agent_id: Some("12345".to_string()),
        agent_label: Some("claude-1".to_string()),
        operation_id: Some("op-7".to_string()),
        operation_intent: Some("refactor auth".to_string()),
        tool_name: Some("Edit".to_string()),
        restore_id: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    let restored: EditEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.agent_id, Some("12345".to_string()));
    assert_eq!(restored.operation_id, Some("op-7".to_string()));
    assert_eq!(restored.restore_id, None);
}

#[test]
fn test_v1_json_deserializes_with_defaults() {
    // v1 JSON has no agent/operation/restore fields
    let v1_json = r#"{"id":1,"ts":1700000000000,"file":"src/main.rs","kind":"modify","patch":"","before_hash":"abc","after_hash":"def","intent":"fix bug","tool":"cursor","lines_added":1,"lines_removed":1}"#;
    let event: EditEvent = serde_json::from_str(v1_json).unwrap();
    assert_eq!(event.agent_id, None);
    assert_eq!(event.operation_id, None);
    assert_eq!(event.restore_id, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib event::tests 2>&1`
Expected: FAIL — new fields don't exist yet

- [ ] **Step 3: Update EditEvent struct**

In `src/event.rs`, replace the `EditEvent` struct with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditEvent {
    pub id: u64,
    pub ts: i64,
    pub file: String,
    pub kind: EditKind,
    pub patch: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    pub lines_added: u32,
    pub lines_removed: u32,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_label: Option<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub operation_intent: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub restore_id: Option<u64>,
}
```

Note: keep `intent` and `tool` for now (backward compat with v1 data and import). Mark them `#[serde(default)]`.

- [ ] **Step 4: Update existing tests and helpers for new fields**

Update the existing `test_edit_event_serialization` test in `src/event.rs` to include the new fields (`agent_id: None, agent_label: None, operation_id: None, operation_intent: None, tool_name: None, restore_id: None`).

Update the `sample_event()` helper in `src/snapshot/edit_log.rs` (line ~76-89) to include the new fields with `None`/default values so edit_log tests continue to compile.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib event::tests 2>&1 && cargo test --lib snapshot::edit_log::tests 2>&1`
Expected: PASS

- [ ] **Step 6: Fix edit_log.rs read_all to skip malformed lines**

In `src/snapshot/edit_log.rs`, update `read_all` to use:

```rust
pub fn read_all(path: &std::path::Path) -> anyhow::Result<Vec<EditEvent>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut events = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<EditEvent>(&line) {
            Ok(event) => events.push(event),
            Err(e) => {
                // Skip malformed lines (e.g., truncated writes from crashes)
                tracing::warn!("skipping malformed line {} in edit log: {}", i + 1, e);
            }
        }
    }
    Ok(events)
}
```

- [ ] **Step 7: Add test for malformed line skipping**

```rust
#[test]
fn test_read_all_skips_malformed_trailing_line() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("edits.jsonl");
    let valid = r#"{"id":1,"ts":0,"file":"a.rs","kind":"modify","patch":"","before_hash":null,"after_hash":"x","lines_added":0,"lines_removed":0}"#;
    std::fs::write(&path, format!("{}\n{{truncated", valid)).unwrap();
    let events = EditLog::read_all(&path).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 1);
}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test 2>&1`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: add agent identity, operation grouping, restore_id to EditEvent"
```

---

### Task 5: Add RestoreEvent and RestoreScope types

**Files:**
- Modify: `src/event.rs` (add new types)

- [ ] **Step 1: Write tests for RestoreEvent serialization**

```rust
#[test]
fn test_restore_event_serialization() {
    let event = RestoreEvent {
        id: 1,
        ts: 1_700_000_000_000,
        scope: RestoreScope::File {
            path: "src/main.rs".to_string(),
            target_edit_id: 42,
        },
        files_restored: vec![RestoreFileEntry {
            path: "src/main.rs".to_string(),
            from_hash: "abc".to_string(),
            to_hash: "def".to_string(),
        }],
    };
    let json = serde_json::to_string(&event).unwrap();
    let restored: RestoreEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, 1);
    match restored.scope {
        RestoreScope::File { target_edit_id, .. } => assert_eq!(target_edit_id, 42),
        _ => panic!("wrong scope"),
    }
}

#[test]
fn test_restore_scope_file_deletion() {
    let entry = RestoreFileEntry {
        path: "new_file.rs".to_string(),
        from_hash: "abc".to_string(),
        to_hash: String::new(), // empty = file deleted
    };
    assert!(entry.to_hash.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib event::tests 2>&1`
Expected: FAIL — types don't exist

- [ ] **Step 3: Add RestoreEvent, RestoreScope, RestoreFileEntry to event.rs**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreEvent {
    pub id: u64,
    pub ts: i64,
    pub scope: RestoreScope,
    pub files_restored: Vec<RestoreFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreFileEntry {
    pub path: String,
    pub from_hash: String,
    pub to_hash: String, // empty string = file was deleted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RestoreScope {
    File { path: String, target_edit_id: u64 },
    Operation { operation_id: String },
    AgentRange { agent_id: String, from_ts: i64, to_ts: i64 },
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib event::tests 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add RestoreEvent, RestoreScope, RestoreFileEntry types"
```

---

### Task 6: Update SessionMeta with agents field

**Files:**
- Modify: `src/session.rs`
- Modify: `src/event.rs` (add AgentInfo struct)

- [ ] **Step 1: Write test for v1 backward compat**

In `src/session.rs` tests:

```rust
#[test]
fn test_v1_meta_deserializes_with_empty_agents() {
    let v1_json = r#"{"id":"test-123","project_path":"/tmp","started_at":0,"mode":"passive"}"#;
    let meta: SessionMeta = serde_json::from_str(v1_json).unwrap();
    assert!(meta.agents.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib session::tests 2>&1`
Expected: FAIL — `agents` field doesn't exist

- [ ] **Step 3: Add AgentInfo to event.rs**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub agent_label: String,
    pub tool_type: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub edit_count: u64,
}
```

- [ ] **Step 4: Add agents field to SessionMeta in session.rs**

Add `use crate::event::AgentInfo;` import and add to the `SessionMeta` struct:

```rust
#[serde(default)]
pub agents: Vec<AgentInfo>,
```

- [ ] **Step 5: Run tests**

Run: `cargo test 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add AgentInfo type and agents field to SessionMeta"
```

---

## Phase 3: Themes (19 presets)

### Task 7: Expand Theme struct and add 19 presets

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Write test for all theme presets**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PRESETS: &[&str] = &[
        "dark", "catppuccin-mocha", "catppuccin-macchiato", "gruvbox-dark",
        "tokyo-night", "tokyo-night-storm", "dracula", "nord", "kanagawa",
        "rose-pine", "one-dark", "solarized-dark", "everforest-dark",
        "light", "catppuccin-latte", "gruvbox-light", "solarized-light",
        "rose-pine-dawn", "everforest-light",
    ];

    #[test]
    fn test_all_presets_load() {
        for name in ALL_PRESETS {
            let theme = Theme::from_preset(name);
            // Verify agent_colors has 6 entries (it's a fixed array)
            assert_eq!(theme.agent_colors.len(), 6, "theme {} missing agent_colors", name);
        }
    }

    #[test]
    fn test_unknown_preset_falls_back_to_dark() {
        let theme = Theme::from_preset("nonexistent");
        let dark = Theme::dark();
        assert_eq!(theme.bg, dark.bg);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib theme::tests 2>&1`
Expected: FAIL — `agent_colors` field doesn't exist, presets missing

- [ ] **Step 3: Rewrite theme.rs with agent_colors and all 19 presets**

Replace `src/theme.rs` entirely. The `Theme` struct gets the `agent_colors: [Color; 6]` field. Add `from_preset` matching all 19 names. Each preset is a static constructor fn. All colors use `Color::Rgb(r, g, b)`.

Reference palettes for accuracy:
- **Catppuccin Mocha/Macchiato/Latte**: catppuccin.com palette values
- **Gruvbox Dark/Light**: github.com/morhetz/gruvbox
- **Tokyo Night/Storm**: github.com/enkia/tokyo-night-vscode-theme
- **Dracula**: draculatheme.com
- **Nord**: nordtheme.com
- **Kanagawa**: github.com/rebelot/kanagawa.nvim
- **Rose Pine/Dawn**: rosepinetheme.com
- **One Dark**: github.com/atom/one-dark-syntax
- **Solarized Dark/Light**: ethanschoonover.com/solarized
- **Everforest Dark/Light**: github.com/sainnhe/everforest

Agent colors per theme should be 6 visually distinct, saturated colors that are legible against that theme's background.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib theme::tests 2>&1`
Expected: PASS

- [ ] **Step 5: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: expand theme system to 19 presets with agent colors"
```

---

## Phase 4: Recorder Module (shared between daemon and TUI)

### Task 8: Extract recorder from tui/mod.rs into recorder/

**Files:**
- Create: `src/recorder/mod.rs`
- Modify: `src/lib.rs` (add `pub mod recorder;`)
- Modify: `src/tui/mod.rs` (remove watcher+snapshot+editlog logic, import recorder)

The recorder encapsulates: FsWatcher setup, file-change processing (diff, snapshot, edit event creation), edit log writing. It takes an `mpsc::Sender<EditEvent>` to push new events to whoever is consuming them (the TUI or the daemon).

- [ ] **Step 1: Write test for Recorder**

Create `src/recorder/mod.rs` with a test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::sync::mpsc;

    #[test]
    fn test_recorder_processes_file_change() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("test.txt"), "hello").unwrap();

        let session_dir = tmp.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();

        let (tx, _rx) = mpsc::channel();
        let mut recorder = Recorder::new(
            project.clone(),
            session_dir.clone(),
        ).unwrap();

        // First call: recorder sees "hello" for the first time (Create)
        let result = recorder.process_file_change(&project.join("test.txt"), &tx, None).unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.event.file, "test.txt");
        assert_eq!(result.event.kind, EditKind::Create);
        assert_eq!(result.old_content, ""); // no prior state
        assert_eq!(result.new_content, "hello");

        // Second call: modify the file (Modify)
        std::fs::write(project.join("test.txt"), "hello world").unwrap();
        let result = recorder.process_file_change(&project.join("test.txt"), &tx, None).unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.event.kind, EditKind::Modify);
        assert_eq!(result.old_content, "hello");
        assert_eq!(result.new_content, "hello world");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib recorder::tests 2>&1`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement Recorder struct**

Create `src/recorder/mod.rs`:

```rust
use crate::config::Config;
use crate::event::{EditEvent, EditKind};
use crate::snapshot::{edit_log::EditLog, store::SnapshotStore};
use crate::watcher::{differ::compute_diff, fs_watcher::FsWatcher};
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct RecordResult {
    pub event: EditEvent,
    pub old_content: String,
    pub new_content: String,
}

pub struct Enrichment {
    pub agent_id: Option<String>,
    pub agent_label: Option<String>,
    pub operation_id: Option<String>,
    pub operation_intent: Option<String>,
    pub tool_name: Option<String>,
    pub restore_id: Option<u64>,
}

pub struct Recorder {
    pub project_root: PathBuf,
    snapshot_store: SnapshotStore,
    edit_log: EditLog,
    file_hashes: HashMap<String, String>,
    edit_id_counter: u64,
}

impl Recorder {
    pub fn new(project_root: PathBuf, session_dir: PathBuf) -> Result<Self> {
        let snapshot_store = SnapshotStore::new(session_dir.join("snapshots"));
        let edit_log = EditLog::new(session_dir.join("edits.jsonl"));
        Ok(Self {
            project_root,
            snapshot_store,
            edit_log,
            file_hashes: HashMap::new(),
            edit_id_counter: 1,
        })
    }

    /// Process a single file change. Returns a RecordResult with the event and content
    /// if the file actually changed. Sends the event on `event_tx` and persists to
    /// edit log and snapshot store. Optional enrichment is applied before writing.
    pub fn process_file_change(
        &mut self,
        abs_path: &std::path::Path,
        event_tx: &mpsc::Sender<EditEvent>,
        enrichment: Option<&Enrichment>,
    ) -> Result<Option<RecordResult>> {
        let rel_path = abs_path
            .strip_prefix(&self.project_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| abs_path.to_string_lossy().to_string());

        let new_content = std::fs::read_to_string(abs_path).unwrap_or_default();

        let old_content = if let Some(hash) = self.file_hashes.get(&rel_path) {
            self.snapshot_store
                .retrieve(hash)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default()
        } else {
            String::new()
        };

        if old_content == new_content {
            return Ok(None);
        }

        let diff = compute_diff(&old_content, &new_content, &rel_path);

        let kind = if !abs_path.exists() {
            EditKind::Delete
        } else if self.file_hashes.contains_key(&rel_path) {
            EditKind::Modify
        } else {
            EditKind::Create
        };

        let after_hash = self.snapshot_store.store(new_content.as_bytes())?;
        let before_hash = self.file_hashes.get(&rel_path).cloned();

        let edit = EditEvent {
            id: self.edit_id_counter,
            ts: Utc::now().timestamp_millis(),
            file: rel_path.clone(),
            kind,
            patch: diff.patch,
            before_hash,
            after_hash: after_hash.clone(),
            intent: None,
            tool: None,
            lines_added: diff.lines_added,
            lines_removed: diff.lines_removed,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };

        // Apply enrichment before writing
        if let Some(enr) = enrichment {
            edit.agent_id = enr.agent_id.clone();
            edit.agent_label = enr.agent_label.clone();
            edit.operation_id = enr.operation_id.clone();
            edit.operation_intent = enr.operation_intent.clone();
            edit.tool_name = enr.tool_name.clone();
            edit.restore_id = enr.restore_id;
        }

        self.edit_id_counter += 1;
        self.file_hashes.insert(rel_path, after_hash);
        self.edit_log.append(&edit)?;
        let _ = event_tx.send(edit.clone());

        Ok(Some(RecordResult {
            event: edit,
            old_content,
            new_content,
        }))
    }

    pub fn current_file_hashes(&self) -> &HashMap<String, String> {
        &self.file_hashes
    }

    pub fn snapshot_store(&self) -> &SnapshotStore {
        &self.snapshot_store
    }
}
```

- [ ] **Step 4: Add module to lib.rs**

Add `pub mod recorder;` to `src/lib.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib recorder::tests 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: extract Recorder module from TUI (shared between daemon and TUI)"
```

---

### Task 9: Refactor TUI to use Recorder

**Files:**
- Modify: `src/tui/mod.rs` (replace inline watcher/snapshot/editlog logic with Recorder)

- [ ] **Step 1: Replace inline recording logic in run_tui_with_options**

In `src/tui/mod.rs`, the file-change processing loop (the `loop { match rx.try_recv() { ... } }` block) currently does: read file, compute diff, create EditEvent, store snapshot, append edit log, update hashes. Replace all of that with a single call to `recorder.process_file_change(&abs_path, &edit_tx)`. The `edit_tx` sends events to a new channel that the TUI loop reads from (replacing the current `rx` channel from FsWatcher).

Rearchitect the channel flow:
- `FsWatcher` sends `PathBuf` to a channel
- A background thread drains that channel, calls `recorder.process_file_change` for each path
- `process_file_change` sends `EditEvent` to the TUI via `event_tx`
- The TUI main loop reads `EditEvent`s from `event_rx`

For now, keep it simpler: the TUI loop drains the FsWatcher channel and calls `recorder.process_file_change` inline (same thread). This matches the current behavior but uses the Recorder abstraction.

- [ ] **Step 2: Remove now-unused imports and variables from tui/mod.rs**

Remove: direct imports of `SnapshotStore`, `EditLog`, `compute_diff`. Remove local variables `file_hashes`, `edit_id_counter`, `current_file_hashes`, `snapshot_store`, `edit_log`. Replace with a single `recorder: Recorder`.

- [ ] **Step 3: Update analysis engine calls to use recorder**

The analysis engines (watchdog, sentinels, blast radius) need `old_content` and `new_content` which `process_file_change` doesn't return. Add a method to Recorder that returns the last old/new content pair, or pass the content through the EditEvent (add it as transient non-serialized state). Simplest: read the file content again in the TUI loop for analysis (it's already on disk and cached by the OS).

- [ ] **Step 4: Verify all tests pass**

Run: `cargo test 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor: TUI uses Recorder module instead of inline recording logic"
```

---

## Phase 5: Daemon

### Task 10: PID file management

**Files:**
- Create: `src/daemon/mod.rs`
- Create: `src/daemon/pid.rs`
- Modify: `src/lib.rs` (add `pub mod daemon;`)

- [ ] **Step 1: Write tests for PID management**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_and_read_pid_file() {
        let tmp = tempdir().unwrap();
        let pid_path = tmp.path().join("daemon.pid");
        write_pid_file(&pid_path, 12345, "session-abc").unwrap();
        let (pid, session_id) = read_pid_file(&pid_path).unwrap();
        assert_eq!(pid, 12345);
        assert_eq!(session_id, "session-abc");
    }

    #[test]
    fn test_is_process_alive_returns_true_for_self() {
        let pid = std::process::id() as i32;
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_returns_false_for_bogus_pid() {
        assert!(!is_process_alive(999999999));
    }

    #[test]
    fn test_cleanup_stale_pid() {
        let tmp = tempdir().unwrap();
        let pid_path = tmp.path().join("daemon.pid");
        let sock_path = tmp.path().join("daemon.sock");
        write_pid_file(&pid_path, 999999999, "old-session").unwrap();
        std::fs::write(&sock_path, "").unwrap();
        cleanup_stale(&pid_path, &sock_path).unwrap();
        assert!(!pid_path.exists());
        assert!(!sock_path.exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib daemon::pid::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement pid.rs**

```rust
use anyhow::{Context, Result};
use std::path::Path;

pub fn write_pid_file(path: &Path, pid: i32, session_id: &str) -> Result<()> {
    let content = format!("{}\n{}", pid, session_id);
    std::fs::write(path, content).with_context(|| format!("write PID file {:?}", path))
}

pub fn read_pid_file(path: &Path) -> Result<(i32, String)> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read PID file {:?}", path))?;
    let mut lines = content.lines();
    let pid: i32 = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty PID file"))?
        .trim()
        .parse()
        .context("parse PID")?;
    let session_id = lines
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok((pid, session_id))
}

pub fn is_process_alive(pid: i32) -> bool {
    // kill(pid, 0) checks if process exists without sending a signal
    let ret = unsafe { libc::kill(pid, 0) };
    if ret == 0 {
        return true;
    }
    // EPERM means process exists but belongs to another user
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub fn cleanup_stale(pid_path: &Path, sock_path: &Path) -> Result<()> {
    if pid_path.exists() {
        std::fs::remove_file(pid_path)?;
    }
    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Create daemon/mod.rs**

```rust
pub mod pid;
```

- [ ] **Step 5: Add `pub mod daemon;` to lib.rs and `libc` to Cargo.toml**

Add `libc = "0.2"` to `[dependencies]` in Cargo.toml.

- [ ] **Step 6: Run tests**

Run: `cargo test --lib daemon::pid::tests 2>&1`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: add daemon PID file management with stale detection"
```

---

### Task 11: Hook listener (Unix socket server)

**Files:**
- Create: `src/daemon/hook_listener.rs`
- Create: `src/daemon/correlation.rs`
- Modify: `src/daemon/mod.rs`

- [ ] **Step 1: Write tests for correlation**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_enrichments_fifo() {
        let mut corr = Correlator::new();
        corr.push_enrichment("src/main.rs", HookPayload {
            agent_id: "a1".to_string(),
            operation_id: "op-1".to_string(),
            tool_name: "Edit".to_string(),
            intent: Some("first".to_string()),
        });
        corr.push_enrichment("src/main.rs", HookPayload {
            agent_id: "a2".to_string(),
            operation_id: "op-2".to_string(),
            tool_name: "Write".to_string(),
            intent: Some("second".to_string()),
        });

        let first = corr.pop_enrichment("src/main.rs");
        assert_eq!(first.unwrap().agent_id, "a1"); // FIFO

        let second = corr.pop_enrichment("src/main.rs");
        assert_eq!(second.unwrap().agent_id, "a2");

        assert!(corr.pop_enrichment("src/main.rs").is_none());
    }

    #[test]
    fn test_stale_enrichments_cleaned() {
        let mut corr = Correlator::new();
        corr.push_enrichment_with_time("src/old.rs", HookPayload {
            agent_id: "a1".to_string(),
            operation_id: "op-1".to_string(),
            tool_name: "Edit".to_string(),
            intent: None,
        }, chrono::Utc::now().timestamp_millis() - 6000); // 6 seconds ago

        corr.cleanup_stale(5000); // 5 second threshold
        assert!(corr.pop_enrichment("src/old.rs").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib daemon::correlation::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement correlation.rs**

```rust
use std::collections::{HashMap, VecDeque};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct HookPayload {
    pub agent_id: String,
    pub operation_id: String,
    pub tool_name: String,
    pub intent: Option<String>,
}

struct TimedPayload {
    payload: HookPayload,
    received_at: i64, // millis
}

pub struct Correlator {
    pending: HashMap<String, VecDeque<TimedPayload>>,
    pending_restores: HashMap<String, u64>,
}

impl Correlator {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            pending_restores: HashMap::new(),
        }
    }

    pub fn push_enrichment(&mut self, file: &str, payload: HookPayload) {
        self.push_enrichment_with_time(file, payload, Utc::now().timestamp_millis());
    }

    pub fn push_enrichment_with_time(&mut self, file: &str, payload: HookPayload, ts: i64) {
        self.pending
            .entry(file.to_string())
            .or_default()
            .push_back(TimedPayload { payload, received_at: ts });
    }

    pub fn pop_enrichment(&mut self, file: &str) -> Option<HookPayload> {
        let queue = self.pending.get_mut(file)?;
        let item = queue.pop_front()?;
        if queue.is_empty() {
            self.pending.remove(file);
        }
        Some(item.payload)
    }

    pub fn cleanup_stale(&mut self, max_age_ms: i64) {
        let now = Utc::now().timestamp_millis();
        self.pending.retain(|_, queue| {
            queue.retain(|item| now - item.received_at < max_age_ms);
            !queue.is_empty()
        });
        // Also clean stale restores (30 second timeout)
        // (would need timestamps on restores too — simplification: caller handles)
    }

    pub fn register_restore(&mut self, restore_id: u64, files: &[String]) {
        for file in files {
            self.pending_restores.insert(file.clone(), restore_id);
        }
    }

    pub fn pop_restore(&mut self, file: &str) -> Option<u64> {
        self.pending_restores.remove(file)
    }

    pub fn clear_restore(&mut self, restore_id: u64) {
        self.pending_restores.retain(|_, id| *id != restore_id);
    }
}
```

- [ ] **Step 4: Implement hook_listener.rs skeleton**

```rust
use anyhow::Result;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc;
use std::io::{BufRead, BufReader};

pub enum SocketMessage {
    Hook(crate::daemon::correlation::HookPayload, String), // payload + filename
    RestoreStart { restore_id: u64, files: Vec<String> },
    RestoreEnd { restore_id: u64 },
    StatusQuery(std::os::unix::net::UnixStream),
    Stop,
}

pub fn listen(
    sock_path: &Path,
    tx: mpsc::Sender<SocketMessage>,
) -> Result<()> {
    let listener = UnixListener::bind(sock_path)?;
    listener.set_nonblocking(false)?;

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tx = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(&stream);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                if let Some(msg) = parse_message(&line) {
                    match msg {
                        SocketMessage::StatusQuery(_) => {
                            // Clone stream for response
                            let _ = tx.send(SocketMessage::StatusQuery(
                                stream.try_clone().unwrap_or_else(|_| panic!("clone stream"))
                            ));
                        }
                        other => { let _ = tx.send(other); }
                    }
                }
            }
        });
    }
    Ok(())
}

fn parse_message(line: &str) -> Option<SocketMessage> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let msg_type = v.get("type")?.as_str()?;
    match msg_type {
        "hook" => {
            let payload = crate::daemon::correlation::HookPayload {
                agent_id: v.get("agent_id")?.as_str()?.to_string(),
                operation_id: v.get("operation_id")?.as_str()?.to_string(),
                tool_name: v.get("tool_name")?.as_str()?.to_string(),
                intent: v.get("intent").and_then(|v| v.as_str()).map(String::from),
            };
            let file = v.get("file")?.as_str()?.to_string();
            Some(SocketMessage::Hook(payload, file))
        }
        "control" => {
            let cmd = v.get("command")?.as_str()?;
            match cmd {
                "stop" => Some(SocketMessage::Stop),
                "status" => None, // handled at connection level
                _ => None,
            }
        }
        "restore_start" => {
            let restore_id = v.get("restore_id")?.as_u64()?;
            let files: Vec<String> = v.get("files")?
                .as_array()?
                .iter()
                .filter_map(|f| f.as_str().map(String::from))
                .collect();
            Some(SocketMessage::RestoreStart { restore_id, files })
        }
        "restore_end" => {
            let restore_id = v.get("restore_id")?.as_u64()?;
            Some(SocketMessage::RestoreEnd { restore_id })
        }
        _ => None,
    }
}
```

- [ ] **Step 5: Update daemon/mod.rs**

```rust
pub mod correlation;
pub mod hook_listener;
pub mod pid;
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib daemon 2>&1`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: add hook listener and correlation engine for daemon"
```

---

### Task 12: Agent registry

**Files:**
- Create: `src/daemon/agent_registry.rs`
- Modify: `src/daemon/mod.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_new_agent() {
        let mut reg = AgentRegistry::new();
        let info = reg.register_or_update("pid-123", "claude-code", 1000);
        assert_eq!(info.agent_label, "claude-1");
        assert_eq!(info.edit_count, 0);
    }

    #[test]
    fn test_second_agent_gets_label_2() {
        let mut reg = AgentRegistry::new();
        reg.register_or_update("pid-1", "claude-code", 1000);
        let info = reg.register_or_update("pid-2", "claude-code", 2000);
        assert_eq!(info.agent_label, "claude-2");
    }

    #[test]
    fn test_update_existing_agent() {
        let mut reg = AgentRegistry::new();
        reg.register_or_update("pid-1", "claude-code", 1000);
        reg.increment_edit_count("pid-1", 2000);
        let info = reg.get("pid-1").unwrap();
        assert_eq!(info.edit_count, 1);
        assert_eq!(info.last_seen, 2000);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib daemon::agent_registry::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement agent_registry.rs**

```rust
use crate::event::AgentInfo;
use std::collections::HashMap;

pub struct AgentRegistry {
    agents: HashMap<String, AgentInfo>,
    next_label: u32,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            next_label: 1,
        }
    }

    pub fn register_or_update(&mut self, agent_id: &str, tool_type: &str, ts: i64) -> &AgentInfo {
        if !self.agents.contains_key(agent_id) {
            let label = format!("claude-{}", self.next_label);
            self.next_label += 1;
            self.agents.insert(agent_id.to_string(), AgentInfo {
                agent_id: agent_id.to_string(),
                agent_label: label,
                tool_type: tool_type.to_string(),
                first_seen: ts,
                last_seen: ts,
                edit_count: 0,
            });
        } else {
            self.agents.get_mut(agent_id).unwrap().last_seen = ts;
        }
        &self.agents[agent_id]
    }

    pub fn increment_edit_count(&mut self, agent_id: &str, ts: i64) {
        if let Some(info) = self.agents.get_mut(agent_id) {
            info.edit_count += 1;
            info.last_seen = ts;
        }
    }

    pub fn get(&self, agent_id: &str) -> Option<&AgentInfo> {
        self.agents.get(agent_id)
    }

    pub fn all(&self) -> Vec<&AgentInfo> {
        self.agents.values().collect()
    }

    pub fn to_vec(&self) -> Vec<AgentInfo> {
        self.agents.values().cloned().collect()
    }
}
```

- [ ] **Step 4: Update daemon/mod.rs**

Add `pub mod agent_registry;`.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib daemon::agent_registry::tests 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add agent registry for multi-agent tracking"
```

---

### Task 13: Daemon main loop and CLI commands

**Files:**
- Modify: `src/daemon/mod.rs` (add daemon entry point)
- Modify: `src/main.rs` (add `Daemon` subcommand with start/stop/status)

- [ ] **Step 1: Implement daemon run loop in daemon/mod.rs**

Add to `src/daemon/mod.rs`:

```rust
pub mod agent_registry;
pub mod correlation;
pub mod hook_listener;
pub mod pid;

use crate::config::Config;
use crate::recorder::Recorder;
use crate::session::SessionManager;
use crate::watcher::fs_watcher::FsWatcher;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// Run the daemon in the current process (called by the spawned child).
pub fn run_daemon(project_path: PathBuf, config: Config) -> Result<()> {
    let vt_dir = project_path.join(".vibetracer");
    std::fs::create_dir_all(&vt_dir)?;

    let pid_path = vt_dir.join("daemon.pid");
    let sock_path = vt_dir.join("daemon.sock");

    // Create session
    let sessions_dir = vt_dir.join("sessions");
    let session_mgr = SessionManager::new(sessions_dir);
    let session = session_mgr.create()?;

    // Write PID file
    let my_pid = std::process::id() as i32;
    pid::write_pid_file(&pid_path, my_pid, &session.id)?;

    // Set up recorder
    let (event_tx, _event_rx) = mpsc::channel();
    let mut recorder = Recorder::new(project_path.clone(), session.dir.clone())?;

    // Set up file watcher
    let (fs_tx, fs_rx) = mpsc::channel::<PathBuf>();
    let mut watcher = FsWatcher::with_ignore(
        project_path.clone(),
        fs_tx,
        config.watch.debounce_ms,
        config.watch.ignore.clone(),
    )?;
    watcher.start()?;

    // Set up socket listener
    let (sock_tx, sock_rx) = mpsc::channel();
    let sock_path_clone = sock_path.clone();
    std::thread::spawn(move || {
        if let Err(e) = hook_listener::listen(&sock_path_clone, sock_tx) {
            eprintln!("socket listener error: {e}");
        }
    });

    // Set up correlator and agent registry
    let mut correlator = correlation::Correlator::new();
    let mut agent_reg = agent_registry::AgentRegistry::new();

    // Main loop
    loop {
        // Drain socket messages
        while let Ok(msg) = sock_rx.try_recv() {
            match msg {
                hook_listener::SocketMessage::Hook(payload, file) => {
                    let ts = chrono::Utc::now().timestamp_millis();
                    agent_reg.register_or_update(&payload.agent_id, "claude-code", ts);
                    correlator.push_enrichment(&file, payload);
                }
                hook_listener::SocketMessage::RestoreStart { restore_id, files } => {
                    correlator.register_restore(restore_id, &files);
                }
                hook_listener::SocketMessage::RestoreEnd { restore_id } => {
                    correlator.clear_restore(restore_id);
                }
                hook_listener::SocketMessage::Stop => {
                    // Clean shutdown
                    let _ = pid::cleanup_stale(&pid_path, &sock_path);
                    watcher.stop();
                    return Ok(());
                }
                hook_listener::SocketMessage::StatusQuery(mut stream) => {
                    use std::io::Write;
                    let status = serde_json::json!({
                        "type": "status",
                        "pid": my_pid,
                        "session_id": session.id,
                        "agents": agent_reg.to_vec(),
                    });
                    let _ = writeln!(stream, "{}", status);
                }
            }
        }

        // Drain file changes
        while let Ok(abs_path) = fs_rx.try_recv() {
            let rel_path = abs_path
                .strip_prefix(&project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| abs_path.to_string_lossy().to_string());

            // Check for restore tag (precedence over hook enrichment)
            let restore_id = correlator.pop_restore(&rel_path);

            // Check for hook enrichment (only if not a restore)
            let enrichment_payload = if restore_id.is_none() {
                correlator.pop_enrichment(&rel_path)
            } else {
                // Discard any pending enrichment for this file during restore
                let _ = correlator.pop_enrichment(&rel_path);
                None
            };

            // Build enrichment from hook data and restore tag
            let enrichment = if enrichment_payload.is_some() || restore_id.is_some() {
                let mut enr = crate::recorder::Enrichment {
                    agent_id: None, agent_label: None, operation_id: None,
                    operation_intent: None, tool_name: None, restore_id,
                };
                if let Some(payload) = enrichment_payload {
                    let label = agent_reg.get(&payload.agent_id)
                        .map(|a| a.agent_label.clone());
                    enr.agent_id = Some(payload.agent_id.clone());
                    enr.agent_label = label;
                    enr.operation_id = Some(payload.operation_id);
                    enr.operation_intent = payload.intent;
                    enr.tool_name = Some(payload.tool_name);
                }
                Some(enr)
            } else {
                None
            };

            if let Ok(Some(result)) = recorder.process_file_change(&abs_path, &event_tx, enrichment.as_ref()) {
                if let Some(ref aid) = result.event.agent_id {
                    agent_reg.increment_edit_count(aid, result.event.ts);
                }
            }
        }

        // Cleanup stale enrichments
        correlator.cleanup_stale(5000);

        std::thread::sleep(Duration::from_millis(50));
    }
}
```

The `Recorder::process_file_change` already accepts an `Enrichment` parameter (from Task 8), so the daemon passes enrichment data directly and the edit is written to the log with all metadata in a single append.

- [ ] **Step 2: Add Daemon subcommand to main.rs**

```rust
#[derive(clap::Subcommand)]
enum DaemonCommands {
    Start,
    Stop,
    Status,
}

// Add to Commands enum:
Daemon {
    #[command(subcommand)]
    command: DaemonCommands,
},
```

Add a hidden flag to `Cli` for the daemon child process:

```rust
/// Internal: run as daemon child process (do not use directly)
#[arg(long, hide = true)]
daemon_child: bool,
```

In `main()`, before the subcommand match, check this flag:

```rust
if cli.daemon_child {
    let project_path = resolve_path(cli.path.as_deref())?;
    let config = load_config_or_default(&project_path);
    return vibetracer::daemon::run_daemon(project_path, config);
}
```

Implement the match arms:
- `Start`: check/cleanup stale PID, spawn child via `std::process::Command::new(std::env::current_exe()?).arg("--daemon-child").arg(&project_path)` with `Stdio::null()` for all stdio, poll for PID file to appear (50ms intervals, 3s timeout)
- `Stop`: read PID from `.vibetracer/daemon.pid`, connect to `.vibetracer/daemon.sock`, send `{"type":"control","command":"stop"}\n`, wait for PID file to disappear
- `Status`: connect to socket, send status query, print response

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: Compiles (daemon not yet fully wired but should compile)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: daemon main loop with correlation, agent registry, and CLI commands"
```

---

## Phase 6: Restore System

### Task 14: Restore engine

**Files:**
- Create: `src/restore/mod.rs`
- Create: `src/restore/restore_log.rs`
- Create: `src/restore/conflict.rs`
- Modify: `src/lib.rs` (add `pub mod restore;`)

- [ ] **Step 1: Write tests for restore engine**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_restore_file_writes_content() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();

        let store = crate::snapshot::store::SnapshotStore::new(tmp.path().join("store"));
        let hash = store.store(b"original content").unwrap();

        // Write modified content
        std::fs::write(project.join("test.txt"), "modified").unwrap();

        let engine = RestoreEngine::new(project.clone(), store);
        engine.restore_file("test.txt", &hash).unwrap();

        let content = std::fs::read_to_string(project.join("test.txt")).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_restore_file_delete() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("new.txt"), "content").unwrap();

        let store = crate::snapshot::store::SnapshotStore::new(tmp.path().join("store"));
        let engine = RestoreEngine::new(project.clone(), store);
        engine.delete_file("new.txt").unwrap();

        assert!(!project.join("new.txt").exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib restore::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement RestoreEngine**

```rust
use crate::snapshot::store::SnapshotStore;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub mod conflict;
pub mod restore_log;

pub struct RestoreEngine {
    project_root: PathBuf,
    store: SnapshotStore,
}

impl RestoreEngine {
    pub fn new(project_root: PathBuf, store: SnapshotStore) -> Self {
        Self { project_root, store }
    }

    pub fn restore_file(&self, relative_path: &str, snapshot_hash: &str) -> Result<()> {
        let content = self.store.retrieve(snapshot_hash)
            .with_context(|| format!("retrieve snapshot for {relative_path}"))?;
        let dest = self.project_root.join(relative_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &content)
            .with_context(|| format!("write restored file {relative_path}"))?;
        Ok(())
    }

    pub fn delete_file(&self, relative_path: &str) -> Result<()> {
        let dest = self.project_root.join(relative_path);
        if dest.exists() {
            std::fs::remove_file(&dest)
                .with_context(|| format!("delete file {relative_path}"))?;
        }
        Ok(())
    }

    pub fn current_hash(&self, relative_path: &str) -> Result<String> {
        let content = std::fs::read(self.project_root.join(relative_path))?;
        self.store.store(&content)
    }
}
```

- [ ] **Step 4: Implement restore_log.rs**

```rust
use crate::event::{RestoreEvent, RestoreFileEntry, RestoreScope};
use anyhow::Result;
use std::path::PathBuf;

pub struct RestoreLog {
    path: PathBuf,
    next_id: u64,
}

impl RestoreLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path, next_id: 1 }
    }

    pub fn append(&mut self, scope: RestoreScope, files: Vec<RestoreFileEntry>) -> Result<RestoreEvent> {
        let event = RestoreEvent {
            id: self.next_id,
            ts: chrono::Utc::now().timestamp_millis(),
            scope,
            files_restored: files,
        };
        self.next_id += 1;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let json = serde_json::to_string(&event)?;
        writeln!(file, "{}", json)?;

        Ok(event)
    }

    pub fn read_all(&self) -> Result<Vec<RestoreEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        use std::io::BufRead;
        let file = std::fs::File::open(&self.path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(event) = serde_json::from_str(&line) {
                events.push(event);
            }
        }
        Ok(events)
    }
}
```

- [ ] **Step 5: Implement conflict.rs skeleton**

```rust
use crate::config::BlastRadiusConfig;
use std::collections::HashSet;

pub struct ConflictChecker {
    config: BlastRadiusConfig,
}

pub struct ConflictSuggestion {
    pub coupled_files: Vec<String>,
    pub reason: String,
}

impl ConflictChecker {
    pub fn new(config: BlastRadiusConfig) -> Self {
        Self { config }
    }

    pub fn check_restore_conflicts(
        &self,
        files_to_restore: &[String],
        edited_files: &HashSet<String>,
    ) -> Vec<ConflictSuggestion> {
        let mut suggestions = Vec::new();
        for dep in &self.config.manual {
            let source_pattern = glob::Pattern::new(&dep.source).ok();
            for file in files_to_restore {
                let matches = source_pattern.as_ref().map(|p| p.matches(file)).unwrap_or(false);
                if matches {
                    let coupled: Vec<String> = dep.dependents.iter()
                        .filter(|d| {
                            let pat = glob::Pattern::new(d).ok();
                            edited_files.iter().any(|ef| {
                                pat.as_ref().map(|p| p.matches(ef)).unwrap_or(false)
                            }) && !files_to_restore.contains(&d.to_string())
                        })
                        .cloned()
                        .collect();
                    if !coupled.is_empty() {
                        suggestions.push(ConflictSuggestion {
                            coupled_files: coupled,
                            reason: format!("{} has declared dependents that were also edited", file),
                        });
                    }
                }
            }
        }
        suggestions
    }
}
```

- [ ] **Step 6: Add `pub mod restore;` to lib.rs**

- [ ] **Step 7: Run tests**

Run: `cargo test --lib restore 2>&1`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add restore engine, restore log, and conflict checker"
```

---

## Phase 7: TUI v2 (per-file playheads, operation grouping, new keybindings)

### Task 15: Extract App state into tui/app.rs

**Files:**
- Create: `src/tui/app.rs`
- Modify: `src/tui/mod.rs` (move App struct out, import from app.rs)

- [ ] **Step 1: Create app.rs with App struct**

Move the `App` struct, `Pane` enum, `SidebarPanel` enum, `PlaybackState` enum, `TrackInfo` struct, and all `App` methods from `tui/mod.rs` into `src/tui/app.rs`. Add the new v2 fields:

```rust
// New fields in App:
pub global_playhead: usize,
pub file_playheads: HashMap<String, usize>,
pub detached_files: HashSet<String>,
pub operations: HashMap<String, OperationGroup>,
pub command_view: bool,      // toggle between edit view and command view
pub show_restore_edits: bool, // toggle showing restore-generated edits
```

Remove: `equation_lens`, `schema_diff_mode`, `terminal`, `terminal_output`, `terminal_visible`, `last_vibetracer_pane`, `equations`.

- [ ] **Step 2: Update tui/mod.rs to import from app.rs**

Replace the inline `App` definition with `pub mod app;` and `pub use app::*;`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: extract App state into tui/app.rs"
```

---

### Task 16: Per-file playhead logic

**Files:**
- Create: `src/tui/playhead.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_scrub_maps_to_file_positions() {
        let mut ph = PlayheadManager::new();
        // a.rs was edited at global positions 0, 2, 4
        ph.register_file("a.rs", vec![0, 2, 4]);
        // b.rs was edited at global positions 1, 3
        ph.register_file("b.rs", vec![1, 3]);

        ph.set_global(3); // after 4 edits total
        // a.rs: edits at 0, 2 are <= 3, so per-file position = 1 (0-indexed, 2 edits seen)
        assert_eq!(ph.file_position("a.rs"), Some(1));
        // b.rs: edits at 1, 3 are <= 3, so per-file position = 1
        assert_eq!(ph.file_position("b.rs"), Some(1));
    }

    #[test]
    fn test_detached_file_not_moved_by_global() {
        let mut ph = PlayheadManager::new();
        ph.register_file("a.rs", vec![0, 2, 4, 6, 8]);
        ph.register_file("b.rs", vec![1, 3, 5, 7, 9]);
        ph.detach("a.rs", 1);
        ph.set_global(9);
        assert_eq!(ph.file_position("a.rs"), Some(1)); // stayed at 1 (detached)
        assert_eq!(ph.file_position("b.rs"), Some(4)); // moved with global (5 edits seen, pos 4)
    }

    #[test]
    fn test_reattach_snaps_to_global() {
        let mut ph = PlayheadManager::new();
        ph.register_file("a.rs", vec![0, 1, 2, 3, 4]);
        ph.detach("a.rs", 1);
        ph.set_global(3);
        ph.reattach("a.rs");
        // After reattach, file position follows global: edits 0,1,2,3 are <= 3, pos = 3
        assert_eq!(ph.file_position("a.rs"), Some(3));
        assert!(!ph.is_detached("a.rs"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::playhead::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement PlayheadManager**

```rust
use std::collections::{HashMap, HashSet};

/// Maps global playhead position to per-file positions.
/// Each file has its own edit indices (indices into the flat edits list).
/// The global playhead is an index into the flat list; a file's position
/// is the number of that file's edits that have occurred up to the global position.
pub struct PlayheadManager {
    global: usize,
    /// Maps filename -> sorted list of global edit indices for that file
    file_edit_indices: HashMap<String, Vec<usize>>,
    detached: HashMap<String, usize>, // file -> detached per-file position
}

impl PlayheadManager {
    pub fn new() -> Self {
        Self {
            global: 0,
            file_edit_indices: HashMap::new(),
            detached: HashMap::new(),
        }
    }

    /// Register a file's edit indices (sorted indices into the global edit list).
    pub fn register_file(&mut self, file: &str, global_indices: Vec<usize>) {
        self.file_edit_indices.insert(file.to_string(), global_indices);
    }

    pub fn set_global(&mut self, pos: usize) {
        self.global = pos;
    }

    pub fn global(&self) -> usize {
        self.global
    }

    /// Get a file's current per-file playhead position.
    /// For non-detached files, this is the count of that file's edits
    /// up to and including the global playhead position.
    pub fn file_position(&self, file: &str) -> Option<usize> {
        if let Some(&pos) = self.detached.get(file) {
            return Some(pos);
        }
        let indices = self.file_edit_indices.get(file)?;
        // Count how many of this file's edits are at or before the global playhead
        let count = indices.iter().filter(|&&idx| idx <= self.global).count();
        Some(count.saturating_sub(1))
    }

    pub fn file_edit_count(&self, file: &str) -> usize {
        self.file_edit_indices.get(file).map(|v| v.len()).unwrap_or(0)
    }

    pub fn detach(&mut self, file: &str, pos: usize) {
        self.detached.insert(file.to_string(), pos);
    }

    pub fn reattach(&mut self, file: &str) {
        self.detached.remove(file);
    }

    pub fn is_detached(&self, file: &str) -> bool {
        self.detached.contains_key(file)
    }

    pub fn scrub_file_left(&mut self, file: &str) {
        let current = self.detached.get(file).copied()
            .or_else(|| self.file_position(file))
            .unwrap_or(0);
        let new_pos = current.saturating_sub(1);
        self.detached.insert(file.to_string(), new_pos);
    }

    pub fn scrub_file_right(&mut self, file: &str) {
        let max = self.file_edit_count(file).saturating_sub(1);
        let current = self.detached.get(file).copied()
            .or_else(|| self.file_position(file))
            .unwrap_or(0);
        let new_pos = (current + 1).min(max);
        self.detached.insert(file.to_string(), new_pos);
    }
}
```

- [ ] **Step 4: Add `pub mod playhead;` to tui/mod.rs**

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::playhead::tests 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: per-file playhead manager with detach/reattach"
```

---

### Task 17: Operation grouping

**Files:**
- Create: `src/tui/operation.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_by_operation_id() {
        let mut mgr = OperationManager::new();
        mgr.add_edit(0, Some("op-1".to_string()), Some("a1".to_string()), None, Some("refactor".to_string()), "src/a.rs".to_string(), 1000, 1100);
        mgr.add_edit(1, Some("op-1".to_string()), Some("a1".to_string()), None, None, "src/b.rs".to_string(), 1050, 1100);

        let groups = mgr.groups_ordered();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].edits.len(), 2);
        assert_eq!(groups[0].files_touched.len(), 2);
    }

    #[test]
    fn test_ungrouped_edits_get_singleton() {
        let mut mgr = OperationManager::new();
        mgr.add_edit(0, None, None, None, None, "src/a.rs".to_string(), 1000, 1000);

        let groups = mgr.groups_ordered();
        assert_eq!(groups.len(), 1);
        assert!(groups[0].operation_id.starts_with("ungrouped-"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::operation::tests 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement OperationManager**

```rust
use std::collections::HashMap;

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

pub struct OperationManager {
    groups: HashMap<String, OperationGroup>,
}

impl OperationManager {
    pub fn new() -> Self {
        Self { groups: HashMap::new() }
    }

    pub fn add_edit(
        &mut self,
        edit_index: usize,
        operation_id: Option<String>,
        agent_id: Option<String>,
        agent_label: Option<String>,
        intent: Option<String>,
        file: String,
        ts: i64,
        _ts_end: i64,
    ) {
        let op_id = operation_id.unwrap_or_else(|| format!("ungrouped-{}", edit_index));

        if let Some(group) = self.groups.get_mut(&op_id) {
            group.edits.push(edit_index);
            group.ts_end = ts;
            if !group.files_touched.contains(&file) {
                group.files_touched.push(file);
            }
        } else {
            self.groups.insert(op_id.clone(), OperationGroup {
                operation_id: op_id,
                agent_id,
                agent_label,
                intent,
                edits: vec![edit_index],
                files_touched: vec![file],
                ts_start: ts,
                ts_end: ts,
            });
        }
    }

    pub fn groups_ordered(&self) -> Vec<&OperationGroup> {
        let mut groups: Vec<&OperationGroup> = self.groups.values().collect();
        groups.sort_by_key(|g| g.ts_start);
        groups
    }

    pub fn get(&self, operation_id: &str) -> Option<&OperationGroup> {
        self.groups.get(operation_id)
    }
}
```

- [ ] **Step 4: Add `pub mod operation;` to tui/mod.rs**

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::operation::tests 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: operation grouping for command-level timeline view"
```

---

### Task 18: Update input.rs with v2 keybindings

**Files:**
- Modify: `src/tui/input.rs`

- [ ] **Step 1: Rewrite Action enum and map_key for v2**

Replace the `Action` enum with:

```rust
pub enum Action {
    Quit,
    QuitAndStopDaemon,
    Help,
    TogglePlay,
    ScrubLeft,
    ScrubRight,
    FilesScrubLeft,    // shift+left
    FilesScrubRight,   // shift+right
    Reattach,          // 'a'
    ToggleCommandView, // 'g'
    Restore,           // 'R'
    RestoreAgentRange, // shift+R (command view only)
    UndoRestore,       // 'u'
    Checkpoint,        // 'c'
    ToggleRestoreEdits, // 'x'
    SoloTrack,         // 's'
    MuteTrack,         // 'm'
    ToggleBlastRadius, // 'b'
    ToggleSentinels,   // 'i'
    ToggleWatchdog,    // 'w'
    CycleTheme,        // 't'
    SoloAgent(u8),     // '1'-'9' (command view)
    Noop,
}
```

Update `map_key` to match the v2 keybindings table from the spec. Remove speed control keys. Add `Q` for `QuitAndStopDaemon`.

- [ ] **Step 2: Update apply_action for new actions**

Add handlers for: `Reattach`, `ToggleCommandView`, `Restore` (sets a flag for the event loop to handle), `UndoRestore`, `ToggleRestoreEdits`, `CycleTheme`, `SoloAgent(n)`, `FilesScrubLeft`, `FilesScrubRight`, `QuitAndStopDaemon`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: v2 keybindings with per-file scrub, command view, restore, themes"
```

---

### Task 19: Extract event loop into tui/event_loop.rs

**Files:**
- Create: `src/tui/event_loop.rs`
- Modify: `src/tui/mod.rs` (slim down to setup + delegate to event_loop)

- [ ] **Step 1: Move the main loop from run_tui_with_options into event_loop.rs**

Create `src/tui/event_loop.rs` containing a `run_event_loop` function that takes `App`, `Terminal`, `Recorder` (optional, for --no-daemon mode), edit log path (for tailing), config, and project path. The function encapsulates:
- Edit log tailing (watch edits.jsonl for new lines via notify)
- Crossterm event polling
- Input handling dispatch
- Analysis engine invocation
- Rendering

- [ ] **Step 2: Update tui/mod.rs to delegate**

`run_tui_with_options` becomes thin: setup terminal, setup app, determine session directory (from daemon PID or create new session for --no-daemon), call `event_loop::run_event_loop(...)`.

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cargo build 2>&1 && cargo test 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: extract TUI event loop into tui/event_loop.rs"
```

---

### Task 20: Update timeline widget for agent colors and command view

**Files:**
- Modify: `src/tui/widgets/timeline.rs`

- [ ] **Step 1: Update timeline rendering**

The timeline widget needs to support two modes:
- **Edit view**: Same as v1 but with agent-colored edit blocks (use theme.agent_colors indexed by agent)
- **Command view**: Each cell represents an OperationGroup instead of an individual edit

Add agent color lookup: if `edit.agent_id` is Some, find the agent's index in the session's agent list, use `theme.agent_colors[index % 6]` (with dimming for index >= 6).

Show detached file playheads in `theme.accent_purple` instead of `theme.accent_warm`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: timeline widget with agent colors and command view mode"
```

---

### Task 21: Add restore confirmation widget

**Files:**
- Create: `src/tui/widgets/restore_confirm.rs`
- Modify: `src/tui/widgets/mod.rs`

- [ ] **Step 1: Implement restore confirmation dialog**

A modal overlay widget that shows:
- Scope description ("Restore 3 files to pre-operation state?")
- File list with line count changes
- Conflict suggestions (coupled files)
- "Enter to confirm, Esc to cancel"

- [ ] **Step 2: Add to widgets/mod.rs**

Add `pub mod restore_confirm;`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: restore confirmation dialog widget"
```

---

### Task 22: Update status bar, help overlay, keybindings bar

**Files:**
- Modify: `src/tui/widgets/status_bar.rs`
- Modify: `src/tui/widgets/help_overlay.rs`
- Modify: `src/tui/mod.rs` (keybindings bar)

- [ ] **Step 1: Update status bar**

Show: active agents (with labels and idle time), current theme name (flash for 2s after switch), command view indicator, detached file count.

- [ ] **Step 2: Update help overlay**

Update all keybinding descriptions to match the v2 keybindings table.

- [ ] **Step 3: Update keybindings bar**

Remove equation, schema diff, refactor keys. Add: `g` command view, `t` theme, `R` restore.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: update status bar, help overlay, keybindings bar for v2"
```

---

## Phase 8: Checkpoint Module (extract from snapshot/)

### Task 23: Move checkpoint to its own module

**Files:**
- Create: `src/checkpoint/mod.rs` (move content from `src/snapshot/checkpoint.rs`)
- Delete content from: `src/snapshot/checkpoint.rs` (or delete file)
- Modify: `src/snapshot/mod.rs` (remove `pub mod checkpoint;`)
- Modify: `src/lib.rs` (add `pub mod checkpoint;`)
- Update all imports across codebase

- [ ] **Step 1: Move checkpoint.rs to checkpoint/mod.rs**

```bash
mkdir -p src/checkpoint
mv src/snapshot/checkpoint.rs src/checkpoint/mod.rs
```

- [ ] **Step 2: Update imports**

Replace `crate::snapshot::checkpoint::CheckpointManager` with `crate::checkpoint::CheckpointManager` everywhere. Remove `pub mod checkpoint;` from `src/snapshot/mod.rs`. Add `pub mod checkpoint;` to `src/lib.rs`.

- [ ] **Step 3: Verify tests pass**

Run: `cargo test 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: move checkpoint module out of snapshot/ (TUI-only concern)"
```

---

## Phase 9: Hook Registration Update

### Task 24: Update Claude Code hook for v2 protocol

**Files:**
- Modify: `src/hook/registration.rs`

- [ ] **Step 1: Update hook command to send JSON to daemon socket**

The v1 hook uses `echo '$TOOL_NAME $TOOL_INPUT' | nc -U <socket>`. The v2 hook should send proper JSON:

```bash
echo '{"type":"hook","agent_id":"'$PPID'","operation_id":"'$CLAUDE_SESSION_ID'-'$TOOL_USE_ID'","tool_name":"'$TOOL_NAME'","file":"'$(echo $TOOL_INPUT | jq -r '.file_path // .path // empty')'"}'  | nc -U <socket>
```

Note: The exact environment variables available in Claude Code hooks need to be verified. Use `$PPID` for agent_id as the Claude Code process PID.

- [ ] **Step 2: Update tests**

Verify the new hook command format is written to settings.local.json correctly.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib hook 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: update Claude Code hook for v2 JSON protocol"
```

---

## Phase 10: Wire Everything Together

### Task 25: TUI auto-start daemon and connect to session

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement daemon auto-start in TUI startup**

In `run_tui_with_options`, before entering the event loop:

1. Check for `.vibetracer/daemon.pid`
2. If exists and alive: read session ID, set session dir
3. If exists and dead: cleanup stale, start new daemon
4. If not exists: start new daemon
5. If `--no-daemon`: skip all of the above, create in-process recorder

- [ ] **Step 2: Implement edit log tailing**

Use the `notify` crate to watch `edits.jsonl` for changes. On change event, read new lines from the last known position using a `BufReader` with retained seek position. Parse new `EditEvent`s and push into `App`.

- [ ] **Step 3: Wire `Q` to send stop signal to daemon**

When the user presses `Q`, send a stop message over the Unix socket before quitting the TUI.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 5: Manual test: start daemon, start TUI, make edits, verify they appear**

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: TUI auto-starts daemon and tails edit log"
```

---

### Task 26: Wire restore actions in the TUI event loop

**Files:**
- Modify: `src/tui/event_loop.rs`

- [ ] **Step 1: Handle Restore action**

When the user presses `R`:
1. Determine scope (file restore if in edit view with a track focused, operation restore if in command view)
2. Build the restore plan (which files, which hashes)
3. Show restore confirmation widget
4. On confirm: send `restore_start` to daemon socket, write files via RestoreEngine, send `restore_end`, log RestoreEvent
5. On cancel: dismiss dialog

- [ ] **Step 2: Handle UndoRestore action**

Read the last `RestoreEvent` from `restores.jsonl`, reverse it (write `from_hash` content back for each file).

- [ ] **Step 3: Handle ToggleRestoreEdits**

Toggle `app.show_restore_edits` flag. When false, filter edits with `restore_id.is_some()` from the timeline display.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: wire restore, undo-restore, and restore edit filtering in TUI"
```

---

### Task 27: Wire theme cycling

**Files:**
- Modify: `src/tui/event_loop.rs` or `src/tui/input.rs`
- Modify: `src/tui/app.rs`

- [ ] **Step 1: Add theme cycling logic**

Add a `THEME_PRESETS` const array with all 19 preset names. On `CycleTheme` action, find the current preset in the array, advance to the next one, rebuild the theme.

```rust
const THEME_PRESETS: &[&str] = &[
    "dark", "catppuccin-mocha", "catppuccin-macchiato", "gruvbox-dark",
    "tokyo-night", "tokyo-night-storm", "dracula", "nord", "kanagawa",
    "rose-pine", "one-dark", "solarized-dark", "everforest-dark",
    "light", "catppuccin-latte", "gruvbox-light", "solarized-light",
    "rose-pine-dawn", "everforest-light",
];
```

- [ ] **Step 2: Add theme name flash to status bar**

Store `theme_name_flash: Option<(String, std::time::Instant)>` in App. Set it on theme change. Clear after 2 seconds. Status bar renders the flash if active.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: theme cycling with status bar flash"
```

---

## Phase 11: Update Import

### Task 28: Update Claude import for v2 fields

**Files:**
- Modify: `src/import/claude.rs`

- [ ] **Step 1: Add agent fields to imported events**

When importing Claude Code JSONL sessions, populate `agent_id` (from the session metadata), `operation_id` (from tool use IDs), `tool_name`, and `operation_intent` (from assistant messages). This enriches imported sessions with the same data that live sessions get from hooks.

- [ ] **Step 2: Verify import still works**

Run: `cargo test --lib import 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: import v2 agent/operation fields from Claude Code JSONL"
```

---

---

## Phase 12: Missing CLI Commands

### Task 32: Add CLI restore subcommand

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add Restore variant to Commands enum**

```rust
/// Restore a file or operation to a prior state (headless, no TUI)
Restore {
    /// File path to restore (for file-scope restore)
    file: Option<String>,
    /// Edit ID to restore to (EditEvent.id)
    #[arg(long)]
    edit_id: Option<u64>,
    /// Operation ID to restore (restores all files in operation)
    #[arg(long)]
    operation: Option<String>,
},
```

- [ ] **Step 2: Implement the Restore match arm**

Read `edits.jsonl` from the active session (find via `daemon.pid` or most recent session). Look up the target edit by `EditEvent.id`. Use `RestoreEngine` to write files. Connect to daemon socket to send `restore_start`/`restore_end` if daemon is running. Write `RestoreEvent` to `restores.jsonl`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add headless CLI restore subcommand"
```

---

### Task 33: Update sessions and replay for v2

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update Replay command**

The replay command should read from an existing session's `edits.jsonl` and open the TUI in read-only mode (same as live mode but no daemon, no new edits). Remove the in-process watcher setup. Just load edits and enter the TUI viewer.

- [ ] **Step 2: Update Sessions command**

Add agent count and edit count to the session listing. Read from `meta.json` (which now includes `agents` field).

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: update sessions/replay commands for v2 daemon model"
```

---

## Phase 13: Final Integration and Cleanup

### Task 30: Full integration test

**Files:**
- All

- [ ] **Step 1: Run full test suite**

Run: `cargo test 2>&1`
Expected: All tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy 2>&1`
Expected: No errors (warnings acceptable for now)

- [ ] **Step 3: Run cargo fmt**

Run: `cargo fmt`

- [ ] **Step 4: Clean up any dead imports or unused variables**

Fix any remaining compiler warnings about unused imports, dead code, etc.

- [ ] **Step 5: Verify the binary runs**

Run: `cargo run -- --help`
Expected: Shows v2 CLI help with `daemon`, `restore` subcommands

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "chore: final cleanup — clippy, fmt, dead code removal"
```

---

### Task 31: Update lib.rs module declarations

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Final lib.rs should declare exactly these modules**

```rust
pub mod analysis;
pub mod auto_detect;
pub mod checkpoint;
pub mod config;
pub mod daemon;
pub mod event;
pub mod hook;
pub mod import;
pub mod recorder;
pub mod restore;
pub mod session;
pub mod snapshot;
pub mod theme;
pub mod tui;
pub mod watcher;
```

No: `demo`, `equation`, `pty`, `rewind`, `splash`.

- [ ] **Step 2: Verify**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "chore: finalize lib.rs module declarations for v2"
```
