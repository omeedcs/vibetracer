# Vibetracer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust TUI that traces, replays, and rewinds filesystem edits made by AI coding assistants, with optional Claude Code integration for intent-aware tracking.

**Architecture:** Single binary with four subsystems (Watcher, Hook Bridge, Snapshot Engine, Renderer) communicating via an in-process event bus. Data persisted to `.vibetracer/` as append-only JSONL logs and content-addressed file snapshots. TUI built with ratatui in a Director's Cut layout (preview pane + horizontal multi-track timeline).

**Tech Stack:** Rust, ratatui, crossterm, notify, similar, tree-sitter, serde, clap, toml

**Spec:** `docs/superpowers/specs/2026-03-20-vibetracer-design.md`

---

## File Structure

```
vibetracer/
  Cargo.toml
  src/
    main.rs                     # CLI entry point (clap), startup sequence
    lib.rs                      # Re-exports for library use
    config.rs                   # Config struct, TOML parsing, defaults
    event.rs                    # Edit event types, event bus (mpsc channel)
    session.rs                  # Session ID generation, meta.json, listing
    snapshot/
      mod.rs                    # Public API
      store.rs                  # Content-addressed file storage (SHA-256)
      edit_log.rs               # Append-only JSONL edit log
      checkpoint.rs             # Full project state snapshots
    watcher/
      mod.rs                    # Public API
      fs_watcher.rs             # notify-based filesystem watcher with debounce
      differ.rs                 # Diff computation (similar crate)
    hook/
      mod.rs                    # Public API
      bridge.rs                 # Unix socket listener, hook payload parsing
      registration.rs           # Write/remove Claude Code hook config
    rewind/
      mod.rs                    # Rewind logic: restore snapshots to disk
    analysis/
      mod.rs                    # Public API
      blast_radius.rs           # Dependency graph, stale file detection
      sentinels.rs              # Invariant rule evaluation
      watchdog.rs               # Constants monitoring
      refactor_tracker.rs       # Rename propagation tracking
      schema_diff.rs            # Structural diff for schemas
      imports.rs                # tree-sitter import parsing
    equation/
      mod.rs                    # Equation detection and rendering pipeline
      detect.rs                 # LaTeX/math pattern extraction from source
      render.rs                 # tectonic/katex/unicode rendering backends
    tui/
      mod.rs                    # App state, main loop, event dispatch
      input.rs                  # Keybinding handling
      layout.rs                 # Zone layout computation (preview, timeline, sidebar)
      widgets/
        mod.rs                  # Widget re-exports
        status_bar.rs           # Top bar: session info, connection status
        preview.rs              # Diff/file preview with syntax highlighting
        timeline.rs             # Horizontal per-file tracks with playhead
        sidebar.rs              # Toggleable right panel container
        blast_radius_panel.rs   # Blast radius display
        sentinel_panel.rs       # Sentinel violations display
        watchdog_panel.rs       # Watchdog alerts display
        refactor_panel.rs       # Refactor progress display
        equation_panel.rs       # Equation index display
        help_overlay.rs         # ? key overlay
  tests/
    integration/
      mod.rs
      watcher_test.rs           # Filesystem watcher integration tests
      snapshot_test.rs          # Snapshot store round-trip tests
      session_test.rs           # Session create/list/replay tests
      rewind_test.rs            # Rewind + undo tests
      hook_test.rs              # Hook bridge socket tests
      config_test.rs            # Config parsing tests
      sentinel_test.rs          # Sentinel rule evaluation tests
      watchdog_test.rs          # Constants watchdog tests
  .github/
    workflows/
      ci.yml                    # Test + lint on push/PR
      release.yml               # Cross-compile + publish on tag
  homebrew/
    vibetracer.rb               # Homebrew formula template
  scripts/
    install.sh                  # curl-pipe-sh installer
```

---

## Phase 1: Core Foundation

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Initialize cargo project**

```bash
cd /Users/omeedtehrani/vibetracer
cargo init --name vibetracer
```

- [ ] **Step 2: Set up Cargo.toml with dependencies**

```toml
[package]
name = "vibetracer"
version = "0.1.0"
edition = "2024"
description = "Real-time tracing, replaying, and rewinding of AI coding assistant edits"
license = "MIT"
repository = "https://github.com/omeedtehrani/vibetracer"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
notify = "7"
notify-debouncer-mini = "0.5"
similar = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
rand = "0.9"
glob = "0.3"
regex = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
dirs = "6"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 3: Create minimal main.rs**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "vibetracer", about = "Trace, replay, and rewind AI coding edits")]
struct Cli {
    /// Project directory to watch (defaults to current directory)
    path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Replay a past session
    Replay { session_id: String },
    /// List past sessions
    Sessions,
    /// Create default config
    Init,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("vibetracer v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

- [ ] **Step 4: Create lib.rs**

```rust
pub mod config;
pub mod event;
pub mod session;
pub mod snapshot;
pub mod watcher;
```

- [ ] **Step 5: Create empty module files**

Create `src/config.rs`, `src/event.rs`, `src/session.rs`, `src/snapshot/mod.rs`, `src/watcher/mod.rs` as empty files with just a comment placeholder.

- [ ] **Step 6: Verify it compiles**

Run: `cargo build`
Expected: Compiles with warnings about unused modules.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: project scaffold with dependencies and CLI skeleton"
```

---

### Task 2: Config Module

**Files:**
- Create: `src/config.rs`
- Create: `tests/integration/config_test.rs`
- Create: `tests/integration/mod.rs`

- [ ] **Step 1: Write failing test for config defaults**

```rust
// tests/integration/config_test.rs
use vibetracer::config::Config;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.watch.debounce_ms, 100);
    assert_eq!(config.watch.auto_checkpoint_every, 25);
    assert!(config.watch.ignore.contains(&".git".to_string()));
    assert!(config.watch.ignore.contains(&".vibetracer".to_string()));
}

#[test]
fn test_config_from_toml() {
    let toml_str = r#"
[watch]
debounce_ms = 200
auto_checkpoint_every = 10
ignore = [".git", "node_modules"]

[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.watch.debounce_ms, 200);
    assert_eq!(config.watchdog.constants.len(), 1);
    assert_eq!(config.watchdog.constants[0].expected, "6371.0");
}

#[test]
fn test_config_generates_default_toml() {
    let config = Config::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    assert!(toml_str.contains("debounce_ms"));
    assert!(toml_str.contains("auto_checkpoint_every"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test integration`
Expected: FAIL — `Config` not defined.

- [ ] **Step 3: Implement Config**

```rust
// src/config.rs
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub sentinels: std::collections::HashMap<String, SentinelRule>,
    #[serde(default)]
    pub watchdog: WatchdogConfig,
    #[serde(default)]
    pub blast_radius: BlastRadiusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    pub debounce_ms: u64,
    pub ignore: Vec<String>,
    pub auto_checkpoint_every: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelRule {
    pub description: String,
    pub watch: Vec<String>,
    pub rule: String,
    #[serde(default)]
    pub pattern_a: Option<PatternSpec>,
    #[serde(default)]
    pub pattern_b: Option<PatternSpec>,
    #[serde(default)]
    pub assert: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSpec {
    pub file: String,
    pub regex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WatchdogConfig {
    #[serde(default)]
    pub constants: Vec<WatchdogConstant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConstant {
    pub file: String,
    pub pattern: String,
    pub expected: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusConfig {
    pub auto_detect: bool,
    #[serde(default)]
    pub manual: Vec<ManualDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualDependency {
    pub source: String,
    pub dependents: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watch: WatchConfig::default(),
            sentinels: std::collections::HashMap::new(),
            watchdog: WatchdogConfig::default(),
            blast_radius: BlastRadiusConfig::default(),
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 100,
            ignore: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".vibetracer".into(),
            ],
            auto_checkpoint_every: 25,
        }
    }
}

impl Default for BlastRadiusConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            manual: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test integration`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs tests/
git commit -m "feat: config module with TOML parsing and defaults"
```

---

### Task 3: Event Types

**Files:**
- Create: `src/event.rs`

- [ ] **Step 1: Write failing test**

```rust
// At bottom of src/event.rs (unit test)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_event_serialization() {
        let edit = EditEvent {
            id: 1,
            ts: 1710934532,
            file: "src/main.rs".into(),
            kind: EditKind::Modify,
            patch: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new".into(),
            before_hash: Some("abc123".into()),
            after_hash: "def456".into(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 1,
        };
        let json = serde_json::to_string(&edit).unwrap();
        let roundtrip: EditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.id, 1);
        assert_eq!(roundtrip.file, "src/main.rs");
    }

    #[test]
    fn test_edit_kind_variants() {
        let create = EditKind::Create;
        let modify = EditKind::Modify;
        let delete = EditKind::Delete;
        assert_ne!(format!("{create:?}"), format!("{modify:?}"));
        assert_ne!(format!("{modify:?}"), format!("{delete:?}"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test event::tests`
Expected: FAIL — types not defined.

- [ ] **Step 3: Implement event types**

```rust
// src/event.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditEvent {
    pub id: u64,
    pub ts: i64,
    pub file: String,
    pub kind: EditKind,
    pub patch: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
    pub intent: Option<String>,
    pub tool: Option<String>,
    pub lines_added: u32,
    pub lines_removed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    Create,
    Modify,
    Delete,
}

/// Messages sent through the internal event bus.
#[derive(Debug, Clone)]
pub enum BusEvent {
    /// A file was edited (from watcher or hook bridge).
    Edit(EditEvent),
    /// A hook payload arrived with enrichment data.
    HookEnrichment {
        file: String,
        tool: String,
        intent: Option<String>,
    },
    /// User requested a checkpoint.
    Checkpoint,
    /// Tick for playback advancement.
    PlaybackTick,
    /// User input event from crossterm.
    Input(crossterm::event::Event),
    /// Quit signal.
    Quit,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test event::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/event.rs
git commit -m "feat: event types for edit tracking and internal bus"
```

---

### Task 4: Snapshot Store

**Files:**
- Create: `src/snapshot/mod.rs`
- Create: `src/snapshot/store.rs`
- Create: `src/snapshot/edit_log.rs`
- Create: `src/snapshot/checkpoint.rs`
- Create: `tests/integration/snapshot_test.rs`

- [ ] **Step 1: Write failing tests for content-addressed store**

```rust
// tests/integration/snapshot_test.rs
use tempfile::TempDir;
use vibetracer::snapshot::store::SnapshotStore;

#[test]
fn test_store_and_retrieve_content() {
    let dir = TempDir::new().unwrap();
    let store = SnapshotStore::new(dir.path().to_path_buf());

    let content = b"fn main() { println!(\"hello\"); }";
    let hash = store.store(content).unwrap();

    // Same content produces same hash
    let hash2 = store.store(content).unwrap();
    assert_eq!(hash, hash2);

    // Retrieve returns original content
    let retrieved = store.retrieve(&hash).unwrap();
    assert_eq!(retrieved, content);
}

#[test]
fn test_store_deduplicates() {
    let dir = TempDir::new().unwrap();
    let store = SnapshotStore::new(dir.path().to_path_buf());

    let content = b"same content";
    store.store(content).unwrap();
    store.store(content).unwrap();

    // Only one file on disk (2-char prefix dir + hash file)
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .collect();
    assert_eq!(entries.len(), 1); // one prefix directory
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test snapshot_test`
Expected: FAIL — `SnapshotStore` not defined.

- [ ] **Step 3: Implement SnapshotStore**

```rust
// src/snapshot/store.rs
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub struct SnapshotStore {
    base_dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn store(&self, content: &[u8]) -> anyhow::Result<String> {
        let hash = Self::hash(content);
        let path = self.hash_to_path(&hash);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
        }

        Ok(hash)
    }

    pub fn retrieve(&self, hash: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.hash_to_path(hash);
        Ok(std::fs::read(path)?)
    }

    pub fn exists(&self, hash: &str) -> bool {
        self.hash_to_path(hash).exists()
    }

    fn hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    fn hash_to_path(&self, hash: &str) -> PathBuf {
        let (prefix, rest) = hash.split_at(2);
        self.base_dir.join(prefix).join(rest)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test snapshot_test`
Expected: PASS

- [ ] **Step 5: Write failing tests for edit log**

```rust
// tests/integration/snapshot_test.rs (append to file)
use vibetracer::snapshot::edit_log::EditLog;
use vibetracer::event::{EditEvent, EditKind};

#[test]
fn test_edit_log_append_and_read() {
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("edits.jsonl");
    let mut log = EditLog::new(log_path.clone());

    let edit = EditEvent {
        id: 1,
        ts: 1710934532,
        file: "src/main.rs".into(),
        kind: EditKind::Modify,
        patch: "-old\n+new".into(),
        before_hash: Some("aaa".into()),
        after_hash: "bbb".into(),
        intent: None,
        tool: None,
        lines_added: 1,
        lines_removed: 1,
    };

    log.append(&edit).unwrap();
    log.append(&edit).unwrap();

    let edits = EditLog::read_all(&log_path).unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].id, 1);
}
```

- [ ] **Step 6: Implement EditLog**

```rust
// src/snapshot/edit_log.rs
use crate::event::EditEvent;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub struct EditLog {
    path: PathBuf,
}

impl EditLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&mut self, event: &EditEvent) -> anyhow::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    pub fn read_all(path: &std::path::Path) -> anyhow::Result<Vec<EditEvent>> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut edits = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                edits.push(serde_json::from_str(&line)?);
            }
        }
        Ok(edits)
    }

    pub fn count(&self) -> anyhow::Result<u64> {
        if !self.path.exists() {
            return Ok(0);
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count() as u64)
    }
}
```

- [ ] **Step 7: Write failing test for checkpoints**

```rust
// tests/integration/snapshot_test.rs (append)
use vibetracer::snapshot::checkpoint::CheckpointManager;
use std::collections::HashMap;

