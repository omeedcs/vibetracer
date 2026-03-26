# TUI Visual Overhaul Design

**Date:** 2026-03-26
**Goal:** Transform vibetracer's TUI from functional-but-rough to "Final Cut Pro for agent observability" -- every pixel intentional, professional information hierarchy, and polished interaction feedback.
**Approach:** Surgical Polish -- fix every identified issue within the existing widget architecture. No new abstractions or rendering layers.

---

## 1. Layout & Visual Framing

### Borders between zones

Add thin box-drawing separators between all major UI zones:

- Horizontal `─` lines between: status bar / preview, preview / timeline, timeline / keybindings bar
- Vertical `│` between preview pane and sidebar (when sidebar visible)
- All separator characters use `theme.separator` color

### Focus indicator

The focused pane (Preview, Timeline, or Sidebar) gets its adjacent border segments drawn in `theme.accent_warm` instead of `theme.separator`. When the user presses Tab, the bright border moves to the next pane.

### Status bar overflow fix

Measure right-side span width first, then truncate left-side spans to `area.width - right_width - 2` before rendering. Prevents left/right collision on narrow terminals.

### Consistent inner padding

All panel content starts at `x + 1` with 1-char left padding. Currently inconsistent -- some widgets start at `area.x`, others at `area.x + 1`. Standardize everywhere.

### Layout constraint changes

Add 1-line `Constraint::Length(1)` rows for separator lines between zones (3 new rows). Reclaim vertical space by reducing timeline default height from 10 to 8.

**Files affected:** `tui/layout.rs`, `tui/event_loop.rs`

---

## 2. Timeline Overhaul

### Wider track names

Increase `TRACK_NAME_WIDTH` from 14 to 20. Smarter truncation: show `parent/file.rs` when it fits, fall back to basename only when the full relative path exceeds 20 chars. Right-align and pad consistently.

### Replace block characters

Swap the visual vocabulary for timeline bars:

| Element | Old | New | Color |
|---------|-----|-----|-------|
| Empty | `░` (light shade) | `─` (horizontal rule) | `theme.bar_empty` |
| Edit | `█` (full block) | `▌` (left half-block) | agent color or `theme.bar_filled` |
| Conflict | `▓` (dense block) | `!` (exclamation mark, replacing the edit char at that column) | `theme.accent_red` |

The half-block creates visual rhythm with gaps between edits. The horizontal rule reads as a "track lane."

### Active track highlight

The track row containing the current edit's file gets a subtle background tint using `change_tint(theme.accent_warm)`. Makes it immediately clear which file the playhead points at.

### Playhead upgrade

Replace thin `│` with `┃` (heavy vertical) for the playhead column overlay. On the ruler line, use `━` (heavy horizontal) for the 3 chars surrounding the playhead position, with regular `─` elsewhere.

### Stale indicator cleanup

Instead of truncating the track name and appending red "stale" text, show the full track name in `theme.fg_dim` (dimmed) with a small `*` suffix in `theme.accent_red`.

### Time scale tick marks

Add `┬` tick marks at each time label position on the scale bar row, connecting labels visually to the tracks below.

**Files affected:** `tui/widgets/timeline.rs`

---

## 3. Preview Pane Polish

### Merged header/footer

Replace the separate header and footer with a single structured header row:

- File mode: `│ filename.rs │ 387 lines │ 35% │`
- Diff mode: `│ filename.rs │ diff │ +12 -3 │`

Uses `theme.separator` for delimiters, `theme.fg` for filename. Removing the footer reclaims one row of vertical space.

### Scrollbar

Single-column scrollbar on the right edge:

- Track: `│` in `theme.bar_empty`
- Thumb: `┃` in `theme.accent_warm`
- Thumb size proportional to `visible_lines / total_lines`
- Only renders when content overflows

### Diff view line numbers

