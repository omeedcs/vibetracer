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

**Director's Cut Interface** — Your coding session is a film. Each file is a track. Each edit is a clip. Scrub through time with a playhead, play back the session, or rewind to any point.

**Three Tiers of Time Travel:**
- Edit-level: undo individual edits surgically
- File-level: rewind a specific file to any prior state
- Session checkpoints: snapshot the entire project and jump between states

**Blast Radius Detection** — When a file is edited, see which dependent files need updating. Catches partial refactors where the AI updates 3 of 5 files and moves on.

**Invariant Sentinels** — Define rules like "tensor input dimensions must match feature count." vibetracer alerts you instantly when an edit breaks an invariant.

**Constants Watchdog** — Register values that should never change (physics constants, API endpoints). Get an alert if the AI "helpfully" modifies them.

**Refactor Tracker** — When a function is renamed, track propagation across the codebase. See how many call sites have been updated vs. how many remain.

**Schema Diff** — Structural diffs for Pydantic models, TypeScript interfaces, and SQL schemas. See "field `age` added" instead of raw text diffs.

**Equation Lens** — Render LaTeX equations found in code comments as typeset math. Watch equations evolve across edits.

**Claude Code Integration** — Optional enriched mode: when connected to Claude Code via hooks, edits include intent labels ("adding auth middleware") and tool metadata.

**Session Import** — Replay past Claude Code sessions with full analysis applied retroactively.

**Embedded Terminal** — Run Claude Code inside vibetracer itself. No tab switching.

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

# Import a past Claude Code session
vibetracer import

# Replay a saved session
vibetracer replay <session-id>

# List past sessions
vibetracer sessions

# Create default config
vibetracer init
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
| `Ctrl+\` | toggle terminal focus |
| `?` | help |
| `q` | quit |

## Configuration

Run `vibetracer init` to create `.vibetracer/config.toml`:

```toml
[watch]
debounce_ms = 100
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer"]
auto_checkpoint_every = 25

# Define invariant rules
[sentinels.tensor_dims]
description = "feature count must match model input size"
watch = ["**/feature_config*.py", "**/predictor*.py"]
rule = "grep_match"
pattern_a = { file = "**/feature_config*.py", regex = 'N_TEMPORAL\s*=\s*(\d+)' }
pattern_b = { file = "**/predictor*.py", regex = 'input_size\s*=\s*(\d+)' }
assert = "a == b"

# Watch constants that should never change
[[watchdog.constants]]
file = "**/*.py"
pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
expected = "6371.0"
severity = "critical"

# Declare file dependencies
[blast_radius]
auto_detect = true

[[blast_radius.manual]]
source = "**/feature_config*.py"
dependents = ["**/predictor*.py", "**/serving*.py"]
```

## How It Works

vibetracer watches your project directory for filesystem changes. Each edit is captured as a diff, stored in a content-addressed snapshot store, and logged to an append-only edit journal. The TUI renders this as a horizontal multi-track timeline (inspired by non-linear video editors).

When Claude Code is detected, vibetracer registers a `PostToolUse` hook to capture tool metadata and intent context, enriching each edit with "why" in addition to "what."

All data is stored locally in `.vibetracer/` within your project directory.

## Tech Stack

Rust, ratatui, crossterm, notify, similar, serde, clap, portable-pty

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