#[test]
fn test_checkpoint_save_and_load() {
    let dir = TempDir::new().unwrap();
    let mgr = CheckpointManager::new(dir.path().to_path_buf());

    let mut state = HashMap::new();
    state.insert("src/main.rs".to_string(), "hash_abc".to_string());
    state.insert("src/lib.rs".to_string(), "hash_def".to_string());

    let id = mgr.save(state.clone()).unwrap();
    assert_eq!(id, 1);

    let loaded = mgr.load(id).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded["src/main.rs"], "hash_abc");

    // Second checkpoint
    let id2 = mgr.save(state).unwrap();
    assert_eq!(id2, 2);

    let all = mgr.list().unwrap();
    assert_eq!(all.len(), 2);
}
```

- [ ] **Step 8: Implement CheckpointManager**

```rust
// src/snapshot/checkpoint.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: u32,
    pub ts: i64,
    pub files: HashMap<String, String>,
}

pub struct CheckpointManager {
    dir: PathBuf,
}

impl CheckpointManager {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn save(&self, files: HashMap<String, String>) -> anyhow::Result<u32> {
        std::fs::create_dir_all(&self.dir)?;
        let id = self.next_id()?;
        let checkpoint = Checkpoint {
            id,
            ts: chrono::Utc::now().timestamp(),
            files,
        };
        let path = self.dir.join(format!("{id:03}.json"));
        let content = serde_json::to_string_pretty(&checkpoint)?;
        std::fs::write(path, content)?;
        Ok(id)
    }

    pub fn load(&self, id: u32) -> anyhow::Result<HashMap<String, String>> {
        let path = self.dir.join(format!("{id:03}.json"));
        let content = std::fs::read_to_string(path)?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)?;
        Ok(checkpoint.files)
    }

    pub fn list(&self) -> anyhow::Result<Vec<u32>> {
        let mut ids = Vec::new();
        if !self.dir.exists() {
            return Ok(ids);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if let Some(stem) = name.strip_suffix(".json") {
                    if let Ok(id) = stem.parse::<u32>() {
                        ids.push(id);
                    }
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn next_id(&self) -> anyhow::Result<u32> {
        let ids = self.list()?;
        Ok(ids.last().copied().unwrap_or(0) + 1)
    }
}
```

- [ ] **Step 9: Wire up snapshot/mod.rs**

```rust
// src/snapshot/mod.rs
pub mod checkpoint;
pub mod edit_log;
pub mod store;
```

- [ ] **Step 10: Run all snapshot tests**

Run: `cargo test snapshot_test`
Expected: ALL PASS

- [ ] **Step 11: Commit**

```bash
git add src/snapshot/ tests/
git commit -m "feat: snapshot engine with content-addressed store, edit log, and checkpoints"
```

---

### Task 5: Session Management

**Files:**
- Create: `src/session.rs`
- Create: `tests/integration/session_test.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/session_test.rs
use tempfile::TempDir;
use vibetracer::session::{Session, SessionManager};

#[test]
fn test_session_id_format() {
    let id = Session::generate_id();
    // Format: YYYYMMDD-HHMMSS-xxxx
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].len(), 8); // date
    assert_eq!(parts[1].len(), 6); // time
    assert_eq!(parts[2].len(), 4); // random suffix
}

#[test]
fn test_session_create_and_list() {
    let dir = TempDir::new().unwrap();
    let mgr = SessionManager::new(dir.path().join("sessions"));

    let s1 = mgr.create().unwrap();
    assert!(s1.dir.exists());

    let s2 = mgr.create().unwrap();
    let sessions = mgr.list().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_session_meta_persists() {
    let dir = TempDir::new().unwrap();
    let mgr = SessionManager::new(dir.path().join("sessions"));

    let session = mgr.create().unwrap();
    let meta = mgr.load_meta(&session.id).unwrap();
    assert_eq!(meta.id, session.id);
    assert!(!meta.project_path.is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test session_test`
Expected: FAIL

- [ ] **Step 3: Implement Session**

```rust
// src/session.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub project_path: String,
    pub started_at: i64,
    pub mode: SessionMode,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Enriched,
    Passive,
}

pub struct Session {
    pub id: String,
    pub dir: PathBuf,
}

impl Session {
    pub fn generate_id() -> String {
        let now = chrono::Utc::now();
        let mut rng = rand::rng();
        let suffix: String = (0..4)
            .map(|_| format!("{:x}", rng.random::<u8>() % 16))
            .collect();
        format!("{}-{suffix}", now.format("%Y%m%d-%H%M%S"))
    }
}

pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    pub fn create(&self) -> anyhow::Result<Session> {
        let id = Session::generate_id();
        let dir = self.sessions_dir.join(&id);
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join("snapshots"))?;
        std::fs::create_dir_all(dir.join("checkpoints"))?;

        let meta = SessionMeta {
            id: id.clone(),
            project_path: std::env::current_dir()?.to_string_lossy().into_owned(),
            started_at: chrono::Utc::now().timestamp(),
            mode: SessionMode::Passive,
        };
        let meta_path = dir.join("meta.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

        Ok(Session { id, dir })
    }

    pub fn list(&self) -> anyhow::Result<Vec<SessionMeta>> {
        let mut sessions = Vec::new();
        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let meta_path = entry.path().join("meta.json");
            if meta_path.exists() {
                let content = std::fs::read_to_string(&meta_path)?;
                if let Ok(meta) = serde_json::from_str::<SessionMeta>(&content) {
                    sessions.push(meta);
                }
            }
        }
        sessions.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok(sessions)
    }

    pub fn load_meta(&self, id: &str) -> anyhow::Result<SessionMeta> {
        let meta_path = self.sessions_dir.join(id).join("meta.json");
        let content = std::fs::read_to_string(meta_path)?;
        Ok(serde_json::from_str(&content)?)
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test session_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/session.rs tests/integration/session_test.rs
git commit -m "feat: session management with create, list, and meta persistence"
```

---

### Task 6: Filesystem Watcher

**Files:**
- Create: `src/watcher/mod.rs`
- Create: `src/watcher/fs_watcher.rs`
- Create: `src/watcher/differ.rs`
- Create: `tests/integration/watcher_test.rs`

- [ ] **Step 1: Write failing test for differ**

```rust
// tests/integration/watcher_test.rs
use vibetracer::watcher::differ;

#[test]
fn test_compute_unified_diff() {
    let old = "line1\nline2\nline3\n";
    let new = "line1\nmodified\nline3\nnew_line\n";

    let result = differ::compute_diff(old, new, "test.rs");
    assert!(result.patch.contains("-line2"));
    assert!(result.patch.contains("+modified"));
    assert!(result.patch.contains("+new_line"));
    assert_eq!(result.lines_added, 2);
    assert_eq!(result.lines_removed, 1);
}

#[test]
fn test_no_diff_when_identical() {
    let content = "same\n";
    let result = differ::compute_diff(content, content, "test.rs");
    assert!(result.patch.is_empty());
    assert_eq!(result.lines_added, 0);
    assert_eq!(result.lines_removed, 0);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test watcher_test`
Expected: FAIL

- [ ] **Step 3: Implement differ**

```rust
// src/watcher/differ.rs
use similar::{ChangeTag, TextDiff};

pub struct DiffResult {
    pub patch: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

pub fn compute_diff(old: &str, new: &str, filename: &str) -> DiffResult {
    let diff = TextDiff::from_lines(old, new);
    let mut added = 0u32;
    let mut removed = 0u32;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }

    let patch = if added == 0 && removed == 0 {
        String::new()
    } else {
        diff.unified_diff()
            .header(&format!("a/{filename}"), &format!("b/{filename}"))
            .to_string()
    };

    DiffResult {
        patch,
        lines_added: added,
        lines_removed: removed,
    }
}
```

- [ ] **Step 4: Run differ tests**

Run: `cargo test watcher_test`
Expected: PASS

- [ ] **Step 5: Write failing test for fs_watcher**

```rust
// tests/integration/watcher_test.rs (append)
use tempfile::TempDir;
use std::sync::mpsc;
use std::time::Duration;
use vibetracer::watcher::fs_watcher::FsWatcher;

#[test]
fn test_watcher_detects_file_create() {
    let dir = TempDir::new().unwrap();
    let (tx, rx) = mpsc::channel();
    let mut watcher = FsWatcher::new(dir.path().to_path_buf(), tx, 50).unwrap();
    watcher.start().unwrap();

    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));
    assert!(event.is_ok());
    let path = event.unwrap();
    assert!(path.to_string_lossy().contains("test.txt"));

    watcher.stop();
}

#[test]
fn test_watcher_respects_ignore_patterns() {
    let dir = TempDir::new().unwrap();
    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();

    let (tx, rx) = mpsc::channel();
    let ignore = vec![".git".to_string()];
    let mut watcher = FsWatcher::with_ignore(dir.path().to_path_buf(), tx, 50, ignore).unwrap();
    watcher.start().unwrap();

    // Write to ignored dir
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main").unwrap();

    // Write to tracked file
    std::fs::write(dir.path().join("real.txt"), "tracked").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));
    assert!(event.is_ok());
    let path = event.unwrap();
    assert!(path.to_string_lossy().contains("real.txt"));

    watcher.stop();
}
```

- [ ] **Step 6: Implement FsWatcher**

```rust
// src/watcher/fs_watcher.rs
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub struct FsWatcher {
    root: PathBuf,
    tx: Sender<PathBuf>,
    debounce_ms: u64,
    ignore: Vec<String>,
    running: Arc<AtomicBool>,
    watcher: Option<RecommendedWatcher>,
}

