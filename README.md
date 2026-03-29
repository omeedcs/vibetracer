# vibetracer

Real-time AI coding session observability. Trace, replay, and understand what your AI assistant is actually doing.

```
cargo install vibetracer
```

---

## What is vibetracer?

vibetracer is a terminal-based observability cockpit for AI coding sessions. It captures every file change, parses Claude Code's conversation logs, and presents everything in a dense htop-style TUI. You can scrub through time, inspect individual edits, see what Claude was thinking, track token costs, and surgically restore files. It works with Claude Code, Cursor, and Codex CLI out of the box.

## Quick Start

```bash
cargo install vibetracer
cd your-project
vibetracer          # starts watching + opens cockpit
vibetracer demo     # try the interactive demo
```

vibetracer auto-starts a background daemon, begins recording, and opens the TUI. Close the TUI and the daemon keeps recording. Reconnect later with `vibetracer` and pick up where you left off.

---

## Features

### Cockpit Dashboard

A dense htop-style dashboard showing everything at a glance. Toggle with `D`.

- Token and cost tracking with burn rate computed from Claude Code logs
- Edit velocity graph (edits per minute over the last 60 seconds)
- File heatmap showing the most-edited files ranked by edit count
- Agent status indicators (active/idle based on 10-second activity window)
- Operation progress tracking with per-operation edit counts
- Blast radius summary: stale, updated, and untouched dependent files
- Sentinel failure count with rule names and descriptions
- Watchdog status with per-constant alert details
- Cache hit rate percentage from Claude's token usage

### Vim-Modal Interface

Four modes, each with dedicated keybindings and a context-sensitive status bar:

- **Normal** -- default mode for navigation, scrubbing, and panel toggles
- **Timeline** (`t`) -- focused timeline manipulation with zoom, pan, and track selection
- **Inspect** (`i`) -- deep-dive into individual edits with diff/file/conversation views
- **Search** (`/`) -- composable filter syntax to narrow edits across the entire session

The mode indicator displays in the status bar. A command palette (`:` or `Ctrl+P`) provides fuzzy search across all actions with MRU ordering.

### Claude Code Integration

When Claude Code's `.claude/` directory is detected, vibetracer auto-registers a `PostToolUse` hook and begins parsing conversation logs in a background thread.

- User prompts and tool call trees parsed in real-time
- Token usage per turn with cost estimates (input, output, cache read)
- Cache hit rate tracking
- Conversation panel (`C` key) with navigable turns and tool calls
- Agent label and operation intent attached to every edit

### Timeline and Playback

```
+-- timeline -------------------------------------------------------+
| src/auth.rs      [====|==  |=====|=  ] ------>                     |
| src/config.py    [==  |    |=    |   ] ------>                     |
| src/model.py     [    |====|     |===] ------>                     |
|                        ^                                           |
|                     playhead                                       |
+--------------------------------------------------------------------+
```

- Per-file horizontal tracks with edit cells color-coded by agent
- Global playhead plus independent per-file playheads
- Detachable file playheads (scrub one file while others stay at global position)
- Solo and mute tracks to focus on specific files
- Timeline zoom (`+`/`-`) and pan (arrow keys in Timeline mode)
- Command view (`g`) groups edits by the AI operation that caused them
- Multi-agent color coding with per-agent solo filtering (`1`-`9`)
- Auto-follow (Live mode) with manual pause/play (`Space`)

### Investigation Tools

**Search and Filter** -- composable syntax with AND logic across predicates:

```
file:auth agent:claude kind:modify tool:Edit after:14:30 before:15:00 lines>20 op:refactor content:token
```

| Predicate | Description |
|-----------|-------------|
| `file:` | Match file path substring |
| `agent:` | Match agent ID or label |
| `kind:` | Filter by create, modify, or delete |
| `tool:` | Match tool name (Edit, Write, etc.) |
| `after:` | Edits after HH:MM offset or edit ID |
| `before:` | Edits before HH:MM offset or edit ID |
| `lines>` / `lines<` | Filter by total lines changed |
| `op:` | Match operation intent substring |
| `content:` | Grep through diff content |
| bare text | Fuzzy match across all fields |