Add gutter to diff view matching file view. Show old line number on `-` lines, new line number on `+` lines. Context lines show the new line number. Same `GUTTER_WIDTH` and formatting for visual consistency.

### Changed line tint increase

Increase tint intensity from `40/255` to `55/255` in `change_tint()`. Also apply the tint color to the gutter background for changed lines (currently only the gutter text color changes).

### Mouse scroll in preview

When mouse scroll events occur over the preview pane area, scroll the preview content instead of zooming the timeline. Requires storing the last computed `AppLayout` in `App` state so the mouse handler can detect which pane the cursor is over.

**Files affected:** `tui/widgets/preview.rs`, `tui/widgets/file_view.rs`, `tui/app.rs`, `tui/event_loop.rs`

---

## 4. Theme Consistency

Eliminate all hardcoded color constants. After this change, every rendered pixel flows through the `Theme` struct, and pressing `t` transforms the entire UI uniformly.

### help_overlay.rs

Remove 5 `const Color` values. Change `HelpOverlay` to accept `&Theme`. Mapping:

- `COLOR_KEY` -> `theme.fg_muted`
- `COLOR_DESC` -> `theme.fg`
- `COLOR_BG` -> `theme.bg`
- `COLOR_BORDER` -> `theme.separator`
- `COLOR_SECTION` -> `theme.accent_warm`

### blast_radius_panel.rs

Remove 6 `const Color` values. Accept `&Theme`. Mapping:

- `COLOR_HEADER` -> `theme.accent_warm`
- `COLOR_DEFAULT` -> `theme.fg`
- `COLOR_DIM` -> `theme.fg_dim`
- `COLOR_STALE` -> `theme.accent_red`
- `COLOR_UPDATED` -> `theme.accent_green`
- `COLOR_SEPARATOR` -> `theme.separator`

### sentinel_panel.rs

Remove 5 `const Color` values. Accept `&Theme`. Mapping:

- `COLOR_HEADER` -> `theme.accent_red`
- `COLOR_DEFAULT` -> `theme.fg`
- `COLOR_DIM` -> `theme.fg_dim`
- `COLOR_SEPARATOR` -> `theme.separator`
- `COLOR_LABEL` -> `theme.accent_warm`

### watchdog_panel.rs

Remove 6 `const Color` values. Accept `&Theme`. Mapping:

- `COLOR_HEADER` -> `theme.accent_warm`
- `COLOR_DEFAULT` -> `theme.fg`
- `COLOR_DIM` -> `theme.fg_dim`
- `COLOR_SEPARATOR` -> `theme.separator`
- `COLOR_LABEL` -> `theme.accent_warm`
- `COLOR_CRITICAL` -> `theme.accent_red`

### event_loop.rs

- Keybindings bar: replace hardcoded `COLOR_MUTED` and `Color::Rgb(138, 143, 152)` with `theme.fg_muted` and `theme.fg`
- "Terminal too small" message: replace hardcoded red with `theme.accent_red`

**Files affected:** `tui/widgets/help_overlay.rs`, `tui/widgets/blast_radius_panel.rs`, `tui/widgets/sentinel_panel.rs`, `tui/widgets/watchdog_panel.rs`, `tui/event_loop.rs`

---

## 5. Interaction Feedback

### Toast notification system

Generalize the existing `theme_flash` mechanism. Add to `App` state:

```rust
toast_message: Option<String>,
toast_style: ToastStyle,  // enum { Info, Success, Warning }
toast_time: Option<Instant>,
```

Display in the status bar right side (before connection status), auto-dismiss after 2 seconds. Color by style:

- `Info` -> `theme.fg`
- `Success` -> `theme.accent_green`
- `Warning` -> `theme.accent_red`

Fire toasts on:

- Restore: "restored {filename}" (Success)
- Undo restore: "restore undone" (Info)
- Checkpoint: "checkpoint #{n}" (Success)
- Toggle restore edits: "restore edits: visible" / "restore edits: hidden" (Info)