impl FsWatcher {
    pub fn new(root: PathBuf, tx: Sender<PathBuf>, debounce_ms: u64) -> anyhow::Result<Self> {
        Ok(Self {
            root,
            tx,
            debounce_ms,
            ignore: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
            watcher: None,
        })
    }

    pub fn with_ignore(
        root: PathBuf,
        tx: Sender<PathBuf>,
        debounce_ms: u64,
        ignore: Vec<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            root,
            tx,
            debounce_ms,
            ignore,
            running: Arc::new(AtomicBool::new(false)),
            watcher: None,
        })
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        self.running.store(true, Ordering::SeqCst);
        let tx = self.tx.clone();
        let ignore = self.ignore.clone();
        let root = self.root.clone();

        let watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let relative = path.strip_prefix(&root).unwrap_or(&path);
                    let should_ignore = ignore.iter().any(|pattern| {
                        relative
                            .components()
                            .any(|c| c.as_os_str().to_string_lossy() == *pattern)
                    });
                    if !should_ignore {
                        let _ = tx.send(path);
                    }
                }
            }
        })?;

        self.watcher = Some(watcher);
        self.watcher
            .as_mut()
            .unwrap()
            .watch(&self.root, RecursiveMode::Recursive)?;

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.watcher = None;
    }
}
```

- [ ] **Step 7: Wire up watcher/mod.rs**

```rust
// src/watcher/mod.rs
pub mod differ;
pub mod fs_watcher;
```

- [ ] **Step 8: Run all watcher tests**

Run: `cargo test watcher_test`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/watcher/ tests/integration/watcher_test.rs
git commit -m "feat: filesystem watcher with debounce, ignore patterns, and diff computation"
```

---

## Phase 2: TUI Foundation

### Task 7: App State and Main Loop

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/input.rs`
- Create: `src/tui/layout.rs`

- [ ] **Step 1: Define App state**

```rust
// src/tui/mod.rs
pub mod input;
pub mod layout;
pub mod widgets;

use crate::event::EditEvent;
use crate::snapshot::checkpoint::CheckpointManager;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    Preview,
    Timeline,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarPanel {
    BlastRadius,
    Sentinels,
    Watchdog,
    Refactor,
    Equations,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Live,
    Paused,
    Playing { speed: u8 },
}

pub struct App {
    pub edits: Vec<EditEvent>,
    pub playhead: usize,
    pub playback: PlaybackState,
    pub focused_pane: Pane,
    pub sidebar_visible: bool,
    pub sidebar_panel: SidebarPanel,
    pub equation_lens: bool,
    pub schema_diff_mode: bool,
    pub solo_track: Option<String>,
    pub muted_tracks: Vec<String>,
    pub checkpoint_ids: Vec<u32>,
    pub session_start: i64,
    pub connected: bool,
    pub should_quit: bool,
    pub tracks: Vec<TrackInfo>,
}

pub struct TrackInfo {
    pub filename: String,
    pub edit_indices: Vec<usize>,
    pub stale: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            playhead: 0,
            playback: PlaybackState::Live,
            focused_pane: Pane::Timeline,
            sidebar_visible: false,
            sidebar_panel: SidebarPanel::BlastRadius,
            equation_lens: false,
            schema_diff_mode: false,
            solo_track: None,
            muted_tracks: Vec::new(),
            checkpoint_ids: Vec::new(),
            session_start: chrono::Utc::now().timestamp(),
            connected: false,
            should_quit: false,
            tracks: Vec::new(),
        }
    }

    pub fn push_edit(&mut self, edit: EditEvent) {
        let filename = edit.file.clone();
        self.edits.push(edit);
        let edit_idx = self.edits.len() - 1;

        // Update or create track
        if let Some(track) = self.tracks.iter_mut().find(|t| t.filename == filename) {
            track.edit_indices.push(edit_idx);
        } else {
            self.tracks.push(TrackInfo {
                filename,
                edit_indices: vec![edit_idx],
                stale: false,
            });
        }

        // In live mode, playhead follows latest edit
        if self.playback == PlaybackState::Live {
            self.playhead = self.edits.len().saturating_sub(1);
        }
    }

    pub fn current_edit(&self) -> Option<&EditEvent> {
        self.edits.get(self.playhead)
    }

    pub fn scrub_left(&mut self) {
        if self.playhead > 0 {
            self.playhead -= 1;
            self.playback = PlaybackState::Paused;
        }
    }

    pub fn scrub_right(&mut self) {
        if self.playhead < self.edits.len().saturating_sub(1) {
            self.playhead += 1;
        }
        if self.playhead == self.edits.len().saturating_sub(1) {
            self.playback = PlaybackState::Live;
        }
    }

    pub fn toggle_play(&mut self) {
        self.playback = match self.playback {
            PlaybackState::Live => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing { speed: 1 },
            PlaybackState::Playing { .. } => PlaybackState::Paused,
        };
    }

    pub fn set_speed(&mut self, speed: u8) {
        if matches!(self.playback, PlaybackState::Playing { .. }) {
            self.playback = PlaybackState::Playing { speed };
        }
    }
}
```

- [ ] **Step 2: Implement input handling**

```rust
// src/tui/input.rs
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::tui::{App, Pane, SidebarPanel};

pub enum Action {
    None,
    Quit,
    TogglePlay,
    ScrubLeft,
    ScrubRight,
    JumpPrevCheckpoint,
    JumpNextCheckpoint,
    SetSpeed(u8),
    Rewind,
    RewindFile,
    UndoRewind,
    CutRange,
    Checkpoint,
    SoloTrack,
    MuteTrack,
    GroupByIntent,
    ToggleEquationLens,
    ToggleBlastRadius,
    ToggleSentinels,
    ToggleSchemaMode,
    ToggleRefactorTracker,
    ToggleWatchdog,
    CycleFocus,
    Search,
    Help,
}

pub fn map_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char(' ') => Action::TogglePlay,
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => Action::JumpPrevCheckpoint,
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => Action::JumpNextCheckpoint,
        KeyCode::Left => Action::ScrubLeft,
        KeyCode::Right => Action::ScrubRight,
        KeyCode::Char(n @ '1'..='5') => Action::SetSpeed(n.to_digit(10).unwrap() as u8),
        KeyCode::Char('r') => Action::Rewind,
        KeyCode::Char('R') => Action::RewindFile,
        KeyCode::Char('u') => Action::UndoRewind,
        KeyCode::Char('x') => Action::CutRange,
        KeyCode::Char('c') => Action::Checkpoint,
        KeyCode::Char('s') => Action::SoloTrack,
        KeyCode::Char('m') => Action::MuteTrack,
        KeyCode::Char('g') => Action::GroupByIntent,
        KeyCode::Char('e') => Action::ToggleEquationLens,
        KeyCode::Char('b') => Action::ToggleBlastRadius,
        KeyCode::Char('i') => Action::ToggleSentinels,
        KeyCode::Char('d') => Action::ToggleSchemaMode,
        KeyCode::Char('f') => Action::ToggleRefactorTracker,
        KeyCode::Char('w') => Action::ToggleWatchdog,
        KeyCode::Tab => Action::CycleFocus,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('?') => Action::Help,
        _ => Action::None,
    }
}

pub fn apply_action(app: &mut App, action: Action) {
    match action {
        Action::Quit => app.should_quit = true,
        Action::TogglePlay => app.toggle_play(),
        Action::ScrubLeft => app.scrub_left(),
        Action::ScrubRight => app.scrub_right(),
        Action::SetSpeed(s) => app.set_speed(s),
        Action::ToggleEquationLens => app.equation_lens = !app.equation_lens,
        Action::ToggleSchemaMode => app.schema_diff_mode = !app.schema_diff_mode,
        Action::ToggleBlastRadius => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::BlastRadius;
        }
        Action::ToggleSentinels => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Sentinels;
        }
        Action::ToggleWatchdog => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Watchdog;
        }
        Action::ToggleRefactorTracker => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Refactor;
        }
        Action::CycleFocus => {
            app.focused_pane = match app.focused_pane {
                Pane::Preview => Pane::Timeline,
                Pane::Timeline => {
                    if app.sidebar_visible {
                        Pane::Sidebar
                    } else {
                        Pane::Preview
                    }
                }
                Pane::Sidebar => Pane::Preview,
            };
        }
        Action::Checkpoint => {} // Handled by caller with snapshot engine access
        Action::Rewind | Action::RewindFile | Action::UndoRewind => {} // Handled by caller
        Action::CutRange | Action::SoloTrack | Action::MuteTrack
        | Action::GroupByIntent | Action::Search | Action::Help => {} // TODO: Phase 3+
        Action::None => {}
    }
}
```

- [ ] **Step 3: Implement layout computation**

```rust
// src/tui/layout.rs
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    pub status_bar: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub timeline: Rect,
    pub keybindings: Rect,
}