**Blame View** (`B`) -- per-line agent and operation attribution overlaid on the file preview. Shows which agent wrote each line and what operation triggered it.

**Inline Annotations** (`A`) -- operation intent displayed alongside code. Mutually exclusive with blame view.

**Session Diff** (`:diff from to`) -- compare two points in the session timeline. Shows per-file change summaries with lines added/removed, edit counts, and which agents touched each file.

**Bookmarks** (`M` to create, `'` to jump) -- mark interesting positions in the timeline for quick navigation. Bookmark list displayed as a popup overlay.

### Restore System

Scrubbing is visual. Restoring writes to disk. These are separate actions.

- **Restore file** (`R`) -- write the file at the current playhead position to disk
- **Undo restore** (`u`) -- every restore is logged, reverse the last one
- **Content-addressed snapshot store** -- every file version stored by SHA-256 hash
- **Checkpoints** (`c`) -- manual full-project snapshots, plus auto-checkpoints every N edits (configurable)
- **CLI restore** -- headless restore without opening the TUI: `vibetracer restore <file> --edit-id <N>`

### Analysis Engines

**Blast Radius** (`b`) -- when a file is edited, see which dependent files may need updating. Catches partial refactors where the AI updates 3 of 5 coupled files and moves on. Tracks stale, updated, and untouched dependents.

**Sentinels** -- cross-file invariant rules. Define assertions like "feature count in config must match model input size." vibetracer alerts instantly when an edit breaks the invariant.

**Watchdog** (`w`) -- register constants that should never change (physics values, API endpoints, config thresholds). Get alerted the moment the AI modifies a watched value. Severity levels: critical, warning, info.

**Configurable Alerts** -- trigger notifications based on session state:

| Condition | Example |
|-----------|---------|
| `session_cost > 1.00` | Cost exceeded threshold |
| `sentinel_failures > 0` | Invariant broken |
| `stale_count > 3` | Too many stale dependents |
| `edit_velocity > 10` | Unusually high edit rate |
| `edit_count > 100` | Large session |

Alert actions: `toast` (status bar), `flash` (screen flash), `bell` (terminal bell). Alerts auto-rearm when the condition becomes false.

### Multi-Agent Support

- Claude Code, Cursor, and Codex CLI all supported
- Import sessions from any agent
- Agent color coding on timeline tracks
- Solo agent filtering (`1`-`9` in command view)
- Per-agent edit attribution on every event
- Conflict indicators when two agents edit the same file within 5 seconds

### Export and Integration

**Agent Trace JSON** -- vendor-neutral session export compatible with Cursor and the git-ai ecosystem:

```bash
vibetracer export --format agent-trace <session-id>
vibetracer export --format agent-trace <session-id> --output trace.json
```

**git-ai compatible git notes** -- attach authorship logs directly to commits:

```bash
vibetracer export --format git-notes <session-id>
```

**MCP Server** -- 7 tools exposed via Model Context Protocol for AI self-correction:

```bash
vibetracer mcp   # start stdio JSON-RPC server
```

| MCP Tool | Description |
|----------|-------------|
| `list_sessions` | List recorded sessions with metadata and edit counts |
| `get_timeline` | Get the edit timeline (paginated, filterable by file glob) |
| `get_frame` | Reconstruct exact file state at any timeline point |
| `diff_frames` | Unified diff between any two timeline points |
| `search_edits` | Find frames where a pattern was modified (regex) |
| `get_regression_window` | Candidate frames for bisecting a regression |
| `subscribe_edits` | Subscribe to live edit notifications |

All list-returning tools support `offset`/`limit` pagination and stream JSONL lazily.

Add to your MCP client configuration:

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

**Claude Code Hook** -- auto-registers a `PostToolUse` hook when `.claude/` is detected, capturing tool metadata and intent context on every edit.

---

## Keybindings

