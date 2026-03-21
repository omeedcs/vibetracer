# vibetracer

Real-time tracing, replaying, and rewinding of AI coding assistant edits. Built for developers who vibe code and need to stay in control.

```
        _ _          _
 __   _(_) |__   ___| |_ _ __ __ _  ___ ___ _ __
 \ \ / / | '_ \ / _ \ __| '__/ _` |/ __/ _ \ '__|
  \ V /| | |_) |  __/ |_| | | (_| | (_|  __/ |
   \_/ |_|_.__/ \___|\__|_|  \__,_|\___\___|_|
```

## The Problem

When an AI coding assistant is making edits to your project, you lose track of what changed, where, why, and in what order. By the time something breaks, the relevant edit is buried under 20 others. You can't surgically undo it. You can't replay the session to understand what happened.

vibetracer solves this.

## Features

**Director's Cut Interface** -- Your coding session is a film. Each file is a track. Each edit is a clip. Scrub through time with a playhead, play back the session, or rewind to any point.

**Three Tiers of Time Travel:**
- Edit-level: undo individual edits surgically
- File-level: rewind a specific file to any prior state
- Session checkpoints: snapshot the entire project and jump between states

**Blast Radius Detection** -- When a file is edited, see which dependent files need updating. Catches partial refactors where the AI updates 3 of 5 files and moves on.

**Invariant Sentinels** -- Define rules like "tensor input dimensions must match feature count." vibetracer alerts you instantly when an edit breaks an invariant.

**Constants Watchdog** -- Register values that should never change (physics constants, API endpoints). Get an alert if the AI "helpfully" modifies them.

**Refactor Tracker** -- When a function is renamed, track propagation across the codebase. See how many call sites have been updated vs. how many remain.

**Schema Diff** -- Structural diffs for Pydantic models, TypeScript interfaces, and SQL schemas. See "field `age` added" instead of raw text diffs.

**Equation Lens** -- Render LaTeX equations found in code comments as typeset math (Unicode rendering). Watch equations evolve across edits.

**Claude Code Integration** -- Optional enriched mode: when connected to Claude Code via hooks, edits include intent labels ("adding auth middleware") and tool metadata. Auto-detected when `.claude/` directory exists.

**Session Import** -- Replay past Claude Code sessions with full analysis applied retroactively. Parses conversation JSONL from `~/.claude/projects/`.

**Embedded Terminal** -- Run Claude Code (or any command) inside vibetracer itself. No tab switching. Toggle focus with `Ctrl+\`.

**Smart Auto-Detection** -- `vibetracer init` scans your project and auto-generates watchdog rules for constants, sentinel rules for config/model invariants, and blast radius mappings for schema and config file dependencies.

**Color Themes** -- Choose from dark (default), catppuccin, gruvbox, or light. Set `preset` in `[theme]` section of config.

**Session Summaries** -- On exit, vibetracer generates a markdown summary of the session with files changed, line stats, and a timeline of every edit.

**Graceful Error Recovery** -- If vibetracer crashes, your terminal is always restored. Use `--debug` to write a log file for bug reports.

## Install

**Homebrew:**
```bash
brew install omeedcs/tap/vibetracer
```

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

**Script:**
```bash
curl -fsSL https://raw.githubusercontent.com/omeedcs/vibetracer/main/scripts/install.sh | sh
```

## Usage

```bash
# Watch current directory
vibetracer

# Watch a specific project
vibetracer ~/my-project

# Run with Claude Code embedded in a pane
vibetracer --embed ~/my-project

# Run with a custom embedded command
vibetracer --embed --cmd zsh ~/my-project

# Import a past Claude Code session (list available)
vibetracer import

# Import a specific session by ID
vibetracer import <session-id>

# Replay a saved vibetracer session
vibetracer replay <session-id>

# List past vibetracer sessions
vibetracer sessions

# Initialize config with smart auto-detection
vibetracer init

# Run the scripted demo (for recording GIFs)
vibetracer demo

# Skip the startup animation
vibetracer --no-splash ~/my-project

# Write debug log for troubleshooting
vibetracer --debug ~/my-project

# Show version
vibetracer --version
```

## Keybindings

| Key | Action |
|-----|--------|
| `Space` | play / pause |
| `left/right` | scrub edit by edit |
| `Shift+left/right` | jump between checkpoints |
| `1-5` | playback speed |
| `r` | rewind all files to playhead |
| `R` | rewind focused file only |
| `u` | undo last rewind |
| `c` | create checkpoint |
| `x` | cut range of edits |
| `s` | solo a track |
| `m` | mute a track |
| `e` | equation lens |
| `b` | blast radius |
| `i` | sentinels |
| `d` | schema diff |
| `f` | refactor tracker |
| `w` | watchdog |
| `Ctrl+\` | toggle terminal focus (embedded mode) |
| `Tab` | cycle between panes |
| `?` | help overlay |
| `q` | quit |

## Configuration

Run `vibetracer init` in your project directory. It scans for constants, schemas, and config files, then generates `.vibetracer/config.toml` with auto-detected rules.

Example generated config:

```toml
# vibetracer configuration (auto-generated)
# https://github.com/omeedcs/vibetracer

[watch]
debounce_ms = 100
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer", ".venv"]
auto_checkpoint_every = 25

[theme]
preset = "dark"    # options: "dark", "catppuccin", "gruvbox", "light"

# Auto-detected watchdog constants
[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"

[[watchdog.constants]]
file = "**/*.py"
pattern = 'SPEED_OF_LIGHT\s*=\s*([\d.]+)'
expected = "299792.458"
severity = "critical"

# Auto-detected sentinel rules
[sentinels.feature_count]
description = "feature count must match model input size"
watch = "**/*.py"
rule = "grep_match"
pattern_a = { file = "config.py", regex = 'N_FEATURES\s*=\s*(\d+)' }
pattern_b = { file = "model.py", regex = 'input_size\s*=\s*(\d+)' }
assert = "a == b"

# Auto-detected file dependencies
[blast_radius]
auto_detect = true

[[blast_radius.manual]]
source = "**/config*.py"
dependents = ["**/model*.py", "**/serving*.py"]
```

You can also write rules manually. See the [design spec](docs/superpowers/specs/2026-03-20-vibetracer-design.md) for the full configuration reference.

## How It Works

vibetracer watches your project directory for filesystem changes. Each edit is captured as a diff, stored in a content-addressed snapshot store, and logged to an append-only edit journal. The TUI renders this as a horizontal multi-track timeline inspired by non-linear video editors like Premiere Pro.

When Claude Code is detected (`.claude/` directory exists), vibetracer automatically registers a `PostToolUse` hook to capture tool metadata and intent context, enriching each edit with "why" in addition to "what." The hook is removed on exit.

Analysis engines run automatically on every edit:
- **Watchdog** fires when a registered constant is modified
- **Sentinels** evaluate when a watched file pattern is touched
- **Blast radius** shows dependent files when a source file changes

All data is stored locally in `.vibetracer/` within your project directory. Add it to your `.gitignore`.

## Tech Stack

Rust, ratatui, crossterm, notify, similar, serde, clap, portable-pty, vt100

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
