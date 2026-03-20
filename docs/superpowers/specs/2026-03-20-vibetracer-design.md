# Vibetracer Design Spec

A real-time terminal tool for tracing, replaying, and rewinding the edits that AI coding assistants make to your codebase. Built for developers who vibe code and need to stay in control.

## Problem

When an AI coding assistant (Claude Code, Cursor, Copilot, etc.) is making edits to your project, you lose track of what changed, where, why, and in what order. By the time something breaks, the relevant edit is buried under 20 others. You can't surgically undo it. You can't replay the session to understand what happened. You're left doing `git diff` archaeology after the fact.

vibetracer solves this by giving you a live, scrubable, rewindable timeline of every edit — with optional awareness of *why* each change was made when connected to Claude Code.

## Core Metaphor: Director's Cut

The interface borrows from non-linear video editors (Premiere Pro, DaVinci Resolve). Your coding session is a film. Each file is a track. Each edit is a clip on that track. A playhead lets you scrub through time. You can play, pause, rewind, cut ranges, and solo individual tracks.

Two zones:
- **Preview pane** (top ~60%) — shows the diff or full file content at the playhead position, with optional overlays (equation lens, schema diff)
- **Track timeline** (bottom ~40%) — horizontal per-file tracks with edit blocks, checkpoints, and a movable playhead

Right sidebar (toggleable) shows contextual panels: blast radius, sentinels, refactor tracker, equation index, edit details.

## Architecture

Four internal subsystems, single binary:

```
Filesystem events --> Watcher --> Event Bus --> Snapshot Engine (persist)
Claude Code hooks --> Hook Bridge --/                |
                                              Renderer (TUI)
```

### Watcher

Uses the `notify` crate to watch the project directory for filesystem events. Debounces rapid changes (AI tools often write files multiple times in quick succession). On each stabilized change, computes a diff against the last known file state and emits an Edit event to the event bus.

Configurable ignore patterns (`.git`, `node_modules`, `target`, `__pycache__`, `.vibetracer` by default).

### Hook Bridge

Optional enrichment layer. If Claude Code is detected (`.claude/` directory exists), vibetracer registers a `PostToolUse` hook that sends lightweight payloads to a local Unix domain socket. This provides:

- Exact tool name and parameters (Edit old_string/new_string, Write file_path)
- Conversation context for intent labeling
- Precise timing without filesystem debounce guesswork

Hook payloads are merged with filesystem watcher events to produce enriched edit entries. If the hook fires but no filesystem change occurs (no-op edit), it is logged but not tracked.

Hook registration: vibetracer writes a hook entry to `.claude/settings.local.json` under the `hooks.PostToolUse` array, pointing to a shell command that POSTs the hook payload to vibetracer's Unix socket. On shutdown, vibetracer removes the entry.

Falls back to passive mode (pure filesystem watching) if Claude Code is not detected or disconnects mid-session. The status bar reflects the current mode.

### Snapshot Engine

Maintains three tiers of history:

**Edit log** — every individual diff as a compact unified patch. Append-only, one JSON line per edit in `edits.jsonl`.

**File snapshots** — full file contents at each edit point. Content-addressed (SHA-256), deduplicated. A file that hasn't changed doesn't get re-stored. Enables instant rewind to any point without replaying patches.

**Session checkpoints** — full project state snapshots. Triggered manually (keybinding) or automatically (every N edits, configurable). Lists every tracked file and its snapshot hash.

### Renderer

ratatui-based TUI. Renders two zones plus an optional sidebar. Handles keyboard/mouse input for playback, scrubbing, rewind, and mode switching.

Supports iTerm2 and Kitty inline image protocols for rendering LaTeX equations as images. Falls back to Unicode math symbols when neither protocol is available.

## Feature: Equation Lens

Toggle with `e`. Scans the file in the preview pane for mathematical expressions and renders them as typeset LaTeX inline beneath their source lines.

Detection:
- Annotated comments: `// @eq: E = mc^2` or `/// $\int_0^\infty e^{-x} dx$`
- LaTeX delimiters in doc comments: `$...$` or `$$...$$`
- Heuristic detection of mathematical assignments (configurable, off by default)

Rendering pipeline:
- Extract equation text from source
- Render to PNG via a bundled lightweight LaTeX engine (tectonic) or KaTeX-based renderer
- Display inline via iTerm2/Kitty image protocol
- Cache aggressively — only re-render when equation text changes
- Fall back to Unicode math rendering when image protocol unavailable

Equation Index panel in the sidebar lists all detected equations in the current file with line numbers. Clicking one jumps to it.

## Feature: Blast Radius

Toggle with `b`. When an edit occurs, vibetracer shows which other files depend on the changed file and whether they have been updated in the current session.