### Normal Mode

| Key | Action |
|-----|--------|
| `q` | Quit TUI (daemon keeps running) |
| `Q` | Quit TUI and stop daemon |
| `?` | Help overlay |
| `Space` | Play / pause |
| `Left` / `Right` | Scrub through edits (global playhead) |
| `Shift+Left` / `Shift+Right` | Scrub per-file (detaches from global timeline) |
| `a` | Reattach detached file to global playhead |
| `t` | Enter Timeline mode |
| `i` | Enter Inspect mode |
| `/` | Enter Search mode |
| `:` or `Ctrl+P` | Open command palette |
| `g` | Toggle edit view / command view |
| `d` | Toggle preview mode (file / diff) |
| `D` | Toggle dashboard panel |
| `C` | Toggle conversation panel |
| `B` | Toggle blame view |
| `A` | Toggle inline annotations |
| `M` | Create bookmark at current playhead |
| `'` | Jump to bookmark (popup list) |
| `R` | Restore file at playhead to disk |
| `u` | Undo last restore |
| `c` | Create checkpoint |
| `x` | Toggle showing restore-generated edits |
| `s` | Solo track (isolate current file) |
| `m` | Mute track (hide current file) |
| `b` | Toggle blast radius panel |
| `w` | Toggle watchdog panel |
| `z` | Maximize focused panel |
| `j` / `k` | Scroll preview down / up |
| `+` / `=` | Zoom timeline in |
| `-` | Zoom timeline out |
| `0` | Reset timeline zoom |
| `Tab` | Cycle pane focus |
| `1`-`9` | Solo agent N (command view) |

### Timeline Mode (`t` to enter)

| Key | Action |
|-----|--------|
| `Esc` | Exit to Normal mode |
| `Left` / `Right` | Pan timeline |
| `Up` / `Down` | Select track |
| `+` / `-` | Zoom in / out |
| `=` | Reset zoom |
| `s` | Solo selected track |
| `m` | Mute selected track |
| `Enter` | Jump playhead to selected track |
| `q` | Quit |

### Inspect Mode (`i` to enter)

| Key | Action |
|-----|--------|
| `Esc` | Exit to Normal mode |
| `n` | Next edit |
| `p` | Previous edit |
| `d` | Toggle diff view |
| `f` | Show full file |
| `c` | Show conversation context |
| `Enter` | Expand details |
| `q` | Quit |

### Search Mode (`/` to enter)

| Key | Action |
|-----|--------|
| `Esc` | Cancel and exit |
| `Enter` | Lock filter and return to Normal mode |
| `Backspace` | Delete character |
| `Up` / `Down` | Scroll results |
| Any character | Append to search query |

### Command Palette (`:` or `Ctrl+P`)

Fuzzy search across all available actions. Recently-used entries float to the top. Navigate with arrow keys, confirm with `Enter`, dismiss with `Esc`.

---

## Configuration

Run `vibetracer init` to auto-detect constants, schemas, and dependencies in your project. Configuration lives in `.vibetracer/config.toml`.

```toml
# .vibetracer/config.toml

[theme]
preset = "tokyo-night"

[watch]
debounce_ms = 100
auto_checkpoint_every = 25
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer"]

# Watchdog: alert when registered constants change
[[watchdog.constants]]
pattern = "MAX_RETRIES"
file = "src/config.rs"
severity = "critical"

[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"

# Sentinels: cross-file invariant rules
[sentinels.feature_count]
watch = "src/model.rs"
assert_eq = "src/features.rs"
description = "feature count must match model input"

[sentinels.tensor_dims]
description = "tensor input dimensions must match feature count"
watch = "**/*.py"
rule = "grep_match"
pattern_a = { file = "config.py", regex = 'N_FEATURES\s*=\s*(\d+)' }
pattern_b = { file = "model.py", regex = 'input_size\s*=\s*(\d+)' }
assert = "a == b"

# Blast radius: declare file dependencies
[[blast_radius.manual]]
source = "src/auth.rs"
dependents = ["src/session.rs", "src/api/login.rs"]

[[blast_radius.manual]]
source = "**/config*.py"
dependents = ["**/model*.py", "**/serving*.py"]

# Configurable alerts
[[alerts]]
name = "cost-warning"
when = "session_cost > 1.00"
action = "toast"
message = "Session cost exceeded $1.00"

[[alerts]]
name = "sentinel-break"
when = "sentinel_failures > 0"
action = "flash"
message = "Invariant broken"

[[alerts]]
name = "runaway-edits"
when = "edit_velocity > 15"
action = "bell"
message = "Unusually high edit rate"
```

