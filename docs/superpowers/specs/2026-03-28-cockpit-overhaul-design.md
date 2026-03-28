# Vibetracer Cockpit Overhaul — Design Spec

**Date:** 2026-03-28
**Version:** 0.6.0 target
**Approach:** Layered Build (infrastructure + features in alternating waves)

## Summary

Transform vibetracer from a timeline replay tool into a full observability cockpit for AI coding sessions. Dense htop-style UI, deep Claude Code integration, live dashboards, powerful investigation tools, and a vim-modal + command palette interaction model. Claude-first, agent-agnostic architecture.

## Design Principles

- **htop-density:** Every pixel carries information. Sparklines, compact bars, inline stats. No wasted whitespace.
- **Vim-modal + command palette:** Modes for muscle memory, palette for discovery. Both always available.
- **Claude-first, others second:** Deep Claude Code log parsing and correlation. Architecture stays open for Cursor/Codex.
- **Full-spectrum usage:** Equally good for live monitoring, active intervention, and post-session investigation.
- **Self-contained data, hybrid sources:** Store everything locally, but pull from Claude Code logs and process info at runtime.

---

## Wave 1: Interaction Infrastructure

### 1.1 Command Palette

- Triggered by `:` (vim-style) or `Ctrl+P`
- Fuzzy search over all registered actions, panels, views, settings
- Keybinding hints shown next to each action
- Recent commands float to top (MRU ordering)
- Context-aware — available actions change based on current focus and mode
- Implemented as an overlay that captures all input until dismissed

### 1.2 Vim-style Modal System

Four modes, indicator shown in status bar:

| Mode | Entry | Exit | Purpose |
|------|-------|------|---------|
| **Normal** | `Esc` | — | Default. Scrubbing, navigation, toggling panels. |
| **Timeline** | `t` | `Esc` | Focused timeline manipulation — zoom, pan, solo/mute, jump-to-file. |
| **Inspect** | `i` | `Esc` | Deep-dive on selected edit/operation — metadata, diff, reasoning, dependencies. |
| **Search** | `/` | `Esc` | Filter edits by file, agent, time range, content, intent. Live-narrowing. |

- Theme cycling (currently `t`) moves to command palette (`:theme <name>` or `:theme next`)
- Each mode has its own keybinding set displayed in the bottom keybinding bar
- Mode transitions are instant (no animation)
- Modes are exclusive — entering one exits the previous

### 1.3 Panel Registry & Layout Engine

- Panels are registered components implementing a `Panel` trait: `render()`, `handle_input()`, `title()`, `id()`
- Dynamic panel grid replaces current fixed layout
- Panels can be: toggled visible/hidden, maximized (full-screen takeover with `z`), focused (`Tab` cycle or `1-9` jump)
- Layout persisted to `config.toml` across sessions
- Default layout is the dense cockpit view (see below)

### 1.4 Default Cockpit Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ [mode] [session] [edits:142] [tokens:45.2k] [cost:$0.38] [agents] │
├────────────────────────────────────┬────────────────────────────────┤
│                                    │  Dashboard                     │
│  Preview (file/diff)               │  (sparklines, file heatmap,   │
│  (syntax highlighted)              │   agent status, operations,   │
│                                    │   blast radius, sentinels,    │
│                                    │   watchdog — all condensed)   │
├────────────────────────────────────┴────────────────────────────────┤
│ Timeline  [file tracks with edit cells, playhead, agent colors]     │
├─────────────────────────────────────────────────────────────────────┤
│ [MODE] context-sensitive keybinding hints                           │
└─────────────────────────────────────────────────────────────────────┘
```

- Preview: 65% width when dashboard visible, 100% when hidden
- Dashboard: 35% width right panel
- Timeline: 4-8 lines adaptive to terminal height
- Status bar: 1 line, dense with session metrics
- Keybinding bar: 1 line, updates per-mode

---

## Wave 2: Claude Deep Integration

### 2.1 Claude Code Log Parser

- Parse Claude Code's JSONL conversation logs
- Locate logs via `~/.claude/projects/` directory structure or configurable path
- Extract per-turn: user prompts, assistant text, tool calls (name, args, result summary), token counts (input, output, cache read/write), model ID, conversation ID
- Correlate with vibetracer EditEvents via timestamp proximity (5-second window) and file path matching
- Background thread tails log file in real-time during live sessions
- Graceful degradation — if no Claude logs found, all other features still work

### 2.2 Conversation Timeline Panel

Vertical stream showing Claude's conversation turns and tool calls:

```
14:32:01  USER   "fix the auth middleware to use JWT"
14:32:03  CLAUDE [thinking 2.1s]
  ├─ Read src/auth.rs
  ├─ Read src/middleware.rs
  ├─ Grep "session_token"        12 matches
  ├─ Edit src/auth.rs            +14 -8
  ├─ Edit src/middleware.rs      +22 -3
  ├─ Edit src/config.rs          +4  -1
  └─ Write tests/auth_test.rs   +45