Dependency detection:
- Auto-detected from import/use/require statements (parsed per-language with tree-sitter)
- Manual overrides in `config.toml` for dependencies that imports don't capture (e.g., feature config files that ML models depend on by convention, not by import)

Display:
- Right sidebar panel listing dependents with status: updated, stale, untouched
- Track timeline annotates stale files with a "stale" marker

This catches the most dangerous class of vibe coding bugs: partial refactors where the AI updates 3 of 5 dependent files and moves on.

## Feature: Invariant Sentinels

Toggle with `i`. User-defined rules that watch for cross-file invariant violations.

Built-in rule types:
- `grep_match` — two regex patterns across two file globs, assert their captured values are equal
- `count_match` — count of items matching a pattern in file A must equal a value in file B
- `schema_fields_match` — fields in a schema definition must match columns in another file

Sentinels are evaluated on every edit that touches a watched file. Violations appear in the sidebar with the rule name, the mismatch, and the rule definition for context.

Not a general-purpose linter. Intentionally limited to a small set of rule types that are fast to evaluate and easy to reason about.

## Feature: Schema Diff Mode

Toggle with `d`. When the preview pane shows a file containing structured data definitions (Pydantic models, TypeScript interfaces, SQL CREATE TABLE, Terraform resource blocks, protobuf messages), renders a structural tree diff instead of a raw text diff.

Shows fields as a tree with types, highlights additions/removals/type changes. Prompts with relevant follow-up checks ("DB migration needed?", "API consumers affected?").

Detection is heuristic, based on file patterns and content structure. Supports Python (Pydantic, dataclasses), TypeScript (interfaces, types), SQL (DDL), Terraform (HCL resource blocks), and protobuf. Extensible via config.

## Feature: Cross-File Refactor Tracker

Toggle with `f`. When vibetracer detects a rename or addition that needs to propagate (a new field, a renamed function, a changed constant), it tracks propagation progress across the codebase.

Detection:
- New identifiers appearing in edits are grepped across the project to find all potential usage/definition sites
- Renamed identifiers (detected via edit pairs where old_string contains name A and new_string contains name B in the same position) are tracked for remaining references to the old name

Display:
- Progress bar showing files updated / files total
- List of remaining files that still reference the old name or don't yet reference the new one

## Feature: Constants Watchdog

Toggle with `w`. A registry of values that should almost never change. When an edit modifies a registered constant, vibetracer shows a high-severity alert with the expected value, the new value, and the downstream impact.

Configured in `config.toml`. Each entry specifies a file glob, a regex pattern to extract the value, and the expected value. Severity levels: `critical` (blocks the timeline with a full-width alert) and `warning` (sidebar note).

Useful for physics constants, API keys patterns, port numbers, well-known URLs, and any value that an AI might "helpfully" change.

## Data Model

Session IDs are timestamp-based with a short random suffix for uniqueness: `20260320-164532-a7f3`. Human-readable, sortable, easy to type or tab-complete.

```
.vibetracer/
  sessions/
    <session-id>/
      meta.json              # start time, project path, mode (enriched/passive)
      edits.jsonl            # append-only edit log
      snapshots/             # content-addressed file snapshots
        ab/cd1234...         # SHA-256 prefix directories
      checkpoints/
        001.json             # file path -> snapshot hash mapping
  config.toml               # sentinels, watchdog, blast radius config
```

Edit log entry schema:

```json
{
  "id": 28,
  "ts": 1710934532,
  "file": "src/models/config.py",
  "kind": "edit",
  "patch": "<unified diff>",
  "before": "ab/cd1234...",
  "after": "ef/gh5678...",
  "intent": "adding humidity feature",
  "tool": "Edit",
  "lines_added": 2,
  "lines_removed": 1
}
```

`intent` and `tool` are null in passive mode.

## Configuration

`config.toml` is created on `vibetracer init` with sensible defaults. Lives in `.vibetracer/`.

```toml
[watch]
debounce_ms = 100
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer"]
auto_checkpoint_every = 25

[sentinels.tensor_dims]
description = "feature count must match model input size"
watch = ["**/feature_config*.py", "**/predictor*.py"]
rule = "grep_match"
pattern_a = { file = "**/feature_config*.py", regex = 'N_TEMPORAL\s*=\s*(\d+)' }
pattern_b = { file = "**/predictor*.py", regex = 'input_size\s*=\s*(\d+)' }
assert = "a == b"

[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"

[blast_radius]
auto_detect = true

[[blast_radius.manual]]
source = "**/feature_config*.py"
dependents = ["**/predictor*.py", "**/serving*.py"]
```

## CLI

```
vibetracer                     # start watching current directory
vibetracer /path/to/project    # start watching specific directory
vibetracer replay <session-id> # replay a past session in the TUI
vibetracer sessions            # list past sessions with timestamps and edit counts
vibetracer init                # create .vibetracer/config.toml with defaults
vibetracer --split             # launch in a new tmux pane to the right
```