---

## CLI Reference

```
vibetracer [path]                          Watch directory (default: cwd)
vibetracer demo                            Interactive feature demo
vibetracer replay <session>                Replay a past session
vibetracer sessions                        List past sessions
vibetracer import [session]                Import Claude Code session
vibetracer restore <file> --edit-id <N>    Restore a file to a specific edit
vibetracer export --format agent-trace <session>   Export as Agent Trace JSON
vibetracer export --format git-notes <session>     Export as git-ai git notes
vibetracer mcp                             Start MCP server (stdio JSON-RPC)
vibetracer daemon start|stop|status        Manage background daemon
vibetracer init                            Create config with auto-detection
vibetracer --no-daemon [path]              Single-process mode (no daemon)
vibetracer --debug [path]                  Write debug log for troubleshooting
```

---

## Themes

19 built-in themes. Cycle at runtime with the command palette -- no restart needed.

**Dark themes:** dark (default), catppuccin-mocha, catppuccin-macchiato, gruvbox-dark, tokyo-night, tokyo-night-storm, dracula, nord, kanagawa, rose-pine, one-dark, solarized-dark, everforest-dark

**Light themes:** light, catppuccin-latte, gruvbox-light, solarized-light, rose-pine-dawn, everforest-light

Set a default in config:

```toml
[theme]
preset = "tokyo-night"
```

---

## Architecture

```
vibetracer
  daemon/           Background recorder (filesystem watcher + snapshot store + edit log)
  recorder/         Shared recording logic (used by daemon and --no-daemon mode)
  snapshot/         Content-addressed file storage (SHA-256) + append-only JSONL edit journal
  checkpoint/       Full project state snapshots (manual + auto)
  restore/          File restoration engine + conflict checker
  analysis/         Blast radius, sentinels, watchdog (evaluated on each edit)
  tui/              Terminal UI: modal system, dashboard, timeline, preview, panels
    widgets/        Command palette, conversation panel, dashboard sparklines
    filter.rs       Composable search/filter engine
    session_diff.rs Point-in-time session comparison
    alerts.rs       Configurable alert evaluator with auto-rearm
    bookmarks.rs    Timeline position bookmarks
  claude_log/       Claude Code conversation log parser (background thread)
  hook/             Claude Code PostToolUse hook registration
  import/           Multi-agent session import (Claude Code, Cursor, Codex CLI)
  export/           Session export (Agent Trace JSON, git-ai git notes)
  mcp/              MCP server (JSON-RPC stdio, 7 tools for AI self-correction)
  theme/            19 color themes with runtime switching
```

The daemon records file changes to an append-only JSONL edit journal in `.vibetracer/`. Every file version is stored in a content-addressed snapshot store keyed by SHA-256 hash. The TUI tails the edit log in real-time and renders the cockpit. Claude Code's conversation logs are parsed in a background thread. Analysis engines evaluate on each incoming edit and surface alerts. The layout uses a dynamic panel registry with focus cycling.

Data is stored in `.vibetracer/` within your project directory. Add it to `.gitignore`.

---

## Installation

```bash
# From crates.io
cargo install vibetracer

# From source
git clone https://github.com/omeedcs/vibetracer
cd vibetracer
cargo install --path .

# Homebrew (macOS)
brew install omeedcs/tap/vibetracer
```

**Requirements:** Rust 1.70+ (for installation from source)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