14:32:41  CLAUDE [complete]  "I've updated the auth..."
14:32:55  USER   "also add rate limiting"
```

- Each tool call navigable — `Enter` jumps to that edit in the timeline
- Non-edit tools (Read, Grep, Bash, WebSearch) shown for reasoning visibility
- Turns collapsible — collapsed shows just timestamp + summary, expanded shows all tool calls
- Color-coded: user prompts, Claude text, edit tools, read tools, other tools
- Scrollable independently of other panels

### 2.3 Tool Call Inspector

Full-screen inspect view when entering Inspect mode on an edit:

- **Header:** file, kind, agent, tool, operation, intent, timestamp
- **Context:** originating prompt (truncated), turn position (e.g., "3 of 7 tool calls"), previous/next tool call
- **Token info:** input tokens, output tokens, cache hit, cost for this turn
- **Diff:** full unified diff with syntax highlighting
- **Navigation:** `n`/`p` for next/prev edit, `d` toggle diff/file view, `Esc` to exit inspect

### 2.4 Token & Cost Tracker

- Running totals in status bar: `tokens:45.2k cost:$0.38`
- Per-turn breakdown available in dashboard panel
- Tracks: input tokens, output tokens, cache read tokens, cache write tokens
- Cost calculation based on model pricing (configurable in config.toml, sensible defaults for Claude models)
- Sparkline of token burn rate in dashboard
- Cache hit ratio displayed as percentage

---

## Wave 3: Live Dashboards

### 3.1 Dashboard Panel

Dense htop-style stats panel, always-visible in default layout (right side, 35% width):

**Sections (top to bottom):**

1. **Tokens** — `in:32.1k out:13.1k cache:68%` with 20-char sparkline of token rate
2. **Cost** — `$0.38 ($0.12/min)` with sparkline
3. **Edit Velocity** — `4.2 edits/min` with sparkline (rolling 60s window)
4. **File Heatmap** — horizontal bars sorted by edit count, top 5-7 files shown, rest collapsed
5. **Agents** — per-agent edit count, active/idle status, color swatch
6. **Operations** — list with progress indication (edit count), active operation highlighted
7. **Blast Radius** — summary: `stale:3 updated:5 untouched:22`, warning lines for stale deps
8. **Sentinels** — summary: `2 pass 1 FAIL`, failure details inline
9. **Watchdog** — summary: `all clear` or alert lines

All sections update in real-time. Sparklines shift left as new data arrives.

### 3.2 Dense Status Bar

Single line, maximum information:

```
NORMAL | live | 142 edits | 12 files | 2 agents | 45.2k tok | $0.38 | 4.2 ed/m | 14:32 elapsed
```

- Mode indicator (left, colored by mode)
- Playback state (live/paused/playing)
- Core counters
- Elapsed session time (right-aligned)

### 3.3 Sparkline Rendering

Utility for rendering sparklines throughout the UI:

- Characters: `▁▂▃▄▅▆▇█` (8 levels)
- Fixed width (20 chars default, configurable per context)
- Auto-scaling to data range
- Used for: token rate, cost rate, edit velocity, per-file edit density

---

## Wave 4: Investigation Tools

### 4.1 Search & Filter System

Activated by `/` to enter Search mode. Live-filtering input that narrows the entire UI.

**Filter syntax:**

| Filter | Example | Matches |
|--------|---------|---------|
| `file:` | `file:auth` | Files containing substring |
| `agent:` | `agent:claude-1` | Edits by agent |
| `kind:` | `kind:create` | Create/modify/delete |
| `tool:` | `tool:Write` | Tool name |
| `after:`/`before:` | `after:14:30` | Time range (HH:MM or edit ID) |
| `lines>` / `lines<` | `lines>20` | Line count threshold |
| `op:` | `op:JWT` | Fuzzy match on operation intent |
| `content:` | `content:session_token` | Grep through diffs |
| bare text | `JWT` | Fuzzy match across all fields |

- Filters compose with space (AND logic)
- Timeline dims non-matching edits, highlights matches
- Preview snaps to first match
- `Enter` locks filter (stays active in Normal mode), `Esc` clears

### 4.2 Operation Dependency Graph

Panel showing inter-operation relationships:

- Operations as nodes, edges from file overlap or temporal sequence
- Rendered as ASCII box-drawing graph
- Conflict highlights where multiple operations touch the same file
- Arrow key navigation between nodes, `Enter` to expand operation details
- Accessible via command palette `:graph` or keybinding in Normal mode

### 4.3 Blame View

Per-line attribution in the preview pane, toggled with `B`:

- Each line annotated with operation ID, agent, and timestamp of last modification
- Color-coded by agent
- Visual separators between contiguous regions from the same edit
- `Enter` on a line jumps to the edit that produced it
- Original (unmodified) lines marked as "original"

### 4.4 Session Diff

Compare two points within a session:

- Triggered via `:diff <from> <to>` (edit IDs or timestamps)
- Aggregated diff across all files for the range
- Shows per-file summary (files changed, lines added/removed) and detailed diffs
- Useful for "what did Claude do in the last 5 minutes?"

### 4.5 Bookmarks

- `M` to create bookmark at current playhead position (prompts for optional label)
- `'` to list bookmarks and jump
- Bookmarks stored in session `meta.json`
- Shown as markers on the timeline track
- Persist across TUI restarts for the same session