pub fn compute_layout(area: Rect, sidebar_visible: bool) -> AppLayout {
    // Top-level: status bar, main, timeline, keybindings
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),     // status bar
            Constraint::Min(10),       // main area (preview + sidebar)
            Constraint::Length(8),     // timeline
            Constraint::Length(1),     // keybindings bar
        ])
        .split(area);

    let (preview, sidebar) = if sidebar_visible {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(65),
                Constraint::Percentage(35),
            ])
            .split(vertical[1]);
        (horizontal[0], Some(horizontal[1]))
    } else {
        (vertical[1], None)
    };

    AppLayout {
        status_bar: vertical[0],
        main_area: vertical[1],
        preview,
        sidebar,
        timeline: vertical[2],
        keybindings: vertical[3],
    }
}
```

- [ ] **Step 4: Create empty widget module stubs**

Create `src/tui/widgets/mod.rs` with:
```rust
pub mod status_bar;
pub mod preview;
pub mod timeline;
pub mod sidebar;
pub mod help_overlay;
```

Create empty stub files for each widget module (`status_bar.rs`, `preview.rs`, `timeline.rs`, `sidebar.rs`, `help_overlay.rs`) with a placeholder comment.

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 6: Commit**

```bash
git add src/tui/
git commit -m "feat: TUI app state, input handling, and layout system"
```

---

### Task 8: Status Bar Widget

**Files:**
- Create: `src/tui/widgets/status_bar.rs`

- [ ] **Step 1: Implement status bar**

```rust
// src/tui/widgets/status_bar.rs
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use crate::tui::{App, PlaybackState};

pub struct StatusBar<'a> {
    app: &'a App,
}

impl<'a> StatusBar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let elapsed = chrono::Utc::now().timestamp() - self.app.session_start;
        let minutes = elapsed / 60;
        let seconds = elapsed % 60;

        let playback_indicator = match self.app.playback {
            PlaybackState::Live => Span::styled("live", Style::default().fg(Color::Rgb(90, 158, 111))),
            PlaybackState::Paused => Span::styled("paused", Style::default().fg(Color::Rgb(138, 143, 152))),
            PlaybackState::Playing { speed } => {
                Span::styled(format!("{speed}x"), Style::default().fg(Color::Rgb(188, 140, 255)))
            }
        };

        let connection = if self.app.connected {
            Span::styled("connected", Style::default().fg(Color::Rgb(90, 158, 111)))
        } else {
            Span::styled("watching", Style::default().fg(Color::Rgb(138, 143, 152)))
        };

        let left = vec![
            Span::styled("vibetracer", Style::default().fg(Color::Rgb(138, 143, 152))),
            Span::styled(" | ", Style::default().fg(Color::Rgb(42, 46, 55))),
            Span::styled(
                format!("{minutes}m {seconds:02}s"),
                Style::default().fg(Color::Rgb(160, 168, 183)),
            ),
            Span::styled(" | ", Style::default().fg(Color::Rgb(42, 46, 55))),
            Span::styled(
                format!("{} edits", self.app.edits.len()),
                Style::default().fg(Color::Rgb(160, 168, 183)),
            ),
            Span::styled(" | ", Style::default().fg(Color::Rgb(42, 46, 55))),
            Span::styled(
                format!("{} ckpts", self.app.checkpoint_ids.len()),
                Style::default().fg(Color::Rgb(160, 168, 183)),
            ),
        ];

        let right = vec![
            connection,
            Span::styled(" | ", Style::default().fg(Color::Rgb(42, 46, 55))),
            playback_indicator,
        ];

        // Render left-aligned
        let left_line = Line::from(left);
        buf.set_line(area.x, area.y, &left_line, area.width);

        // Render right-aligned
        let right_line = Line::from(right);
        let right_width = right_line.width() as u16;
        if area.width > right_width {
            buf.set_line(area.x + area.width - right_width, area.y, &right_line, right_width);
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/status_bar.rs
git commit -m "feat: status bar widget with session info and connection status"
```

---

### Task 9: Preview Pane Widget

**Files:**
- Create: `src/tui/widgets/preview.rs`

- [ ] **Step 1: Implement preview pane**

```rust
// src/tui/widgets/preview.rs
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use crate::event::EditEvent;

pub struct PreviewPane<'a> {
    edit: Option<&'a EditEvent>,
    equation_lens: bool,
}

impl<'a> PreviewPane<'a> {
    pub fn new(edit: Option<&'a EditEvent>, equation_lens: bool) -> Self {
        Self { edit, equation_lens }
    }
}

impl Widget for PreviewPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::NONE);

        let inner = block.inner(area);
        block.render(area, buf);

        let Some(edit) = self.edit else {
            let empty = Paragraph::new("no edits yet")
                .style(Style::default().fg(Color::Rgb(58, 62, 71)));
            empty.render(inner, buf);
            return;
        };

        // Header: edit number and filename
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("edit #{}", edit.id),
                    Style::default().fg(Color::Rgb(90, 101, 119)),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(
                    &edit.file,
                    Style::default().fg(Color::Rgb(122, 133, 153)),
                ),
            ]),
        ];

        // Intent line if available
        if let Some(ref intent) = edit.intent {
            lines.push(Line::from(vec![
                Span::styled("intent: ", Style::default().fg(Color::Rgb(58, 62, 71))),
                Span::styled(intent, Style::default().fg(Color::Rgb(138, 117, 96))),
            ]));
        }

        lines.push(Line::from(""));

        // Render diff lines with colors
        for line in edit.patch.lines() {
            let styled = if line.starts_with('+') && !line.starts_with("+++") {
                Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Rgb(90, 158, 111)),
                ))
            } else if line.starts_with('-') && !line.starts_with("---") {
                Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Rgb(158, 90, 90)),
                ))
            } else if line.starts_with("@@") {
                Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Rgb(90, 122, 158)),
                ))
            } else {
                Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Rgb(160, 168, 183)),
                ))
            };
            lines.push(styled);
        }

        // Stats footer
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("+{}", edit.lines_added),
                Style::default().fg(Color::Rgb(90, 158, 111)),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(
                format!("-{}", edit.lines_removed),
                Style::default().fg(Color::Rgb(158, 90, 90)),
            ),
        ]));

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        paragraph.render(inner, buf);
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/preview.rs
git commit -m "feat: preview pane widget with diff rendering"
```

---

### Task 10: Timeline Widget

**Files:**
- Create: `src/tui/widgets/timeline.rs`

- [ ] **Step 1: Implement timeline**

```rust
// src/tui/widgets/timeline.rs
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use crate::tui::App;

pub struct Timeline<'a> {
    app: &'a App,
}

impl<'a> Timeline<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl Widget for Timeline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 20 {
            return;
        }

        let total_edits = self.app.edits.len();
        let track_name_width: u16 = 14;
        let track_area_width = area.width.saturating_sub(track_name_width + 1);

        // Header
        let header = Line::from(vec![
            Span::styled(
                "tracks",
                Style::default().fg(Color::Rgb(58, 62, 71)),
            ),
        ]);
        buf.set_line(area.x, area.y, &header, area.width);

        // Render each track (up to area.height - 3 tracks, leaving room for playhead)
        let max_tracks = (area.height as usize).saturating_sub(3);
        let visible_tracks: Vec<_> = self.app.tracks.iter()
            .filter(|t| {
                if let Some(ref solo) = self.app.solo_track {
                    &t.filename == solo
                } else {
                    !self.app.muted_tracks.contains(&t.filename)
                }
            })
            .take(max_tracks)
            .collect();

        for (i, track) in visible_tracks.iter().enumerate() {
            let y = area.y + 1 + i as u16;
            if y >= area.y + area.height - 2 {
                break;
            }

            // Track name (truncated)
            let name = if track.filename.len() > track_name_width as usize - 1 {
                let short = track.filename
                    .rsplit('/')
                    .next()
                    .unwrap_or(&track.filename);
                if short.len() > track_name_width as usize - 1 {
                    &short[..track_name_width as usize - 1]
                } else {
                    short
                }
            } else {
                &track.filename
            };

            let name_color = if track.stale {
                Color::Rgb(158, 90, 90)
            } else {
                Color::Rgb(122, 133, 153)
            };
            let name_span = Span::styled(name, Style::default().fg(name_color));
            buf.set_line(area.x, y, &Line::from(name_span), track_name_width);

            // Track bar
            if total_edits > 0 {
                let bar_x = area.x + track_name_width;
                for col in 0..track_area_width {
                    let edit_idx = (col as usize * total_edits) / track_area_width as usize;
                    let has_edit = track.edit_indices.iter().any(|&ei| {
                        let mapped = (ei * track_area_width as usize) / total_edits;
                        mapped == col as usize
                    });
                    let ch = if has_edit { "█" } else { "░" };
                    let color = if has_edit {
                        Color::Rgb(90, 101, 119)
                    } else {
                        Color::Rgb(26, 29, 34)
                    };
                    buf.set_string(
                        bar_x + col,
                        y,
                        ch,
                        Style::default().fg(color),
                    );
                }
            }

            // Stale marker
            if track.stale {
                let stale_x = area.x + track_name_width + track_area_width + 1;
                if stale_x < area.x + area.width - 5 {
                    buf.set_string(
                        stale_x,
                        y,
                        "stale",
                        Style::default().fg(Color::Rgb(158, 90, 90)),
                    );
                }
            }
        }

        // Playhead line
        let playhead_y = area.y + area.height - 2;
        if total_edits > 0 && playhead_y < area.y + area.height {
            let playhead_col = if total_edits > 1 {
                (self.app.playhead * track_area_width as usize) / total_edits
            } else {
                0
            };
            let bar_x = area.x + track_name_width;
            for col in 0..track_area_width {
                let ch = if col == playhead_col as u16 { "|" } else { "-" };
                let color = if col == playhead_col as u16 {
                    Color::Rgb(138, 117, 96)
                } else {
                    Color::Rgb(42, 46, 55)
                };
                buf.set_string(bar_x + col, playhead_y, ch, Style::default().fg(color));
            }
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/timeline.rs
git commit -m "feat: timeline widget with per-file tracks and playhead"
```

---

### Task 11: Keybindings Bar and Help Overlay

**Files:**
- Create: `src/tui/widgets/help_overlay.rs`

- [ ] **Step 1: Implement keybindings bar rendering (inline in layout render)**

The keybindings bar is a single line rendered directly in the main render function. No separate widget needed — it's just a styled Line.

- [ ] **Step 2: Implement help overlay**

```rust
// src/tui/widgets/help_overlay.rs
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = 50.min(area.width.saturating_sub(4));
        let height = 24.min(area.height.saturating_sub(4));
        let x = area.x + (area.width - width) / 2;
        let y = area.y + (area.height - height) / 2;
        let overlay = Rect::new(x, y, width, height);

        Clear.render(overlay, buf);

        let bindings = vec![
            ("Space", "play / pause"),
            ("left/right", "scrub edit by edit"),
            ("Shift+left/right", "jump checkpoints"),
            ("1-5", "playback speed"),
            ("r", "rewind to playhead"),
            ("R", "rewind focused file"),
            ("u", "undo last rewind"),
            ("x", "cut range"),
            ("c", "checkpoint"),
            ("s", "solo track"),
            ("m", "mute track"),
            ("g", "group by intent"),
            ("e", "equation lens"),
            ("b", "blast radius"),
            ("i", "sentinels"),
            ("d", "schema diff"),
            ("f", "refactor tracker"),
            ("w", "watchdog"),
            ("Tab", "cycle panes"),
            ("/", "search"),
            ("q", "quit"),
        ];

        let lines: Vec<Line> = bindings
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(
                        format!("{key:>18}"),
                        Style::default().fg(Color::Rgb(90, 101, 119)),
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(160, 168, 183))),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(30, 34, 42)))
            .title(" keybindings ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 17, 21)));

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(overlay, buf);
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/help_overlay.rs
git commit -m "feat: help overlay widget"
```

---

### Task 12: Wire Up the Main Render Loop

**Files:**
- Modify: `src/main.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add run_tui function to tui/mod.rs**

Add a `run_tui` function that initializes the terminal, runs the event loop (polling crossterm events + receiving bus events), renders widgets, and cleans up on exit. This ties together all widgets, the app state, the watcher, and the snapshot engine.

The main loop:
1. Poll crossterm events (100ms timeout)
2. Check mpsc channel for watcher events
3. On watcher event: compute diff, create EditEvent, store snapshot, append to log, push to app state
4. On key event: map to Action, apply
5. Render: status bar, preview, timeline, keybindings, sidebar (if visible), help (if active)

- [ ] **Step 2: Wire main.rs CLI to run_tui**

Update `main.rs` so the default command (no subcommand) calls `run_tui` with the project path. `init` writes default config. `sessions` lists sessions. `replay` loads a session and opens in playback mode.

- [ ] **Step 3: Update lib.rs exports**

```rust
pub mod config;
pub mod event;
pub mod session;
pub mod snapshot;
pub mod watcher;
pub mod tui;
```

- [ ] **Step 4: Manual smoke test**

Run: `cargo run`
Expected: TUI opens, shows "vibetracer" status bar, empty timeline, "no edits yet" in preview. Press `q` to quit cleanly.

- [ ] **Step 5: Manual watcher test**

In one terminal: `cargo run -- /tmp/test-project`
In another: `echo "hello" > /tmp/test-project/test.txt`
Expected: Edit appears on timeline, diff shows in preview pane.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/tui/ src/lib.rs
git commit -m "feat: wire up main render loop with watcher integration"
```

---

## Phase 3: Rewind and Checkpoints

### Task 13: Rewind System

**Files:**
- Create: `src/rewind/mod.rs`
- Create: `tests/integration/rewind_test.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/rewind_test.rs
use tempfile::TempDir;
use vibetracer::rewind::RewindEngine;
use vibetracer::snapshot::store::SnapshotStore;
use vibetracer::snapshot::checkpoint::CheckpointManager;
use std::collections::HashMap;

#[test]
fn test_rewind_restores_file() {
    let project = TempDir::new().unwrap();
    let storage = TempDir::new().unwrap();

    // Create original file
    let file_path = project.path().join("main.rs");
    std::fs::write(&file_path, "original content").unwrap();

    // Store snapshot of original
    let store = SnapshotStore::new(storage.path().join("snapshots"));
    let hash = store.store(b"original content").unwrap();

    // Modify file (simulating AI edit)
    std::fs::write(&file_path, "modified content").unwrap();

    // Rewind
    let engine = RewindEngine::new(
        project.path().to_path_buf(),
        store,
        CheckpointManager::new(storage.path().join("checkpoints")),
    );
    engine.rewind_file("main.rs", &hash).unwrap();

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "original content");
}