### Playback state flash

When toggling between Live/Paused/Playing, flash the playback label in `theme.accent_warm` for 500ms using an `Instant`-based timer, then return to normal color.

### Active track flash on scrub

When the playhead moves via arrow keys and lands on a different file, flash the track name of the new file in `theme.accent_warm` for 300ms.

### Mouse-aware scroll routing

Store last `AppLayout` in `App` state (updated each frame). In the mouse scroll handler, check mouse position against layout rects:

- Preview area: scroll preview content
- Timeline area: zoom timeline (existing)
- Sidebar area: no-op

**Files affected:** `tui/app.rs`, `tui/widgets/status_bar.rs`, `tui/event_loop.rs`, `tui/widgets/timeline.rs`

---

## 6. Remaining Polish

### Empty state refresh

Replace the 5-line ASCII art logo with a smaller 3-line version. Use `theme.fg_dim` instead of `theme.accent_warm`. Move hint text closer to logo.

### Keybindings bar grouping

Visual grouping using spacing: `b blast | i sentinel | w watchdog   t theme | g commands   ? help`. Groups separated by double-space, items within groups separated by ` | ` in `theme.separator`.

### Implement SoloAgent (1-9 keys)

Currently `Action::SoloAgent(n)` is a no-op. Implementation:

1. Collect unique agent IDs from edits, sorted by first appearance
2. Key `n` maps to the nth agent
3. Filter `visible_tracks()` to only show edits from that agent
4. Toggle: pressing same number un-solos
5. Store as `solo_agent: Option<String>` on App state

### Consistent separator character

Replace ` | ` (ASCII pipe with spaces) in the status bar with ` │ ` (box-drawing light vertical) using `theme.separator` color. Matches the border vocabulary used elsewhere.

### Sidebar inner padding

All three sidebar panels: add 1-char left padding (`area.x + 1`) and 1-char right margin (reduce available width by 2). Matches preview pane padding.

**Files affected:** `tui/widgets/preview.rs` (empty state), `tui/event_loop.rs` (keybindings bar), `tui/input.rs` (SoloAgent), `tui/app.rs` (solo_agent field), `tui/widgets/status_bar.rs` (separator chars), `tui/widgets/blast_radius_panel.rs`, `tui/widgets/sentinel_panel.rs`, `tui/widgets/watchdog_panel.rs` (padding)

---

## Files Summary

| File | Changes |
|------|---------|
| `tui/layout.rs` | Add separator constraints, reduce timeline height |
| `tui/app.rs` | Add toast state, solo_agent, store AppLayout, ToastStyle enum |
| `tui/event_loop.rs` | Render separators, themed keybindings bar, mouse routing, toast display |
| `tui/input.rs` | Implement SoloAgent action |
| `tui/widgets/timeline.rs` | Track name width, bar chars, active highlight, playhead, stale indicator, tick marks |
| `tui/widgets/status_bar.rs` | Overflow fix, toast rendering, separator chars, playback flash |
| `tui/widgets/preview.rs` | Merged header, diff line numbers, empty state |
| `tui/widgets/file_view.rs` | Scrollbar, tint increase, gutter tint |
| `tui/widgets/help_overlay.rs` | Accept Theme, remove hardcoded colors |
| `tui/widgets/blast_radius_panel.rs` | Accept Theme, remove hardcoded colors, inner padding |
| `tui/widgets/sentinel_panel.rs` | Accept Theme, remove hardcoded colors, inner padding |
| `tui/widgets/watchdog_panel.rs` | Accept Theme, remove hardcoded colors, inner padding |
| `theme.rs` | No changes needed -- existing Theme struct covers all semantic colors |

## Non-Goals

- No new TUI framework or abstraction layer
- No animated transitions beyond timed flashes
- No mouse click handling (only scroll)
- No new theme colors added to Theme struct
- No changes to data flow, analysis engines, or daemon communication