Startup sequence:
1. Check for `.vibetracer/` — create if missing, prompt to add to `.gitignore`
2. Check for `.claude/` — if found, register hooks, enter enriched mode
3. Start filesystem watcher
4. Start Unix socket listener for hook payloads
5. Open TUI

Sessions auto-save continuously. Retained for 30 days by default.

## Keybindings

| Key | Action |
|-----|--------|
| `Space` | play / pause session replay |
| `left/right` | scrub edit by edit |
| `Shift+left/right` | jump between checkpoints |
| `1-5` | playback speed (1x, 2x, 4x, 6x, 8x) |
| `r` | rewind all files to playhead position |
| `R` | rewind only the focused file |
| `x` | enter range select mode, cut a span of edits |
| `u` | undo last rewind |
| `c` | create manual checkpoint |
| `s` | solo a track (show only that file) |
| `m` | mute a track (hide from timeline) |
| `g` | group tracks by intent (enriched mode) |
| `e` | toggle equation lens |
| `b` | toggle blast radius panel |
| `i` | toggle sentinels panel |
| `d` | toggle schema diff mode |
| `f` | toggle refactor tracker |
| `w` | toggle watchdog log |
| `Tab` | cycle focus between panes |
| `/` | search edits (by file, intent, or content) |
| `?` | help overlay |
| `q` | quit |

## Distribution & Installation

vibetracer is a Rust binary. Installation should be one command.

### Homebrew (primary for macOS)

```
brew install vibetracer
```

Publish via a homebrew tap (`vibetracer/tap`) initially, submit to homebrew-core once stable.

### cargo install

```
cargo install vibetracer
```

Works anywhere Rust compiles. Requires a Rust toolchain.

### Prebuilt binaries via GitHub Releases

Every tagged release publishes prebuilt binaries for:
- macOS arm64 (Apple Silicon)
- macOS x86_64
- Linux x86_64
- Linux arm64

Automated via GitHub Actions using `cross` for cross-compilation. Binaries are statically linked where possible.

Install script for the lazy:
```
curl -fsSL https://vibetracer.dev/install.sh | sh
```

The install script detects OS/arch, downloads the right binary, and places it in `~/.local/bin` (or `/usr/local/bin` with sudo).

### LaTeX rendering dependency (optional)

Equation lens requires a LaTeX renderer. Two options, checked in order:
1. `tectonic` — lightweight, self-contained LaTeX engine. Can be installed via `cargo install tectonic` or `brew install tectonic`.
2. `katex` — JavaScript-based, invoked via a bundled Node script. Requires Node.js.

If neither is available, equation lens falls back to Unicode math rendering (still useful, just less pretty). vibetracer prints a one-time note on startup if no LaTeX renderer is found.

### Python helper (optional, for advanced sentinel rules)

For projects that want to write custom sentinel rules beyond the built-in types, a Python helper can evaluate arbitrary assertions. Installed via:

```
uv tool install vibetracer-sentinel
```

This is optional and not required for core functionality.

## Tech Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, single binary, great TUI ecosystem |
| TUI framework | ratatui + crossterm | Most mature Rust TUI stack, good mouse support |
| File watching | notify | Cross-platform, battle-tested |
| Diffing | similar | Rust diff library, fast, supports unified diffs |
| Import parsing | tree-sitter | Multi-language AST parsing for blast radius auto-detect. v1 bundles grammars for Python, TypeScript, Rust, Go, and Java (compiled into the binary via tree-sitter's C bindings). Additional languages added based on demand. |
| Serialization | serde + serde_json | Standard Rust serialization |
| Config | toml | Human-readable, Rust-native support |
| CLI parsing | clap | Standard Rust CLI framework |
| Image protocol | viuer or custom | iTerm2/Kitty inline image rendering |
| LaTeX | tectonic (optional) | Self-contained, no TeX Live dependency |
| Build/CI | cargo + GitHub Actions | Standard Rust toolchain |
| Cross-compilation | cross | Docker-based cross-compilation for release binaries |

## Non-Goals

- Not a code editor. vibetracer does not modify files (except during rewind, which restores snapshots). Rewind always creates an automatic pre-rewind checkpoint first, and prompts for confirmation before writing to disk. The `u` keybinding restores from this checkpoint.
- Not a git replacement. Sessions are ephemeral working memory, not version control.
- Not a linter. Sentinels check cross-file invariants, not code style.
- Not IDE-specific. Works with any tool that edits files. Claude Code integration is optional enrichment.

## Future Considerations (not in v1)

- VS Code extension that embeds the TUI in a panel
- Multi-session comparison (diff two sessions)
- Team mode — share sessions for async code review
- Plugin system for custom renderers and rule types
- Web UI replay viewer for sharing session recordings