#[test]
fn test_rewind_creates_pre_rewind_checkpoint() {
    let project = TempDir::new().unwrap();
    let storage = TempDir::new().unwrap();

    let file_path = project.path().join("main.rs");
    std::fs::write(&file_path, "before rewind").unwrap();

    let store = SnapshotStore::new(storage.path().join("snapshots"));
    let original_hash = store.store(b"original").unwrap();
    let ckpt_mgr = CheckpointManager::new(storage.path().join("checkpoints"));

    let engine = RewindEngine::new(
        project.path().to_path_buf(),
        store,
        ckpt_mgr,
    );

    let mut file_states = HashMap::new();
    file_states.insert("main.rs".to_string(), "current_hash".to_string());

    let pre_rewind_ckpt = engine.rewind_all(&file_states, &original_hash).unwrap();
    assert!(pre_rewind_ckpt > 0); // A checkpoint was created before rewind
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test rewind_test`
Expected: FAIL

- [ ] **Step 3: Implement RewindEngine**

```rust
// src/rewind/mod.rs
use crate::snapshot::checkpoint::CheckpointManager;
use crate::snapshot::store::SnapshotStore;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct RewindEngine {
    project_root: PathBuf,
    store: SnapshotStore,
    checkpoint_mgr: CheckpointManager,
}

impl RewindEngine {
    pub fn new(
        project_root: PathBuf,
        store: SnapshotStore,
        checkpoint_mgr: CheckpointManager,
    ) -> Self {
        Self {
            project_root,
            store,
            checkpoint_mgr,
        }
    }

    /// Rewind a single file to a specific snapshot.
    pub fn rewind_file(&self, relative_path: &str, snapshot_hash: &str) -> anyhow::Result<()> {
        let content = self.store.retrieve(snapshot_hash)?;
        let full_path = self.project_root.join(relative_path);
        std::fs::write(full_path, content)?;
        Ok(())
    }

    /// Rewind all files to their state at a given point.
    /// Creates a pre-rewind checkpoint first. Returns the checkpoint ID.
    pub fn rewind_all(
        &self,
        current_file_states: &HashMap<String, String>,
        _target_hash: &str,
    ) -> anyhow::Result<u32> {
        // Create pre-rewind checkpoint with current state
        let ckpt_id = self.checkpoint_mgr.save(current_file_states.clone())?;
        Ok(ckpt_id)
    }

    /// Rewind all tracked files to a checkpoint.
    pub fn rewind_to_checkpoint(&self, checkpoint_id: u32) -> anyhow::Result<()> {
        let files = self.checkpoint_mgr.load(checkpoint_id)?;
        for (relative_path, hash) in &files {
            self.rewind_file(relative_path, hash)?;
        }
        Ok(())
    }

    /// Undo the last rewind by restoring from the pre-rewind checkpoint.
    pub fn undo_rewind(&self, pre_rewind_checkpoint_id: u32) -> anyhow::Result<()> {
        self.rewind_to_checkpoint(pre_rewind_checkpoint_id)
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test rewind_test`
Expected: PASS

- [ ] **Step 5: Update lib.rs exports, commit**

```bash
git add src/rewind/ tests/integration/rewind_test.rs src/lib.rs
git commit -m "feat: rewind engine with pre-rewind checkpoints and undo"
```

---

## Phase 4: Hook Bridge

### Task 14: Claude Code Hook Integration

**Files:**
- Create: `src/hook/mod.rs`
- Create: `src/hook/bridge.rs`
- Create: `src/hook/registration.rs`
- Create: `tests/integration/hook_test.rs`

- [ ] **Step 1: Write failing tests for registration**

```rust
// tests/integration/hook_test.rs
use tempfile::TempDir;
use vibetracer::hook::registration;

#[test]
fn test_register_hook_creates_settings() {
    let project = TempDir::new().unwrap();
    let claude_dir = project.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    let socket_path = "/tmp/vibetracer-test.sock";
    registration::register_hook(&claude_dir, socket_path).unwrap();

    let settings_path = claude_dir.join("settings.local.json");
    assert!(settings_path.exists());

    let content = std::fs::read_to_string(&settings_path).unwrap();
    assert!(content.contains("PostToolUse"));
    assert!(content.contains(socket_path));
}

#[test]
fn test_unregister_hook_removes_entry() {
    let project = TempDir::new().unwrap();
    let claude_dir = project.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    let socket_path = "/tmp/vibetracer-test.sock";
    registration::register_hook(&claude_dir, socket_path).unwrap();
    registration::unregister_hook(&claude_dir).unwrap();

    let settings_path = claude_dir.join("settings.local.json");
    let content = std::fs::read_to_string(&settings_path).unwrap();
    assert!(!content.contains("vibetracer"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test hook_test`
Expected: FAIL

- [ ] **Step 3: Implement hook registration**

```rust
// src/hook/registration.rs
use std::path::Path;

pub fn register_hook(claude_dir: &Path, socket_path: &str) -> anyhow::Result<()> {
    let settings_path = claude_dir.join("settings.local.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let hook_entry = serde_json::json!({
        "matcher": "PostToolUse",
        "hooks": [{
            "type": "command",
            "command": format!(
                "echo '$TOOL_NAME $TOOL_INPUT' | nc -U {socket_path}"
            ),
            "description": "vibetracer edit tracking"
        }]
    });

    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert(serde_json::json!([]));

    if let Some(arr) = hooks.as_array_mut() {
        // Remove existing vibetracer hook if any
        arr.retain(|h| {
            h.get("hooks")
                .and_then(|h| h.as_array())
                .map(|hooks| {
                    !hooks.iter().any(|hook| {
                        hook.get("description")
                            .and_then(|d| d.as_str())
                            .map(|d| d.contains("vibetracer"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(true)
        });
        arr.push(hook_entry);
    }

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

pub fn unregister_hook(claude_dir: &Path) -> anyhow::Result<()> {
    let settings_path = claude_dir.join("settings.local.json");
    if !settings_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_array_mut()) {
        hooks.retain(|h| {
            h.get("hooks")
                .and_then(|h| h.as_array())
                .map(|hooks| {
                    !hooks.iter().any(|hook| {
                        hook.get("description")
                            .and_then(|d| d.as_str())
                            .map(|d| d.contains("vibetracer"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(true)
        });
    }

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}
```

- [ ] **Step 4: Implement hook bridge (Unix socket listener)**

```rust
// src/hook/bridge.rs
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use crate::event::BusEvent;

pub struct HookBridge {
    socket_path: PathBuf,
    tx: Sender<BusEvent>,
}

impl HookBridge {
    pub fn new(socket_path: PathBuf, tx: Sender<BusEvent>) -> Self {
        Self { socket_path, tx }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Start listening for hook payloads. Runs in a background thread.
    pub fn start(self) -> anyhow::Result<std::thread::JoinHandle<()>> {
        // Remove stale socket
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    let Ok(line) = line else { continue };
                    if let Some(enrichment) = Self::parse_payload(&line) {
                        let _ = self.tx.send(enrichment);
                    }
                }
            }
        });

        Ok(handle)
    }

    fn parse_payload(line: &str) -> Option<BusEvent> {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return None;
        }
        let tool = parts[0].to_string();
        // Extract file path from tool input JSON (best-effort)
        let input = parts[1];
        let file = serde_json::from_str::<serde_json::Value>(input)
            .ok()
            .and_then(|v| v.get("file_path").and_then(|f| f.as_str()).map(String::from))
            .unwrap_or_default();

        Some(BusEvent::HookEnrichment {
            file,
            tool,
            intent: None, // Intent requires conversation context, future enhancement
        })
    }
}

impl Drop for HookBridge {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
```

- [ ] **Step 5: Wire up hook/mod.rs**

```rust
// src/hook/mod.rs
pub mod bridge;
pub mod registration;
```

- [ ] **Step 6: Run all hook tests**

Run: `cargo test hook_test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/hook/ tests/integration/hook_test.rs src/lib.rs
git commit -m "feat: Claude Code hook bridge with registration and Unix socket listener"
```

---

## Phase 5: Analysis Features

### Task 15: Constants Watchdog

**Files:**
- Create: `src/analysis/mod.rs`
- Create: `src/analysis/watchdog.rs`
- Create: `tests/integration/watchdog_test.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/watchdog_test.rs
use vibetracer::analysis::watchdog::Watchdog;
use vibetracer::config::WatchdogConstant;

#[test]
fn test_watchdog_detects_constant_change() {
    let rules = vec![WatchdogConstant {
        file: "**/*.py".into(),
        pattern: r"EARTH_RADIUS_KM\s*=\s*([\d.]+)".into(),
        expected: "6371.0".into(),
        severity: "critical".into(),
    }];
    let watchdog = Watchdog::new(rules);

    let old = "EARTH_RADIUS_KM = 6371.0";
    let new = "EARTH_RADIUS_KM = 6400.0";
    let alerts = watchdog.check("constants.py", old, new);

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].expected, "6371.0");
    assert_eq!(alerts[0].actual, "6400.0");
    assert_eq!(alerts[0].severity, "critical");
}

#[test]
fn test_watchdog_no_alert_when_unchanged() {
    let rules = vec![WatchdogConstant {
        file: "**/*.py".into(),
        pattern: r"SPEED\s*=\s*([\d.]+)".into(),
        expected: "299792.458".into(),
        severity: "critical".into(),
    }];
    let watchdog = Watchdog::new(rules);

    let old = "SPEED = 299792.458";
    let new = "SPEED = 299792.458\n# added comment";
    let alerts = watchdog.check("physics.py", old, new);

    assert!(alerts.is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test watchdog_test`
Expected: FAIL

- [ ] **Step 3: Implement Watchdog**

```rust
// src/analysis/watchdog.rs
use crate::config::WatchdogConstant;
use regex::Regex;

pub struct WatchdogAlert {
    pub constant_pattern: String,
    pub expected: String,
    pub actual: String,
    pub severity: String,
    pub file: String,
}

pub struct Watchdog {
    rules: Vec<WatchdogConstant>,
}

impl Watchdog {
    pub fn new(rules: Vec<WatchdogConstant>) -> Self {
        Self { rules }
    }

    pub fn check(&self, filename: &str, old_content: &str, new_content: &str) -> Vec<WatchdogAlert> {
        let mut alerts = Vec::new();

        for rule in &self.rules {
            let file_pattern = glob::Pattern::new(&rule.file).ok();
            let matches_file = file_pattern
                .as_ref()
                .map(|p| p.matches(filename))
                .unwrap_or(false);

            if !matches_file {
                continue;
            }

            let Ok(re) = Regex::new(&rule.pattern) else {
                continue;
            };

            let old_val = re.captures(old_content).and_then(|c| c.get(1)).map(|m| m.as_str());
            let new_val = re.captures(new_content).and_then(|c| c.get(1)).map(|m| m.as_str());

            if let (Some(old_v), Some(new_v)) = (old_val, new_val) {
                if old_v != new_v && new_v != rule.expected {
                    alerts.push(WatchdogAlert {
                        constant_pattern: rule.pattern.clone(),
                        expected: rule.expected.clone(),
                        actual: new_v.to_string(),
                        severity: rule.severity.clone(),
                        file: filename.to_string(),
                    });
                }
            }
        }

        alerts
    }
}
```

- [ ] **Step 4: Create analysis/mod.rs**

```rust
// src/analysis/mod.rs
pub mod watchdog;
pub mod blast_radius;
pub mod sentinels;
pub mod refactor_tracker;
pub mod schema_diff;
pub mod imports;
```

- [ ] **Step 5: Create stub files for other analysis modules**

Create empty `blast_radius.rs`, `sentinels.rs`, `refactor_tracker.rs`, `schema_diff.rs`, `imports.rs` with placeholder comments.

- [ ] **Step 6: Run tests**

Run: `cargo test watchdog_test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/analysis/ tests/integration/watchdog_test.rs src/lib.rs
git commit -m "feat: constants watchdog with regex-based change detection"
```

---

### Task 16: Invariant Sentinels

**Files:**
- Create: `src/analysis/sentinels.rs`
- Create: `tests/integration/sentinel_test.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/sentinel_test.rs
use tempfile::TempDir;
use vibetracer::analysis::sentinels::{SentinelEngine, SentinelViolation};
use vibetracer::config::{SentinelRule, PatternSpec};

#[test]
fn test_grep_match_sentinel_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("config.py"), "N_FEATURES = 4").unwrap();
    std::fs::write(dir.path().join("model.py"), "input_size = 4").unwrap();

    let rule = SentinelRule {
        description: "feature count matches".into(),
        watch: vec!["*.py".into()],
        rule: "grep_match".into(),
        pattern_a: Some(PatternSpec {
            file: "config.py".into(),
            regex: r"N_FEATURES\s*=\s*(\d+)".into(),
        }),
        pattern_b: Some(PatternSpec {
            file: "model.py".into(),
            regex: r"input_size\s*=\s*(\d+)".into(),
        }),
        assert: Some("a == b".into()),
    };

    let engine = SentinelEngine::new(dir.path().to_path_buf());
    let violations = engine.evaluate("dims", &rule);
    assert!(violations.is_empty());
}

#[test]
fn test_grep_match_sentinel_fails() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("config.py"), "N_FEATURES = 4").unwrap();
    std::fs::write(dir.path().join("model.py"), "input_size = 3").unwrap();

    let rule = SentinelRule {
        description: "feature count matches".into(),
        watch: vec!["*.py".into()],
        rule: "grep_match".into(),
        pattern_a: Some(PatternSpec {
            file: "config.py".into(),
            regex: r"N_FEATURES\s*=\s*(\d+)".into(),
        }),
        pattern_b: Some(PatternSpec {
            file: "model.py".into(),
            regex: r"input_size\s*=\s*(\d+)".into(),
        }),
        assert: Some("a == b".into()),
    };

    let engine = SentinelEngine::new(dir.path().to_path_buf());
    let violations = engine.evaluate("dims", &rule);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].value_a, "4");
    assert_eq!(violations[0].value_b, "3");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test sentinel_test`
Expected: FAIL

- [ ] **Step 3: Implement SentinelEngine**

```rust
// src/analysis/sentinels.rs
use crate::config::{SentinelRule, PatternSpec};
use regex::Regex;
use std::path::PathBuf;

pub struct SentinelViolation {
    pub rule_name: String,
    pub description: String,
    pub value_a: String,
    pub value_b: String,
    pub assertion: String,
}

pub struct SentinelEngine {
    project_root: PathBuf,
}

impl SentinelEngine {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn evaluate(&self, rule_name: &str, rule: &SentinelRule) -> Vec<SentinelViolation> {
        match rule.rule.as_str() {
            "grep_match" => self.evaluate_grep_match(rule_name, rule),
            _ => Vec::new(),
        }
    }

    fn evaluate_grep_match(&self, rule_name: &str, rule: &SentinelRule) -> Vec<SentinelViolation> {
        let (Some(ref pa), Some(ref pb)) = (&rule.pattern_a, &rule.pattern_b) else {
            return Vec::new();
        };

        let val_a = self.extract_value(pa);
        let val_b = self.extract_value(pb);

        let (Some(a), Some(b)) = (val_a, val_b) else {
            return Vec::new();
        };

        let passes = match rule.assert.as_deref() {
            Some("a == b") => a == b,
            Some("a != b") => a != b,
            _ => a == b, // default to equality
        };

        if passes {
            Vec::new()
        } else {
            vec![SentinelViolation {
                rule_name: rule_name.to_string(),
                description: rule.description.clone(),
                value_a: a,
                value_b: b,
                assertion: rule.assert.clone().unwrap_or_else(|| "a == b".into()),
            }]
        }
    }

    fn extract_value(&self, spec: &PatternSpec) -> Option<String> {
        let path = self.project_root.join(&spec.file);
        let content = std::fs::read_to_string(path).ok()?;
        let re = Regex::new(&spec.regex).ok()?;
        let caps = re.captures(&content)?;
        caps.get(1).map(|m| m.as_str().to_string())
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test sentinel_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analysis/sentinels.rs tests/integration/sentinel_test.rs
git commit -m "feat: invariant sentinels with grep_match rule evaluation"
```

---

### Task 17: Blast Radius

**Files:**
- Create: `src/analysis/blast_radius.rs`
- Create: `src/analysis/imports.rs`

- [ ] **Step 1: Write failing tests**

```rust
// Add to tests/integration/ as blast_radius_test.rs
use vibetracer::analysis::blast_radius::BlastRadiusTracker;
use vibetracer::config::{BlastRadiusConfig, ManualDependency};
use std::collections::HashSet;

#[test]
fn test_manual_blast_radius() {
    let config = BlastRadiusConfig {
        auto_detect: false,
        manual: vec![ManualDependency {
            source: "config.py".into(),
            dependents: vec!["model.py".into(), "serving.py".into()],
        }],
    };

    let tracker = BlastRadiusTracker::new(config);
    let dependents = tracker.get_dependents("config.py");

    assert!(dependents.contains(&"model.py".to_string()));
    assert!(dependents.contains(&"serving.py".to_string()));
    assert_eq!(dependents.len(), 2);
}

#[test]
fn test_blast_radius_stale_detection() {
    let config = BlastRadiusConfig {
        auto_detect: false,
        manual: vec![ManualDependency {
            source: "config.py".into(),
            dependents: vec!["a.py".into(), "b.py".into(), "c.py".into()],
        }],
    };

    let tracker = BlastRadiusTracker::new(config);

    let edited: HashSet<String> = ["config.py", "a.py"].iter().map(|s| s.to_string()).collect();
    let status = tracker.check_staleness("config.py", &edited);

    assert_eq!(status.updated.len(), 1); // a.py
    assert_eq!(status.stale.len(), 2); // b.py, c.py
}
```

- [ ] **Step 2: Implement BlastRadiusTracker**

```rust
// src/analysis/blast_radius.rs
use crate::config::{BlastRadiusConfig, ManualDependency};
use std::collections::{HashMap, HashSet};

pub struct DependencyStatus {
    pub updated: Vec<String>,
    pub stale: Vec<String>,
    pub untouched: Vec<String>,
}

pub struct BlastRadiusTracker {
    config: BlastRadiusConfig,
    manual_deps: HashMap<String, Vec<String>>,
}

impl BlastRadiusTracker {
    pub fn new(config: BlastRadiusConfig) -> Self {
        let mut manual_deps = HashMap::new();
        for dep in &config.manual {
            manual_deps.insert(dep.source.clone(), dep.dependents.clone());
        }
        Self { config, manual_deps }
    }

    pub fn get_dependents(&self, source: &str) -> Vec<String> {
        // Check manual mappings first (exact match and glob)
        for (pattern, deps) in &self.manual_deps {
            if pattern == source {
                return deps.clone();
            }
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches(source) {
                    return deps.clone();
                }
            }
        }
        Vec::new()
    }

    pub fn check_staleness(&self, source: &str, edited_files: &HashSet<String>) -> DependencyStatus {
        let dependents = self.get_dependents(source);
        let mut updated = Vec::new();
        let mut stale = Vec::new();
        let mut untouched = Vec::new();

        for dep in dependents {
            if edited_files.contains(&dep) {
                updated.push(dep);
            } else {
                stale.push(dep.clone());
                untouched.push(dep);
            }
        }

        DependencyStatus {
            updated,
            stale,
            untouched,
        }
    }
}
```

- [ ] **Step 3: Create stub imports.rs**

```rust
// src/analysis/imports.rs
// tree-sitter import parsing is deferred to v1.1.
// v1 relies on manual blast_radius.manual config in config.toml.
// When auto_detect is enabled in a future version, this module will use
// tree-sitter grammars (Python, TypeScript, Rust, Go, Java) to parse
// import/use/require statements and build a dependency graph.

pub fn extract_imports(_content: &str, _language: &str) -> Vec<String> {
    Vec::new()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test blast_radius_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analysis/blast_radius.rs src/analysis/imports.rs tests/integration/
git commit -m "feat: blast radius tracker with manual dependency mappings"
```

---

### Task 18: Refactor Tracker

**Files:**
- Create: `src/analysis/refactor_tracker.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/refactor_tracker_test.rs
use tempfile::TempDir;
use vibetracer::analysis::refactor_tracker::RefactorTracker;

#[test]
fn test_detect_rename() {
    let tracker = RefactorTracker::new();
    let rename = tracker.detect_rename(
        "fn check_auth(req: &Request)",
        "fn validate_token(t: &Token)",
    );
    assert!(rename.is_some());
    let (old, new) = rename.unwrap();
    assert_eq!(old, "check_auth");
    assert_eq!(new, "validate_token");
}

#[test]
fn test_track_propagation() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.rs"), "use crate::check_auth;").unwrap();
    std::fs::write(dir.path().join("b.rs"), "check_auth(req)").unwrap();
    std::fs::write(dir.path().join("c.rs"), "validate_token(t)").unwrap();

    let mut tracker = RefactorTracker::new();
    tracker.track_rename(
        dir.path(),
        "check_auth",
        "validate_token",
    );

    let status = tracker.get_status("check_auth");
    assert!(status.is_some());
    let s = status.unwrap();
    assert_eq!(s.remaining_old_refs, 2); // a.rs, b.rs still reference old name
    assert_eq!(s.total_sites, 3);
}
```

- [ ] **Step 2: Implement RefactorTracker**

```rust
// src/analysis/refactor_tracker.rs
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct RenameStatus {
    pub old_name: String,
    pub new_name: String,
    pub remaining_old_refs: usize,
    pub updated_new_refs: usize,
    pub total_sites: usize,
    pub remaining_files: Vec<String>,
}

pub struct RefactorTracker {
    renames: HashMap<String, RenameInfo>,
}

struct RenameInfo {
    old_name: String,
    new_name: String,
    old_ref_files: Vec<String>,
    new_ref_files: Vec<String>,
}

impl RefactorTracker {
    pub fn new() -> Self {
        Self {
            renames: HashMap::new(),
        }
    }

    pub fn detect_rename(&self, old_line: &str, new_line: &str) -> Option<(String, String)> {
        // Look for function/method renames: fn old_name -> fn new_name
        let fn_re = Regex::new(r"\bfn\s+(\w+)").ok()?;
        let old_name = fn_re.captures(old_line)?.get(1)?.as_str().to_string();
        let new_name = fn_re.captures(new_line)?.get(1)?.as_str().to_string();

        if old_name != new_name {
            return Some((old_name, new_name));
        }
        None
    }

    pub fn track_rename(&mut self, project_root: &Path, old_name: &str, new_name: &str) {
        let mut old_refs = Vec::new();
        let mut new_refs = Vec::new();

        // Scan project for references
        Self::walk_files(project_root, &mut |path, content| {
            let relative = path.strip_prefix(project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if content.contains(old_name) {
                old_refs.push(relative.clone());
            }
            if content.contains(new_name) {
                new_refs.push(relative);
            }
        });

        self.renames.insert(old_name.to_string(), RenameInfo {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
            old_ref_files: old_refs,
            new_ref_files: new_refs,
        });
    }

    pub fn get_status(&self, old_name: &str) -> Option<RenameStatus> {
        let info = self.renames.get(old_name)?;
        let total = info.old_ref_files.len() + info.new_ref_files.len();
        // Deduplicate files that appear in both lists
        let mut all_files: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for f in &info.old_ref_files {
            all_files.insert(f);
        }
        for f in &info.new_ref_files {
            all_files.insert(f);
        }

        Some(RenameStatus {
            old_name: info.old_name.clone(),
            new_name: info.new_name.clone(),
            remaining_old_refs: info.old_ref_files.len(),
            updated_new_refs: info.new_ref_files.len(),
            total_sites: all_files.len(),
            remaining_files: info.old_ref_files.clone(),
        })
    }

    fn walk_files(dir: &Path, cb: &mut dyn FnMut(&Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }
                Self::walk_files(&path, cb);
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                cb(&path, &content);
            }
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test refactor_tracker_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analysis/refactor_tracker.rs tests/integration/
git commit -m "feat: refactor tracker with rename detection and propagation status"
```

---

### Task 19: Schema Diff

**Files:**
- Create: `src/analysis/schema_diff.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/integration/schema_diff_test.rs
use vibetracer::analysis::schema_diff::{parse_schema, diff_schemas, SchemaKind};

#[test]
fn test_parse_pydantic_model() {
    let source = r#"
class UserModel(BaseModel):
    name: str
    email: str
    age: int
"#;
    let schema = parse_schema(source, SchemaKind::Pydantic);
    assert!(schema.is_some());
    let s = schema.unwrap();
    assert_eq!(s.name, "UserModel");
    assert_eq!(s.fields.len(), 3);
    assert_eq!(s.fields[0].name, "name");
    assert_eq!(s.fields[0].field_type, "str");
}

#[test]
fn test_diff_schemas_detects_additions() {
    let old = r#"
class User(BaseModel):
    name: str
    email: str
"#;
    let new = r#"
class User(BaseModel):
    name: str
    email: str
    age: int
"#;
    let old_schema = parse_schema(old, SchemaKind::Pydantic).unwrap();
    let new_schema = parse_schema(new, SchemaKind::Pydantic).unwrap();
    let diff = diff_schemas(&old_schema, &new_schema);

    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.added[0].name, "age");
    assert!(diff.removed.is_empty());
}
```

- [ ] **Step 2: Implement schema diff**

```rust
// src/analysis/schema_diff.rs
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaKind {
    Pydantic,
    TypeScriptInterface,
    SqlTable,
}

#[derive(Debug, Clone)]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub name: String,
    pub kind: SchemaKind,
    pub fields: Vec<SchemaField>,
}

pub struct SchemaDiff {
    pub added: Vec<SchemaField>,
    pub removed: Vec<SchemaField>,
    pub type_changed: Vec<(SchemaField, SchemaField)>,
}

pub fn parse_schema(source: &str, kind: SchemaKind) -> Option<Schema> {
    match kind {
        SchemaKind::Pydantic => parse_pydantic(source),
        SchemaKind::TypeScriptInterface => parse_typescript_interface(source),
        SchemaKind::SqlTable => None, // TODO
    }
}

fn parse_pydantic(source: &str) -> Option<Schema> {
    let class_re = Regex::new(r"class\s+(\w+)\s*\(.*(?:BaseModel|Base).*\)\s*:").ok()?;
    let field_re = Regex::new(r"^\s+(\w+)\s*:\s*(\w[\w\[\], ]*)").ok()?;

    let caps = class_re.captures(source)?;
    let name = caps.get(1)?.as_str().to_string();

    let mut fields = Vec::new();
    let mut in_class = false;
    for line in source.lines() {
        if line.contains(&format!("class {name}")) {
            in_class = true;
            continue;
        }
        if in_class {
            if let Some(field_caps) = field_re.captures(line) {
                fields.push(SchemaField {
                    name: field_caps.get(1).unwrap().as_str().to_string(),
                    field_type: field_caps.get(2).unwrap().as_str().trim().to_string(),
                });
            } else if !line.trim().is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break; // End of class
            }
        }
    }

    Some(Schema {
        name,
        kind: SchemaKind::Pydantic,
        fields,
    })
}

fn parse_typescript_interface(source: &str) -> Option<Schema> {
    let iface_re = Regex::new(r"(?:interface|type)\s+(\w+)").ok()?;
    let field_re = Regex::new(r"^\s+(\w+)\??\s*:\s*(.+?)\s*;?\s*$").ok()?;

    let caps = iface_re.captures(source)?;
    let name = caps.get(1)?.as_str().to_string();

    let mut fields = Vec::new();
    let mut in_body = false;
    for line in source.lines() {
        if line.contains('{') && line.contains(&name) {
            in_body = true;
            continue;
        }
        if in_body {
            if line.contains('}') {
                break;
            }
            if let Some(f) = field_re.captures(line) {
                fields.push(SchemaField {
                    name: f.get(1).unwrap().as_str().to_string(),
                    field_type: f.get(2).unwrap().as_str().trim().to_string(),
                });
            }
        }
    }

    Some(Schema {
        name,
        kind: SchemaKind::TypeScriptInterface,
        fields,
    })
}

pub fn diff_schemas(old: &Schema, new: &Schema) -> SchemaDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut type_changed = Vec::new();

    for new_field in &new.fields {
        if let Some(old_field) = old.fields.iter().find(|f| f.name == new_field.name) {
            if old_field.field_type != new_field.field_type {
                type_changed.push((old_field.clone(), new_field.clone()));
            }
        } else {
            added.push(new_field.clone());
        }
    }

    for old_field in &old.fields {
        if !new.fields.iter().any(|f| f.name == old_field.name) {
            removed.push(old_field.clone());
        }
    }

    SchemaDiff {
        added,
        removed,
        type_changed,
    }
}

/// Auto-detect schema kind from file content.
pub fn detect_schema_kind(filename: &str, content: &str) -> Option<SchemaKind> {
    if filename.ends_with(".py") && content.contains("BaseModel") {
        Some(SchemaKind::Pydantic)
    } else if (filename.ends_with(".ts") || filename.ends_with(".tsx"))
        && (content.contains("interface ") || content.contains("type "))
    {
        Some(SchemaKind::TypeScriptInterface)
    } else {
        None
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test schema_diff_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analysis/schema_diff.rs tests/integration/
git commit -m "feat: schema diff with Pydantic and TypeScript interface parsing"
```

---

### Task 20: Sidebar Widgets

**Files:**
- Create: `src/tui/widgets/sidebar.rs`
- Create: `src/tui/widgets/blast_radius_panel.rs`
- Create: `src/tui/widgets/sentinel_panel.rs`
- Create: `src/tui/widgets/watchdog_panel.rs`
- Create: `src/tui/widgets/refactor_panel.rs`
- Create: `src/tui/widgets/equation_panel.rs`

- [ ] **Step 1: Implement sidebar container and all panel widgets**

Each panel is a ratatui Widget that takes a reference to its relevant analysis state and renders a focused view. The sidebar container selects which panel to show based on `app.sidebar_panel`.

The blast radius panel renders the dependency status list. The sentinel panel renders violations. The watchdog panel renders alerts. The refactor panel renders rename progress. The equation panel renders the equation index.

All follow the same muted color scheme established in the design (desaturated palette, no emojis, typographic hierarchy).

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/
git commit -m "feat: sidebar panel widgets for all analysis features"
```

---

## Phase 6: Equation Lens

### Task 21: Equation Detection and Rendering

**Files:**
- Create: `src/equation/mod.rs`
- Create: `src/equation/detect.rs`
- Create: `src/equation/render.rs`

- [ ] **Step 1: Write failing tests for detection**

```rust
// tests/integration/equation_test.rs
use vibetracer::equation::detect;

#[test]
fn test_detect_annotated_equation() {
    let source = r#"
// @eq: F = G * (m1 * m2) / r^2
let force = G * (m1 * m2) / r.powi(2);
"#;
    let equations = detect::extract_equations(source);
    assert_eq!(equations.len(), 1);
    assert_eq!(equations[0].latex, "F = G * (m1 * m2) / r^2");
    assert_eq!(equations[0].line, 2);
}

#[test]
fn test_detect_latex_delimiters() {
    let source = r#"
/// Compute gravitational acceleration
/// $a = \frac{F}{m}$
fn accel(force: f64, mass: f64) -> f64 {
"#;
    let equations = detect::extract_equations(source);
    assert_eq!(equations.len(), 1);
    assert!(equations[0].latex.contains("\\frac"));
}

#[test]
fn test_detect_no_equations() {
    let source = "fn main() { println!(\"hello\"); }";
    let equations = detect::extract_equations(source);
    assert!(equations.is_empty());
}
```

- [ ] **Step 2: Implement detection**

```rust
// src/equation/detect.rs
use regex::Regex;

pub struct DetectedEquation {
    pub line: usize,
    pub latex: String,
    pub raw_comment: String,
}

pub fn extract_equations(source: &str) -> Vec<DetectedEquation> {
    let mut equations = Vec::new();

    let annotated_re = Regex::new(r"(?://|#|///)\s*@eq:\s*(.+)$").unwrap();
    let inline_re = Regex::new(r"\$([^$]+)\$").unwrap();
    let display_re = Regex::new(r"\$\$([^$]+)\$\$").unwrap();

    for (i, line) in source.lines().enumerate() {
        let line_num = i + 1;

        // Check @eq: annotation
        if let Some(caps) = annotated_re.captures(line) {
            equations.push(DetectedEquation {
                line: line_num,
                latex: caps.get(1).unwrap().as_str().trim().to_string(),
                raw_comment: line.trim().to_string(),
            });
            continue;
        }

        // Check $$...$$ (display math)
        for caps in display_re.captures_iter(line) {
            equations.push(DetectedEquation {
                line: line_num,
                latex: caps.get(1).unwrap().as_str().trim().to_string(),
                raw_comment: line.trim().to_string(),
            });
        }

        // Check $...$ (inline math) — only in comments
        if line.trim_start().starts_with("//")
            || line.trim_start().starts_with('#')
            || line.trim_start().starts_with("///")
            || line.trim_start().starts_with("* ")
        {
            for caps in inline_re.captures_iter(line) {
                equations.push(DetectedEquation {
                    line: line_num,
                    latex: caps.get(1).unwrap().as_str().trim().to_string(),
                    raw_comment: line.trim().to_string(),
                });
            }
        }
    }

    equations
}
```

- [ ] **Step 3: Implement render backends**

```rust
// src/equation/render.rs
use std::process::Command;

pub enum RenderBackend {
    Tectonic,
    Unicode,
}

pub fn detect_backend() -> RenderBackend {
    if Command::new("tectonic").arg("--version").output().is_ok() {
        RenderBackend::Tectonic
    } else {
        RenderBackend::Unicode
    }
}

/// Render equation to Unicode approximation (always available).
pub fn render_unicode(latex: &str) -> String {
    // Basic substitutions for common math
    latex
        .replace("\\frac{", "")
        .replace("}{", "/")
        .replace("\\int", "\u{222B}")
        .replace("\\sum", "\u{2211}")
        .replace("\\sqrt", "\u{221A}")
        .replace("\\infty", "\u{221E}")
        .replace("\\pi", "\u{03C0}")
        .replace("\\alpha", "\u{03B1}")
        .replace("\\beta", "\u{03B2}")
        .replace("\\gamma", "\u{03B3}")
        .replace("\\delta", "\u{03B4}")
        .replace("\\Delta", "\u{0394}")
        .replace("^2", "\u{00B2}")
        .replace("^3", "\u{00B3}")
        .replace("_0", "\u{2080}")
        .replace("_1", "\u{2081}")
        .replace("_2", "\u{2082}")
        .replace("\\cdot", "\u{00B7}")
        .replace("\\times", "\u{00D7}")
        .replace("\\leq", "\u{2264}")
        .replace("\\geq", "\u{2265}")
        .replace("\\neq", "\u{2260}")
        .replace("\\approx", "\u{2248}")
        .replace('{', "")
        .replace('}', "")
        .replace('\\', "")
}

/// Render equation to PNG via tectonic (if available).
/// Returns the PNG bytes or None if rendering fails.
pub fn render_png(latex: &str) -> Option<Vec<u8>> {
    let tex_content = format!(
        r"\documentclass[preview,border=2pt]{{standalone}}
\usepackage{{amsmath}}
\begin{{document}}
$\displaystyle {}$
\end{{document}}",
        latex
    );

    let dir = tempfile::tempdir().ok()?;
    let tex_path = dir.path().join("eq.tex");
    std::fs::write(&tex_path, &tex_content).ok()?;

    let output = Command::new("tectonic")
        .arg(&tex_path)
        .arg("--outdir")
        .arg(dir.path())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // Convert PDF to PNG (requires additional tooling, fall back to unicode for now)
    // In full implementation, use pdf2svg or direct DVI rendering
    None
}
```

- [ ] **Step 4: Wire up equation/mod.rs**

```rust
// src/equation/mod.rs
pub mod detect;
pub mod render;
```

- [ ] **Step 5: Run tests**

Run: `cargo test equation_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/equation/ tests/integration/
git commit -m "feat: equation detection from comments and Unicode rendering"
```

---

## Phase 7: Distribution

### Task 22: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create CI workflow**

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

  test-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "ci: add test and lint workflow"
```

---

### Task 23: Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create release workflow**

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross (Linux ARM)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: cargo install cross

      - name: Build
        run: |
          if [ "${{ matrix.target }}" = "aarch64-unknown-linux-gnu" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Package
        run: |
          cd target/${{ matrix.target }}/release
          tar czf vibetracer-${{ matrix.target }}.tar.gz vibetracer
          mv vibetracer-${{ matrix.target }}.tar.gz ../../../

      - name: Upload
        uses: softprops/action-gh-release@v2
        with:
          files: vibetracer-${{ matrix.target }}.tar.gz

  publish-crate:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow with cross-platform builds"
```

---

### Task 24: Install Script and Homebrew

**Files:**
- Create: `scripts/install.sh`
- Create: `homebrew/vibetracer.rb`

- [ ] **Step 1: Create install script**

```bash
#!/usr/bin/env bash
# scripts/install.sh — curl-pipe-sh installer for vibetracer
set -euo pipefail

REPO="omeedtehrani/vibetracer"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os="apple-darwin" ;;
    Linux)  os="unknown-linux-gnu" ;;
    *)      echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac

  case "$arch" in
    x86_64)  arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)       echo "Unsupported arch: $arch" >&2; exit 1 ;;
  esac

  echo "${arch}-${os}"
}

main() {
  local platform version url tmp

  platform="$(detect_platform)"
  version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep tag_name | cut -d'"' -f4)"

  echo "Installing vibetracer ${version} for ${platform}..."

  url="https://github.com/${REPO}/releases/download/${version}/vibetracer-${platform}.tar.gz"
  tmp="$(mktemp -d)"

  curl -fsSL "$url" | tar xz -C "$tmp"
  mkdir -p "$INSTALL_DIR"
  mv "$tmp/vibetracer" "$INSTALL_DIR/vibetracer"
  chmod +x "$INSTALL_DIR/vibetracer"
  rm -rf "$tmp"

  echo "Installed to ${INSTALL_DIR}/vibetracer"

  if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

main
```

- [ ] **Step 2: Create homebrew formula template**

```ruby
# homebrew/vibetracer.rb
class Vibetracer < Formula
  desc "Real-time tracing, replaying, and rewinding of AI coding assistant edits"
  homepage "https://github.com/omeedtehrani/vibetracer"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/omeedtehrani/vibetracer/releases/download/v#{version}/vibetracer-aarch64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
    on_intel do
      url "https://github.com/omeedtehrani/vibetracer/releases/download/v#{version}/vibetracer-x86_64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/omeedtehrani/vibetracer/releases/download/v#{version}/vibetracer-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
    on_intel do
      url "https://github.com/omeedtehrani/vibetracer/releases/download/v#{version}/vibetracer-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  def install
    bin.install "vibetracer"
  end

  test do
    assert_match "vibetracer", shell_output("#{bin}/vibetracer --version")
  end
end
```

- [ ] **Step 3: Make install script executable and commit**

```bash
chmod +x scripts/install.sh
git add scripts/install.sh homebrew/vibetracer.rb
git commit -m "feat: install script and homebrew formula for distribution"
```

---

### Task 25: Final Integration Test

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run format check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Manual end-to-end smoke test**

```bash
cargo run -- init
cargo run -- /tmp/test-vibetracer
# In another terminal, make edits to files in /tmp/test-vibetracer
# Verify: edits appear on timeline, scrubbing works, checkpoint works
```

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final integration pass"
```

---

## Known Deferrals (intentionally omitted from v1 plan)

These spec items are deferred to a follow-up plan. They are not forgotten.

- **`--split` flag** — tmux pane auto-launch. Requires tmux CLI detection and pane splitting. Low complexity, high polish.
- **`.gitignore` prompting** — on first run, prompt user to add `.vibetracer/` to `.gitignore`. Simple but requires interactive terminal detection.
- **Session retention/cleanup** — 30-day TTL with automatic pruning. Needs a background cleanup pass or on-startup sweep.
- **`count_match` sentinel rule** — count items matching a pattern in file A, assert equals value in file B.
- **`schema_fields_match` sentinel rule** — structural comparison of schema definitions across files.
- **Rewind confirmation prompt** — interactive Y/N before writing snapshot files to disk. The pre-rewind checkpoint and `u` undo provide safety, but an explicit prompt is better UX.
- **tree-sitter import parsing** — auto-detect blast radius from import/use/require statements. v1 uses manual config only.
- **Full LaTeX-to-PNG rendering** — tectonic produces PDF, needs PDF-to-image conversion. v1 uses Unicode math fallback.