---

## Wave 5: Power Features

### 5.1 Inline Annotations

Contextual annotations alongside code in the preview pane:

- Toggle with `A` key
- Right-aligned annotation column showing operation intent or tool context for changed lines
- Pulled from: operation_intent, tool call context, conversation turn summary
- Only on changed lines — original code stays clean
- Color-coded to match agent/operation

### 5.2 Session Comparison

Side-by-side comparison of two sessions:

- Triggered via `:compare <session-a> <session-b>`
- Shows: edit counts, file lists, agent counts, token/cost totals, sentinel/watchdog outcomes
- Highlights differences in approach (which files touched, operation structure)
- Drill into any file to see divergent diffs
- Useful for comparing different approaches to the same task

### 5.3 Export Reports

Generate shareable reports from command palette:

- `:export markdown` — session summary, stats, operation list, key diffs
- `:export json` — machine-readable full session data
- `:export html` — styled standalone HTML with embedded sparklines and syntax highlighting
- Output path configurable, defaults to `.vibetracer/exports/`
- Reports include: summary, token/cost breakdown, operation graph, file heatmap, sentinel results

### 5.4 Live Process Monitor

Real-time Claude Code process info panel:

- Monitors Claude Code process (by PID or process name detection)
- Shows: current state (thinking, tool call, waiting), elapsed time per turn, CPU/memory
- Lightweight polling (1s interval)
- Accessible as a dashboard section or standalone panel
- Graceful degradation — shows "no process detected" if Claude Code isn't running

### 5.5 Notification Triggers

Configurable alerts in `config.toml`:

```toml
[[alerts]]
name = "cost-warning"
when = "session_cost > 1.00"
action = "toast"
message = "Session cost exceeded $1.00"

[[alerts]]
name = "sentinel-fail"
when = "sentinel_failures > 0"
action = "flash"

[[alerts]]
name = "stale-deps"
when = "stale_count > 3"
action = "toast"
```

- Actions: `toast` (status bar notification), `flash` (brief screen flash), `bell` (terminal bell)
- Triggers: cost thresholds, sentinel failures, stale dependency counts, edit velocity anomalies, agent idle time
- Evaluated on each state update

---

## Data Architecture Changes

### New Data Sources

1. **Claude Code JSONL logs** — parsed by new `claude_log` module
   - Path discovery: check `~/.claude/projects/` for matching project
   - Fields extracted: conversation turns, tool calls, token usage, model info
   - Correlation: timestamp + file path matching to vibetracer EditEvents

2. **Process monitor** — new `process` module
   - Polls `/proc` or macOS `sysctl` for Claude Code process info
   - Lightweight, non-intrusive

### Extended EditEvent (backward-compatible)

New optional fields added to EditEvent:

