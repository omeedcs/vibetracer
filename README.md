# vibetracer

Real-time tracing, replaying, and restoring of AI coding assistant edits. A background daemon records every change. A TUI lets you scrub through time, inspect diffs, and surgically restore files to any prior state. An MCP server exposes trace data back to AI assistants for self-correction. Cross-agent support for Claude Code, Cursor, and Codex CLI. Export sessions as Agent Trace JSON or git-ai compatible git notes.

```
        _ _          _
 __   _(_) |__   ___| |_ _ __ __ _  ___ ___ _ __
 \ \ / / | '_ \ / _ \ __| '__/ _` |/ __/ _ \ '__|
  \ V /| | |_) |  __/ |_| | | (_| | (_|  __/ |
   \_/ |_|_.__/ \___|\__|_|  \__,_|\___\___|_|
```

## The Problem

AI coding assistants edit fast. By the time something breaks, the relevant change is buried under dozens of others. You can't surgically undo a single AI command. You can't see which agent touched which file. You can't rewind one file without rewinding everything else.

vibetracer fixes this.

## How It Works

vibetracer runs a lightweight **background daemon** that watches your project directory. Every file change is captured as a diff, stored in a content-addressed snapshot store, and logged to an append-only JSONL edit journal. The daemon is invisible -- it records silently and costs near-zero overhead.

When you want to inspect, open the **TUI viewer**. It reads the edit log from disk and renders a multi-track timeline inspired by non-linear video editors. Each file is a track. Each edit is a clip. Scrub through time, inspect diffs, and restore files when you need to.

When Claude Code is detected, vibetracer registers a hook to capture **which agent** made each edit, **what command** triggered it, and **why**. Multiple Claude Code sessions are tracked independently with per-agent attribution.

```
vibetracer                         # start TUI (auto-starts daemon)
vibetracer daemon start            # start daemon without TUI
vibetracer daemon status           # check daemon state
```

## Features

### Daemon + Viewer Architecture

The daemon records in the background. The TUI connects as a read-only viewer. Close the TUI -- the daemon keeps recording. Reconnect later and pick up where you left off.

```bash
vibetracer daemon start ~/my-project   # start recording
# ... work for hours ...
vibetracer ~/my-project                # open TUI, see everything
```

### Per-File Playheads

Each file gets its own independent playhead. Rewind `model.py` to 3 edits ago while keeping `config.py` at its latest state. Detach a file from the global timeline, scrub it independently, then reattach when done.

### Command-Level Operation Grouping

Toggle between **edit view** (every individual file change) and **command view** (edits grouped by the AI command that caused them). See "Claude refactored auth middleware (touched 4 files)" as a single timeline entry. Restore an entire command atomically.

### Multi-Agent Tracking

Multiple AI agents editing the same project are tracked independently. Claude Code, Cursor, and Codex CLI are all supported. Each agent gets a distinct color on the timeline. When two agents edit the same file within 5 seconds, the timeline shows conflict indicators. Filter by agent. Restore everything one agent did in a time range.

### Restore System

"Rewind" scrubs the timeline visually. "Restore" writes files to disk. These are separate actions -- vibetracer is an observer by default.

- **Restore file**: Single file to any prior edit point
- **Undo restore**: Every restore is logged. Press `u` to reverse the last one.
- **Conflict detection**: Blast radius checks suggest restoring coupled files together

```bash
# Headless restore from CLI (no TUI needed)
vibetracer restore src/main.rs --edit-id 42
```

### Analysis Engines

**Blast Radius Detection** -- When a file is edited, see which dependent files need updating. Catches partial refactors where the AI updates 3 of 5 files and moves on.

**Invariant Sentinels** -- Define rules like "tensor input dimensions must match feature count." vibetracer alerts you instantly when an edit breaks an invariant.

**Constants Watchdog** -- Register values that should never change (physics constants, API endpoints). Get an alert if the AI modifies them.

### 19 Color Themes

Cycle themes at runtime with `t`. No restart needed.

**Dark:** dark (default), catppuccin-mocha, catppuccin-macchiato, gruvbox-dark, tokyo-night, tokyo-night-storm, dracula, nord, kanagawa, rose-pine, one-dark, solarized-dark, everforest-dark

**Light:** light, catppuccin-latte, gruvbox-light, solarized-light, rose-pine-dawn, everforest-light

```toml
[theme]
preset = "tokyo-night"
```

### Cross-Agent Import

Import sessions from multiple AI coding assistants:

- **Claude Code** -- JSONL session files with full tool metadata and intent context
- **Cursor** -- Agent Trace JSON format (`.agent-trace/` directory)
- **Codex CLI** -- Agent Trace compatible output

When Claude Code's `.claude/` directory exists, vibetracer auto-registers a `PostToolUse` hook to capture tool metadata and intent context. Works in passive mode (filesystem-only) when no AI tool is detected.

### Export to Ecosystem Formats

Export your temporal recordings to formats the provenance ecosystem consumes:

```bash
vibetracer export --format agent-trace <session-id>     # Agent Trace JSON
vibetracer export --format agent-trace <session-id> --output trace.json
vibetracer export --format git-notes <session-id>        # git-ai compatible git notes
```

Agent Trace JSON is the vendor-neutral format used by Cursor and the git-ai ecosystem. Git notes attach authorship logs directly to commits, compatible with `git-ai blame`.

### MCP Server (AI Self-Correction)

vibetracer includes an [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that exposes trace data back to AI coding assistants. This creates a feedback loop: the AI makes edits, vibetracer records them, and when something breaks, the AI can scrub through its own edit history to pinpoint which change introduced the regression -- like `git bisect` but at sub-commit granularity.

```bash
vibetracer mcp   # start stdio JSON-RPC server
```

Add to your MCP client configuration (e.g., `.claude.json`, Cursor settings, or any MCP-compatible tool):

```json
{
  "mcpServers": {
    "vibetracer": {
      "command": "vibetracer",
      "args": ["mcp"]
    }
  }
}
```

**Available tools:**

| Tool | Description |
|------|-------------|
| `list_sessions` | List recorded trace sessions with metadata and edit counts |
| `get_timeline` | Get the edit timeline for a session (paginated, filterable by file glob) |
| `get_frame` | Reconstruct the exact state of files at any point in the timeline |
| `diff_frames` | Unified diff between any two points in the timeline |
| `search_edits` | Find frames where a specific function/pattern was modified (regex) |
| `get_regression_window` | Get candidate frames for bisecting a regression (filter by file and range) |
| `subscribe_edits` | Subscribe to live edit notifications from an active recording session |

All list-returning tools support `offset`/`limit` pagination and stream JSONL lazily -- they handle arbitrarily large traces without loading everything into memory.

**Self-correction skill:** Copy `skills/vibetracer-review.md` to your AI assistant's skills directory. It orchestrates the full workflow: load the trace, identify scope, run tests, bisect the regression through the timeline, and fix surgically at the source rather than patching on top.

### Session Management

```bash
vibetracer sessions                # list past sessions
vibetracer replay <session-id>     # replay a past session in the TUI
vibetracer import                  # list importable Claude Code sessions
vibetracer import <session-id>     # import and replay a Claude Code session
```

## Install

**Cargo:**
```bash
cargo install vibetracer
```

**From source:**
```bash
git clone https://github.com/omeedcs/vibetracer.git
cd vibetracer
cargo install --path .
```

## Usage

```bash
# Watch current directory (auto-starts daemon)
vibetracer

# Watch a specific project
vibetracer ~/my-project

# Single-process mode (no daemon, for debugging)
vibetracer --no-daemon ~/my-project

# Daemon management
vibetracer daemon start [path]
vibetracer daemon stop [path]
vibetracer daemon status [path]

# Restore a file from CLI (headless)
vibetracer restore <file> --edit-id <N>

# Import a Claude Code session
vibetracer import [session-id]

# Replay a past vibetracer session
vibetracer replay <session-id>

# List past sessions
vibetracer sessions

# Export session as Agent Trace JSON
vibetracer export --format agent-trace <session-id>

# Export session as git-ai git notes
vibetracer export --format git-notes <session-id>

# Start MCP server for AI coding assistants
vibetracer mcp

# Initialize config with smart auto-detection
vibetracer init

# Write debug log for troubleshooting
vibetracer --debug ~/my-project
```

## Keybindings

| Key | Action |
|-----|--------|
| `Space` | play / pause |
| `Left / Right` | scrub through edits (global) |
| `Shift+Left / Right` | scrub per-file (detaches from global timeline) |
| `a` | reattach detached file to global playhead |
| `g` | toggle edit view / command view |
| `R` | restore file at playhead to disk |
| `u` | undo last restore |
| `c` | create checkpoint |
| `x` | toggle showing restore-generated edits |
| `s` | solo a track |
| `m` | mute a track |
| `b` | blast radius panel |
| `i` | sentinels panel |
| `w` | watchdog panel |
| `t` | cycle color theme |
| `1-9` | solo agent (command view) |
| `Tab` | cycle pane focus |
| `?` | help overlay |
| `q` | quit TUI (daemon keeps running) |
| `Q` | quit TUI and stop daemon |

## Configuration

Run `vibetracer init` to auto-detect constants, schemas, and dependencies in your project.

```toml
# .vibetracer/config.toml

[watch]
debounce_ms = 100
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer"]
auto_checkpoint_every = 25

[theme]
preset = "dark"

# Watchdog: alert when registered constants change
[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"

# Sentinels: alert when invariants break
[sentinels.feature_count]
description = "feature count must match model input size"
watch = "**/*.py"
rule = "grep_match"
pattern_a = { file = "config.py", regex = 'N_FEATURES\s*=\s*(\d+)' }
pattern_b = { file = "model.py", regex = 'input_size\s*=\s*(\d+)' }
assert = "a == b"

# Blast radius: declare file dependencies
[[blast_radius.manual]]
source = "**/config*.py"
dependents = ["**/model*.py", "**/serving*.py"]
```

## Architecture

```
vibetracer
  daemon/           Background recorder (watcher + snapshot store + edit log)
  recorder/          Shared recording logic (used by daemon and --no-daemon mode)
  snapshot/          Content-addressed file storage + append-only JSONL edit log
  checkpoint/        Full project state snapshots
  restore/           File restoration engine + conflict checker
  analysis/          Watchdog, sentinels, blast radius (run in TUI)
  tui/               Terminal UI (app state, event loop, playheads, widgets)
  hook/              Claude Code hook registration
  import/            Multi-agent session import (Claude Code, Cursor, Codex CLI)
  export/            Session export (Agent Trace JSON, git-ai git notes)
  mcp/               MCP server (JSON-RPC stdio, exposes trace data to AI assistants)
```

Data is stored in `.vibetracer/` within your project directory. Add it to `.gitignore`.

## Tech Stack

Rust, ratatui, crossterm, notify, similar, serde, serde_json, clap, sha2, regex, glob, chrono, libc

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