```rust
pub struct EditEvent {
    // ... existing fields ...

    // Wave 2 additions (from Claude log correlation)
    pub prompt_snippet: Option<String>,    // Truncated originating prompt
    pub turn_index: Option<u32>,           // Position within conversation turn
    pub turn_tool_count: Option<u32>,      // Total tools in this turn
    pub tokens_input: Option<u64>,         // Input tokens for this turn
    pub tokens_output: Option<u64>,        // Output tokens for this turn
    pub cache_hit: Option<bool>,           // Whether cache was used
    pub model: Option<String>,             // Model ID
}
```

All new fields are `Option` — old JSONL logs deserialize fine without them.

### New State in App

```rust
pub struct App {
    // ... existing fields ...

    // Wave 1
    pub mode: Mode,                        // Normal/Timeline/Inspect/Search
    pub command_palette: CommandPalette,
    pub panel_registry: PanelRegistry,

    // Wave 2
    pub conversation: Vec<ConversationTurn>,
    pub token_totals: TokenStats,

    // Wave 3
    pub dashboard: DashboardState,         // Sparkline buffers, computed metrics

    // Wave 4
    pub active_filter: Option<Filter>,
    pub bookmarks: Vec<Bookmark>,

    // Wave 5
    pub alerts: Vec<AlertConfig>,
}
```

---

## Keybinding Map (All Modes)

### Normal Mode
| Key | Action |
|-----|--------|
| `Space` | Play/pause |
| `Left`/`Right` | Scrub global playhead |
| `Shift+Left`/`Shift+Right` | Scrub per-file playhead |
| `a` | Reattach detached file |
| `t` | Enter Timeline mode |
| `i` | Enter Inspect mode |
| `/` | Enter Search mode |
| `:` or `Ctrl+P` | Command palette |
| `d` | Toggle diff/file view |
| `g` | Toggle command/edit view |
| `R` | Restore file at playhead |
| `u` | Undo restore |
| `c` | Create checkpoint |
| `x` | Toggle restore edit visibility |
| `s` | Solo track |
| `m` | Mute track |
| `b` | Toggle blast radius in dashboard |
| `w` | Toggle watchdog in dashboard |
| `B` | Toggle blame view |
| `A` | Toggle inline annotations |
| `M` | Create bookmark |
| `'` | Jump to bookmark |
| `z` | Maximize/restore focused panel |
| `Tab` | Cycle panel focus |
| `1-9` | In command view: solo agent. Otherwise: jump to panel by index. |
| `+`/`-`/`=` | Timeline zoom |
| `?` | Help overlay |
| `q` | Quit |
| `Q` | Quit + stop daemon |

### Timeline Mode (`t`)
| Key | Action |
|-----|--------|
| `Left`/`Right` | Pan timeline |
| `Up`/`Down` | Select track |
| `+`/`-` | Zoom |
| `=` | Reset zoom |
| `s` | Solo selected track |
| `m` | Mute selected track |
| `Enter` | Jump to edit under cursor |
| `Esc` | Back to Normal |

### Inspect Mode (`i`)
| Key | Action |
|-----|--------|
| `n` | Next edit |
| `p` | Previous edit |
| `d` | Toggle diff/file |
| `f` | Show full file |
| `c` | Show conversation context |
| `Enter` | Expand/collapse section |
| `Esc` | Back to Normal |

### Search Mode (`/`)
| Key | Action |
|-----|--------|
| typing | Live filter input |
| `Enter` | Lock filter, return to Normal |
| `Tab` | Autocomplete filter field |
| `Up`/`Down` | Cycle through matches |
| `Esc` | Clear filter, return to Normal |

---

## Migration & Backward Compatibility

- All new EditEvent fields are `Option` — existing JSONL files work without modification
- **Repurposed keybindings:**
  - `t` was theme cycle → now enters Timeline mode. Theme cycle via `:theme next` in command palette.
  - `i` was sentinels panel toggle → now enters Inspect mode. Sentinels visible in dashboard; full panel via `:sentinels` in command palette.
  - `b` was blast radius toggle → stays as blast radius toggle (dashboard section expand/collapse)
  - `w` was watchdog toggle → stays as watchdog toggle (dashboard section expand/collapse)
- Config.toml gains new sections (layout, alerts) but existing configs parse fine
- Panel registry wraps existing widget implementations — blast_radius_panel, sentinel_panel, watchdog_panel become registered panels
- No breaking changes to CLI commands, daemon protocol, or MCP server
