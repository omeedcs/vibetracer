# TUI Visual Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform vibetracer's TUI from functional-but-rough to "Final Cut Pro for agent observability" -- every pixel intentional, professional information hierarchy, polished interaction feedback.

**Architecture:** Surgical polish of every existing widget. No new abstractions -- fix 30+ visual issues across 12 files within the existing ratatui widget architecture. Each task targets one component and produces a compilable, testable state.

**Tech Stack:** Rust, ratatui, crossterm

**Spec:** `docs/superpowers/specs/2026-03-26-tui-visual-overhaul-design.md`

---

### Task 1: Add Toast State and Layout Storage to App

**Files:**
- Modify: `src/tui/app.rs:1-107` (struct fields and new())
- Modify: `src/tui/layout.rs` (make AppLayout Clone-able, add separator rects)

This task adds the new fields that later tasks depend on. No visual changes yet.

- [ ] **Step 1: Add ToastStyle enum and new fields to App**

In `src/tui/app.rs`, add the `ToastStyle` enum after the `PreviewMode` enum (after line 52), and add new fields to the `App` struct:

```rust
/// Style for toast notifications displayed in the status bar.
#[derive(Debug, Clone, PartialEq)]
pub enum ToastStyle {
    Info,
    Success,
    Warning,
}
```

Add these fields to the `App` struct (after line 106, before the closing `}`):

```rust
    /// Toast notification message (displayed in status bar, auto-dismissed).
    pub toast_message: Option<String>,
    /// Toast notification style (determines color).
    pub toast_style: ToastStyle,
    /// When the toast was triggered (for auto-dismiss timing).
    pub toast_time: Option<std::time::Instant>,
    /// Agent solo filter: only show edits from this agent_id.
    pub solo_agent: Option<String>,
    /// Last computed layout (for mouse-aware scroll routing).
    pub last_layout: Option<AppLayout>,
    /// Flash timer for active track highlight on scrub.
    pub track_flash: Option<(String, std::time::Instant)>,
    /// Flash timer for playback state change.
    pub playback_flash: Option<std::time::Instant>,
```

- [ ] **Step 2: Initialize new fields in App::new()**

In `App::new()`, add these initializations after the `timeline_scroll: 0` line (line 149):

```rust
            toast_message: None,
            toast_style: ToastStyle::Info,
            toast_time: None,
            solo_agent: None,
            last_layout: None,
            track_flash: None,
            playback_flash: None,
```

- [ ] **Step 3: Add a `show_toast` helper method**

Add this method to the `impl App` block (after `changed_lines_from_patch`, before the closing `}`):

```rust
    /// Display a toast notification in the status bar for 2 seconds.
    pub fn show_toast(&mut self, message: String, style: ToastStyle) {
        self.toast_message = Some(message);
        self.toast_style = style;
        self.toast_time = Some(std::time::Instant::now());
    }

    /// Check if the toast is still active (within 2 seconds).
    pub fn toast_active(&self) -> bool {
        self.toast_time
            .map(|t| t.elapsed().as_secs() < 2)
            .unwrap_or(false)
    }
```

- [ ] **Step 4: Add `use` for AppLayout in app.rs**

At the top of `src/tui/app.rs`, add the import:

```rust
use crate::tui::layout::AppLayout;
```

- [ ] **Step 5: Update AppLayout to derive Clone**

In `src/tui/layout.rs`, add `#[derive(Clone)]` above the `AppLayout` struct:

```rust
#[derive(Clone)]
pub struct AppLayout {
    pub status_bar: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub timeline: Rect,
    pub keybindings: Rect,
}
```

- [ ] **Step 6: Add separator rects to AppLayout**

Add three new fields to `AppLayout`:

```rust
#[derive(Clone)]
pub struct AppLayout {
    pub status_bar: Rect,
    pub sep_after_status: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub sep_after_main: Rect,
    pub timeline: Rect,
    pub sep_after_timeline: Rect,
    pub keybindings: Rect,
}
```

- [ ] **Step 7: Update compute_layout to produce separators**

Replace the entire `compute_layout` function body:

```rust
pub fn compute_layout(area: Rect, sidebar_visible: bool) -> AppLayout {
    let timeline_height = if area.height < 15 {
        4
    } else if area.height < 25 {
        6
    } else {
        8
    };

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),               // status bar
            Constraint::Length(1),               // separator
            Constraint::Min(3),                  // main area (minimum 3 rows)
            Constraint::Length(1),               // separator
            Constraint::Length(timeline_height), // timeline
            Constraint::Length(1),               // separator
            Constraint::Length(1),               // keybindings
        ])
        .split(area);

    let status_bar = vertical[0];
    let sep_after_status = vertical[1];
    let main_area = vertical[2];
    let sep_after_main = vertical[3];
    let timeline = vertical[4];
    let sep_after_timeline = vertical[5];
    let keybindings = vertical[6];

    let (preview, sidebar) = if sidebar_visible {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main_area);
        (horizontal[0], Some(horizontal[1]))
    } else {
        (main_area, None)
    };

    AppLayout {
        status_bar,
        sep_after_status,
        main_area,
        preview,
        sidebar,
        sep_after_main,
        timeline,
        sep_after_timeline,
        keybindings,
    }
}
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo check 2>&1 | head -30`
Expected: Compilation errors in `event_loop.rs` referencing old `vertical[N]` indices -- this is expected, we fix it in Task 2.

- [ ] **Step 9: Commit**

```bash
git add src/tui/app.rs src/tui/layout.rs
git commit -m "feat(tui): add toast state, layout storage, and separator rects to App"
```

---

### Task 2: Render Separators and Store Layout in Event Loop

**Files:**
- Modify: `src/tui/event_loop.rs`

This task renders the new separator lines and stores the layout for mouse routing.

- [ ] **Step 1: Add a separator rendering helper**

At the top of `src/tui/event_loop.rs`, after the `BgFill` struct (after line 39), add:

```rust
/// Render a horizontal separator line filling the given area with `─`.
struct HorizontalSep {
    color: Color,
    focused: bool,
    focus_color: Color,
}
impl Widget for HorizontalSep {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let color = if self.focused { self.focus_color } else { self.color };
        let line = "─".repeat(area.width as usize);
        buf.set_string(area.x, area.y, &line, Style::default().fg(color));
    }
}
```

- [ ] **Step 2: Render the three separator lines in the draw closure**

In the `terminal.draw(|frame| { ... })` closure, after the `BgFill` render and the `compute_layout` call, add separator rendering. Insert after `let lo = layout::compute_layout(area, app.sidebar_visible);` (around line 115):

```rust
            // Store layout for mouse routing.
            app.last_layout = Some(lo.clone());

            // Determine which pane is focused for border highlighting.
            let focus_color = app.theme.accent_warm;
            let sep_color = app.theme.separator;

            // Render horizontal separators between zones.
            HorizontalSep {
                color: sep_color,
                focused: app.focused_pane == crate::tui::Pane::Preview,
                focus_color,
            }
            .render(lo.sep_after_status, buf);

            HorizontalSep {
                color: sep_color,
                focused: app.focused_pane == crate::tui::Pane::Timeline,
                focus_color,
            }
            .render(lo.sep_after_main, buf);

            HorizontalSep {
                color: sep_color,
                focused: false,
                focus_color,
            }
            .render(lo.sep_after_timeline, buf);
```

- [ ] **Step 3: Add vertical separator between preview and sidebar**

After the sidebar panel rendering block (after the `SidebarPanel::Watchdog` match arm closes), add:

```rust
            // Vertical separator between preview and sidebar.
            if let Some(sidebar_rect) = lo.sidebar {
                let sep_x = sidebar_rect.x.saturating_sub(1);
                let focused = app.focused_pane == crate::tui::Pane::Sidebar;
                let color = if focused { focus_color } else { sep_color };
                for y in lo.main_area.y..lo.main_area.y + lo.main_area.height {
                    if sep_x >= lo.main_area.x && sep_x < lo.main_area.x + lo.main_area.width {
                        buf.set_string(sep_x, y, "│", Style::default().fg(color));
                    }
                }
            }
```

- [ ] **Step 4: Verify it compiles and runs**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation (or warnings only).

- [ ] **Step 5: Commit**

```bash
git add src/tui/event_loop.rs
git commit -m "feat(tui): render separator lines between zones with focus highlighting"
```

---

### Task 3: Theme Consistency -- Eliminate Hardcoded Colors

**Files:**
- Modify: `src/tui/widgets/help_overlay.rs`
- Modify: `src/tui/widgets/blast_radius_panel.rs`
- Modify: `src/tui/widgets/sentinel_panel.rs`
- Modify: `src/tui/widgets/watchdog_panel.rs`
- Modify: `src/tui/event_loop.rs` (keybindings bar + "terminal too small")

This task eliminates all hardcoded color constants. After this, every pixel respects theme cycling.

- [ ] **Step 1: Refactor help_overlay.rs to accept Theme**

Replace the entire `src/tui/widgets/help_overlay.rs` with:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

use crate::theme::Theme;

const MAX_WIDTH: u16 = 52;
const MAX_HEIGHT: u16 = 30;

const KEYBINDINGS: &[(&str, &str)] = &[
    ("", "-- playback --"),
    ("space", "toggle play / pause"),
    ("left / right", "scrub timeline"),
    ("shift+left", "scrub file backward"),
    ("shift+right", "scrub file forward"),
    ("a", "reattach file to global"),
    ("", "-- actions --"),
    ("R", "restore file to playhead"),
    ("u", "undo last restore"),
    ("c", "create checkpoint"),
    ("x", "toggle restore edits"),
    ("", "-- view --"),
    ("g", "toggle command view"),
    ("s", "solo track"),
    ("m", "mute track"),
    ("1-9", "solo agent N"),
    ("t", "cycle theme"),
    ("", "-- panels --"),
    ("b", "toggle blast radius"),
    ("i", "toggle sentinels"),
    ("w", "toggle watchdog"),
    ("tab", "cycle focus"),
    ("", "-- meta --"),
    ("q", "quit"),
    ("Q", "quit and stop daemon"),
    ("?", "show this help"),
];

pub struct HelpOverlay<'a> {
    pub theme: &'a Theme,
}

impl<'a> HelpOverlay<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for HelpOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = self.theme;

        let width = MAX_WIDTH.min(area.width);
        let height = MAX_HEIGHT.min(area.height);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;

        let overlay_area = Rect { x, y, width, height };

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .title(" keybindings ")
            .borders(Borders::ALL)
            .style(Style::default().bg(t.bg).fg(t.separator));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let key_col_width: u16 = 18;
        let gap: u16 = 2;

        for (i, (key, desc)) in KEYBINDINGS.iter().enumerate() {
            let row_y = inner.y + i as u16;
            if row_y >= inner.y + inner.height {
                break;
            }

            let row_area = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };

            if key.is_empty() {
                let line = Line::from(vec![
                    Span::raw(format!(
                        "{:width$}",
                        "",
                        width = (key_col_width + gap) as usize
                    )),
                    Span::styled(*desc, Style::default().fg(t.accent_warm)),
                ]);
                line.render(row_area, buf);
            } else {
                let key_str = format!("{:>width$}", key, width = key_col_width as usize);

                let line = Line::from(vec![
                    Span::styled(key_str, Style::default().fg(t.fg_muted)),
                    Span::raw(format!("{:width$}", "", width = gap as usize)),
                    Span::styled(*desc, Style::default().fg(t.fg)),
                ]);

                line.render(row_area, buf);
            }
        }
    }
}
```

- [ ] **Step 2: Update help overlay call site in event_loop.rs**

In `src/tui/event_loop.rs`, change the help overlay render call from:

```rust
                widgets::help_overlay::HelpOverlay.render(area, buf);
```

to:

```rust
                widgets::help_overlay::HelpOverlay::new(&app.theme).render(area, buf);
```

- [ ] **Step 3: Refactor blast_radius_panel.rs to accept Theme**

Replace the color constants and struct definition in `src/tui/widgets/blast_radius_panel.rs`. Remove the 6 `const COLOR_*` lines (lines 11-16). Change the struct to hold a `&Theme`:

```rust
use crate::analysis::blast_radius::DependencyStatus;
use crate::theme::Theme;
```

Change the struct:

```rust
pub struct BlastRadiusPanel<'a> {
    pub source_file: &'a str,
    pub status: &'a DependencyStatus,
    pub theme: &'a Theme,
}

impl<'a> BlastRadiusPanel<'a> {
    pub fn new(source_file: &'a str, status: &'a DependencyStatus, theme: &'a Theme) -> Self {
        Self {
            source_file,
            status,
            theme,
        }
    }
}
```

In the `render` method, replace all `COLOR_*` references:
- `COLOR_HEADER` -> `self.theme.accent_warm`
- `COLOR_DEFAULT` -> `self.theme.fg`
- `COLOR_DIM` -> `self.theme.fg_dim`
- `COLOR_STALE` -> `self.theme.accent_red`
- `COLOR_UPDATED` -> `self.theme.accent_green`
- `COLOR_SEPARATOR` -> `self.theme.separator`

Also add 1-char inner padding: change all `area.x` references for content rendering to `area.x + 1` and reduce width calculations by 2.

- [ ] **Step 4: Update blast_radius_panel call site in event_loop.rs**

Change:
```rust
                        widgets::blast_radius_panel::BlastRadiusPanel::new(source, status)
                            .render(sidebar_rect, buf);
```
to:
```rust
                        widgets::blast_radius_panel::BlastRadiusPanel::new(source, status, &app.theme)
                            .render(sidebar_rect, buf);
```

- [ ] **Step 5: Refactor sentinel_panel.rs to accept Theme**

Same pattern as blast_radius_panel. Remove the 5 `const COLOR_*` lines (lines 11-15). Add `use crate::theme::Theme;`. Change struct to hold `&'a Theme`. Replace in render:
- `COLOR_HEADER` -> `self.theme.accent_red`
- `COLOR_DEFAULT` -> `self.theme.fg`
- `COLOR_DIM` -> `self.theme.fg_dim`
- `COLOR_SEPARATOR` -> `self.theme.separator`
- `COLOR_LABEL` -> `self.theme.accent_warm`

Add 1-char inner padding (same as blast_radius_panel).

- [ ] **Step 6: Update sentinel_panel call site in event_loop.rs**

Change:
```rust
                        widgets::sentinel_panel::SentinelPanel::new(&app.sentinel_violations)
                            .render(sidebar_rect, buf);
```
to:
```rust
                        widgets::sentinel_panel::SentinelPanel::new(&app.sentinel_violations, &app.theme)
                            .render(sidebar_rect, buf);
```

- [ ] **Step 7: Refactor watchdog_panel.rs to accept Theme**

Same pattern. Remove the 6 `const COLOR_*` lines (lines 11-16). Add `use crate::theme::Theme;`. Change struct to hold `&'a Theme`. Replace in render:
- `COLOR_HEADER` -> `self.theme.accent_warm`
- `COLOR_DEFAULT` -> `self.theme.fg`
- `COLOR_DIM` -> `self.theme.fg_dim`
- `COLOR_SEPARATOR` -> `self.theme.separator`
- `COLOR_LABEL` -> `self.theme.accent_warm`
- `COLOR_CRITICAL` -> `self.theme.accent_red`

Add 1-char inner padding.

- [ ] **Step 8: Update watchdog_panel call site in event_loop.rs**

Change:
```rust
                        widgets::watchdog_panel::WatchdogPanel::new(&app.watchdog_alerts)
                            .render(sidebar_rect, buf);
```
to:
```rust
                        widgets::watchdog_panel::WatchdogPanel::new(&app.watchdog_alerts, &app.theme)
                            .render(sidebar_rect, buf);
```

- [ ] **Step 9: Theme the keybindings bar in event_loop.rs**

Replace the hardcoded keybindings bar (lines 164-183 in event_loop.rs). Remove the `const COLOR_MUTED` at the top of the file. Replace the keybindings bar with:

```rust
            let kb_sep = Span::styled(" │ ", Style::default().fg(app.theme.separator));
            let kb_gap = Span::styled("  ", Style::default());
            let kb_key = |k: &str| Span::styled(k, Style::default().fg(app.theme.fg));
            let kb_desc = |d: &str| Span::styled(d, Style::default().fg(app.theme.fg_muted));

            let kb_line = Line::from(vec![
                Span::styled(" ", Style::default()),
                kb_key("b"), kb_desc(" blast"), kb_sep.clone(),
                kb_key("i"), kb_desc(" sentinel"), kb_sep.clone(),
                kb_key("w"), kb_desc(" watchdog"),
                kb_gap.clone(),
                kb_key("t"), kb_desc(" theme"), kb_sep.clone(),
                kb_key("g"), kb_desc(" commands"),
                kb_gap.clone(),
                kb_key("?"), kb_desc(" help"),
            ]);
            kb_line.render(lo.keybindings, buf);
```

- [ ] **Step 10: Theme the "terminal too small" message**

In `event_loop.rs`, change the hardcoded red in the "terminal too small" section:

```rust
                buf.set_string(x, y, msg, Style::default().fg(app.theme.accent_red));
```

- [ ] **Step 11: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 12: Run existing tests**

Run: `cargo test --lib 2>&1 | tail -5`
Expected: All 168 tests pass (no TUI widget tests exist yet, but theme tests should still pass).

- [ ] **Step 13: Commit**

```bash
git add src/tui/widgets/help_overlay.rs src/tui/widgets/blast_radius_panel.rs src/tui/widgets/sentinel_panel.rs src/tui/widgets/watchdog_panel.rs src/tui/event_loop.rs
git commit -m "fix(tui): eliminate all hardcoded colors, pipe everything through Theme"
```

---

### Task 4: Timeline Visual Overhaul

**Files:**
- Modify: `src/tui/widgets/timeline.rs`

- [ ] **Step 1: Update TRACK_NAME_WIDTH and display_name**

Change the constant and rewrite `display_name`:

```rust
const TRACK_NAME_WIDTH: usize = 20;
```

Replace the `display_name` method:

```rust
    fn display_name(filename: &str) -> String {
        if filename.len() <= TRACK_NAME_WIDTH {
            return format!("{:>width$}", filename, width = TRACK_NAME_WIDTH);
        }
        // Try parent/basename.
        let path = std::path::Path::new(filename);
        let base = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);
        let parent = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());

        let with_parent = if let Some(p) = parent {
            format!("{}/{}", p, base)
        } else {
            base.to_string()
        };

        if with_parent.len() <= TRACK_NAME_WIDTH {
            format!("{:>width$}", with_parent, width = TRACK_NAME_WIDTH)
        } else if base.len() <= TRACK_NAME_WIDTH {
            format!("{:>width$}", base, width = TRACK_NAME_WIDTH)
        } else {
            format!("{:>width$.width$}", base, width = TRACK_NAME_WIDTH)
        }
    }
```

- [ ] **Step 2: Replace block characters in track bar rendering**

In the `render` method, change the bar initialization (line 247):

From:
```rust
            let mut bar: Vec<(char, Color)> = vec![('\u{2591}', color_bar_empty); bar_width];
```
To:
```rust
            let mut bar: Vec<(char, Color)> = vec![('\u{2500}', color_bar_empty); bar_width];
```

Change the edit block character (around line 257). Replace:
```rust
                        bar[col] = ('\u{2593}', t.accent_red);
                    } else {
                        bar[col] = ('\u{2588}', agent_col.unwrap_or(color_bar_edit));
```
With:
```rust
                        bar[col] = ('!', t.accent_red);
                    } else {
                        bar[col] = ('\u{258C}', agent_col.unwrap_or(color_bar_edit));
```

- [ ] **Step 3: Add active track highlight**

After building the bar spans but before rendering the Line (around line 267), add background tint for the active track:

```rust
            // Determine if this is the active track (contains current edit's file).
            let is_active = self.app.current_edit()
                .map(|e| e.file == track.filename)
                .unwrap_or(false);

            // Check for track flash (brief highlight when scrubbing to a new file).
            let is_flashing = self.app.track_flash
                .as_ref()
                .map(|(f, t)| f == &track.filename && t.elapsed().as_millis() < 300)
                .unwrap_or(false);

            let row_bg = if is_active || is_flashing {
                Some(change_tint(t.accent_warm))
            } else {
                None
            };
```

Then when building spans, apply the background. Change the name span to include bg:

```rust
            let name_style = if let Some(bg) = row_bg {
                Style::default().fg(name_color).bg(bg)
            } else {
                Style::default().fg(name_color)
            };

            let mut spans: Vec<Span> = vec![
                Span::styled(
                    format!("{:<width$}", name_text, width = TRACK_NAME_WIDTH),
                    name_style,
                ),
                Span::styled(SEPARATOR, if let Some(bg) = row_bg {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                }),
            ];
```

And for bar spans:

```rust
            for &(ch, color) in &bar {
                let style = if let Some(bg) = row_bg {
                    Style::default().fg(color).bg(bg)
                } else {
                    Style::default().fg(color)
                };
                spans.push(Span::styled(ch.to_string(), style));
            }
```

- [ ] **Step 4: Add change_tint function to timeline.rs**

Add this at the bottom of the file (before the closing of the module):

```rust
/// Produce a muted tint color for background highlights.
fn change_tint(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16) * 40 / 255) as u8,
            ((g as u16) * 40 / 255) as u8,
            ((b as u16) * 40 / 255) as u8,
        ),
        _ => Color::Rgb(10, 8, 6),
    }
}
```

- [ ] **Step 5: Update stale indicator**

In the track name rendering section, replace the stale branch:

From:
```rust
            let (name_text, name_color) = if track.stale {
                let display = Self::display_name(&track.filename);
                let truncated: String = display.chars().take(TRACK_NAME_WIDTH.min(8)).collect();
                (format!("{} stale", truncated), color_track_stale)
```
To:
```rust
            let (name_text, name_color) = if track.stale {
                let display = Self::display_name(&track.filename);
                let trimmed = display.trim_start();
                let padded = format!("{:>width$}", format!("{}*", trimmed), width = TRACK_NAME_WIDTH);
                (padded, t.fg_dim)
```

- [ ] **Step 6: Upgrade playhead character**

Change the playhead overlay character. In the full-height playhead overlay section (around line 301):

From:
```rust
                    buf.set_string(ph_x, track_row, "\u{2502}", Style::default().fg(ph_color));
```
To:
```rust
                    buf.set_string(ph_x, track_row, "\u{2503}", Style::default().fg(ph_color));
```

In the playhead ruler row (around line 314-319), change the ruler characters:

From:
```rust
                let ch = if cell == playhead_col {
                    "\u{2502}"
                } else {
                    "\u{2500}"
                };
```
To:
```rust
                let ch = if cell == playhead_col {
                    "\u{2503}"
                } else if cell >= playhead_col.saturating_sub(1) && cell <= playhead_col + 1 {
                    "\u{2501}"
                } else {
                    "\u{2500}"
                };
```

- [ ] **Step 7: Add tick marks to time scale**

In the time scale bar section (around line 203-207), after writing each time label, also write a `┬` tick at the start of the label:

Change the character writing block. After the `for (i, ch) in label.chars().enumerate()` loop, add a tick mark at the column position:

```rust
                // Write tick mark at the label position.
                if write_x < area.x + area.width {
                    buf.set_string(write_x, row, "\u{252C}", Style::default().fg(t.fg_dim));
                }
```

Wait -- actually, the tick mark should be written first, then the label overwrites from that position. Better: write the tick mark as the first character of the label area. Change the label rendering to prefix with the tick:

Replace the label character writing loop:

```rust
                let prefixed = format!("\u{252C}{}", label);
                for (i, ch) in prefixed.chars().enumerate() {
                    let cx = write_x + i as u16;
                    if cx < area.x + area.width {
                        buf.set_string(cx, row, ch.to_string(), Style::default().fg(t.fg_dim));
                    }
                }
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 9: Commit**

```bash
git add src/tui/widgets/timeline.rs
git commit -m "feat(tui): overhaul timeline visuals -- wider names, half-blocks, active highlight, heavy playhead"
```

---

### Task 5: Status Bar Polish

**Files:**
- Modify: `src/tui/widgets/status_bar.rs`

- [ ] **Step 1: Replace pipe separators with box-drawing characters**

In `src/tui/widgets/status_bar.rs`, change the separator span (line 67):

From:
```rust
        let sep = Span::styled(" | ", Style::default().fg(color_separator));
```
To:
```rust
        let sep = Span::styled(" \u{2502} ", Style::default().fg(color_separator));
```

- [ ] **Step 2: Add toast rendering to right side**

In the right-side span building (around line 145-153), add toast rendering before the theme flash. Insert before the `if self.theme_flash_active()` block:

```rust
        // Toast notification (if active)
        if self.app.toast_active() {
            if let Some(ref msg) = self.app.toast_message {
                let toast_color = match self.app.toast_style {
                    crate::tui::app::ToastStyle::Info => t.fg,
                    crate::tui::app::ToastStyle::Success => t.accent_green,
                    crate::tui::app::ToastStyle::Warning => t.accent_red,
                };
                right_spans.push(Span::styled(msg.clone(), Style::default().fg(toast_color)));
                right_spans.push(sep.clone());
            }
        }
```

- [ ] **Step 3: Add playback flash**

Change the playback state rendering (around line 159-169). Replace:

```rust
        match &self.app.playback {
            PlaybackState::Playing { speed } => {
                right_spans.push(Span::styled(
                    format!("{speed}x"),
                    Style::default().fg(color_speed),
                ));
            }
            _ => {
                right_spans.push(Span::styled(pb_text, Style::default().fg(pb_color)));
            }
        }
```

With:

```rust
        let pb_flashing = self.app.playback_flash
            .map(|t| t.elapsed().as_millis() < 500)
            .unwrap_or(false);

        match &self.app.playback {
            PlaybackState::Playing { speed } => {
                let color = if pb_flashing { color_accent } else { color_speed };
                right_spans.push(Span::styled(
                    format!("{speed}x"),
                    Style::default().fg(color),
                ));
            }
            _ => {
                let color = if pb_flashing { color_accent } else { pb_color };
                right_spans.push(Span::styled(pb_text, Style::default().fg(color)));
            }
        }
```

- [ ] **Step 4: Fix status bar overflow**

In the render method, before rendering the left line (around line 172-183), add overflow protection:

```rust
        // Truncate left side if it would overlap with right side.
        let left_width: u16 = left_line.spans.iter().map(|s| s.content.len() as u16).sum();
        let available = area.width.saturating_sub(right_width + 2);

        let left_line = if left_width > available {
            // Rebuild with truncated spans
            let mut total: u16 = 0;
            let mut truncated_spans = Vec::new();
            for span in left_line.spans {
                let span_len = span.content.len() as u16;
                if total + span_len > available {
                    let remaining = available.saturating_sub(total) as usize;
                    if remaining > 0 {
                        let truncated: String = span.content.chars().take(remaining).collect();
                        truncated_spans.push(Span::styled(truncated, span.style));
                    }
                    break;
                }
                total += span_len;
                truncated_spans.push(span);
            }
            Line::from(truncated_spans)
        } else {
            left_line
        };
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 6: Commit**

```bash
git add src/tui/widgets/status_bar.rs
git commit -m "feat(tui): polish status bar -- box-drawing separators, toast, playback flash, overflow fix"
```

---

### Task 6: Preview Pane -- Merged Header, Scrollbar, Tint

**Files:**
- Modify: `src/tui/widgets/file_view.rs`
- Modify: `src/tui/widgets/preview.rs`

- [ ] **Step 1: Increase change_tint intensity**

In `src/tui/widgets/file_view.rs`, change the `change_tint` function (line 243-252):

From:
```rust
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16) * 40 / 255) as u8,
            ((g as u16) * 40 / 255) as u8,
            ((b as u16) * 40 / 255) as u8,
        ),
        _ => Color::Rgb(6, 24, 7),
```
To:
```rust
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16) * 55 / 255) as u8,
            ((g as u16) * 55 / 255) as u8,
            ((b as u16) * 55 / 255) as u8,
        ),
        _ => Color::Rgb(8, 33, 10),
```

- [ ] **Step 2: Apply tint to gutter background for changed lines**

In the gutter span rendering (around line 134-140), add background tint:

From:
```rust
            let gutter_color = if is_changed {
                theme.accent_green
            } else {
                theme.fg_dim
            };
            let gutter_text = format!("{:>width$} ", line_num, width = (GUTTER_WIDTH - 1) as usize);
            let gutter_span = Span::styled(gutter_text, Style::default().fg(gutter_color));
```
To:
```rust
            let gutter_color = if is_changed {
                theme.accent_green
            } else {
                theme.fg_dim
            };
            let gutter_bg = if is_changed {
                change_tint(theme.accent_green)
            } else {
                Color::Reset
            };
            let gutter_text = format!("{:>width$} ", line_num, width = (GUTTER_WIDTH - 1) as usize);
            let gutter_span = Span::styled(gutter_text, Style::default().fg(gutter_color).bg(gutter_bg));
```

- [ ] **Step 3: Merge header and footer into a single header row**

In the FileView `render` method, replace the header rendering (lines 86-98):

From:
```rust
        let header_line = Line::from(vec![Span::styled(
            format!(" {}", self.filename),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )]);
```
To:
```rust
        let scroll_pct = if total_lines <= body_height {
            100
        } else {
            let max_scroll = total_lines.saturating_sub(body_height);
            if max_scroll == 0 { 100 } else { (scroll.min(max_scroll) * 100) / max_scroll }
        };

        let header_line = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(self.filename, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502} ", Style::default().fg(theme.separator)),
            Span::styled(format!("{} lines", total_lines), Style::default().fg(theme.fg_muted)),
            Span::styled(" \u{2502} ", Style::default().fg(theme.separator)),
            Span::styled(format!("{}%", scroll_pct), Style::default().fg(theme.fg_muted)),
        ]);
```

- [ ] **Step 4: Remove the separate footer**

Change `footer_rows` from 1 to 0:

```rust
        let footer_rows: u16 = 0;
```

Remove the entire footer rendering block (lines 189-213 approximately -- everything from `// -- Footer --` to the end of the render method before the final `}`). Keep only the closing `}` of the render method.

- [ ] **Step 5: Add scrollbar to file view**

After the body line rendering loop (after the `for row_idx in 0..body_height` loop), add scrollbar rendering:

```rust
        // -- Scrollbar (right edge) --
        if total_lines > body_height {
            let scrollbar_x = area.x + area.width - 1;
            let scrollbar_height = body_height;
            let thumb_size = ((body_height as f64 / total_lines as f64) * scrollbar_height as f64)
                .max(1.0) as usize;
            let max_scroll = total_lines.saturating_sub(body_height);
            let thumb_pos = if max_scroll == 0 {
                0
            } else {
                (scroll.min(max_scroll) * scrollbar_height.saturating_sub(thumb_size)) / max_scroll
            };

            for i in 0..scrollbar_height {
                let y = area.y + header_rows + i as u16;
                if y >= area.y + area.height {
                    break;
                }
                let (ch, color) = if i >= thumb_pos && i < thumb_pos + thumb_size {
                    ("\u{2503}", theme.accent_warm)
                } else {
                    ("\u{2502}", theme.bar_empty)
                };
                buf.set_string(scrollbar_x, y, ch, Style::default().fg(color));
            }
        }
```

- [ ] **Step 6: Add line numbers to diff view**

In `src/tui/widgets/preview.rs`, in the `render_diff` method, add line number tracking and gutter rendering. Replace the diff lines loop (lines 105-129):

```rust
        // Diff lines with line numbers.
        let mut old_line: usize = 0;
        let mut new_line: usize = 0;

        for diff_line in edit.patch.lines() {
            if row >= max_y {
                break;
            }

            // Parse hunk headers for line numbers.
            if diff_line.starts_with("@@") {
                // Parse @@ -old,count +new,count @@
                if let Some(minus_pos) = diff_line.find('-') {
                    let after_minus = &diff_line[minus_pos + 1..];
                    let num_str: String = after_minus.chars().take_while(|c| c.is_ascii_digit()).collect();
                    old_line = num_str.parse::<usize>().unwrap_or(0);
                }
                if let Some(plus_pos) = diff_line.find('+') {
                    let after_plus = &diff_line[plus_pos + 1..];
                    let num_str: String = after_plus.chars().take_while(|c| c.is_ascii_digit()).collect();
                    new_line = num_str.parse::<usize>().unwrap_or(0);
                }

                render_at(
                    Line::from(vec![
                        Span::styled(format!("{:>5} ", ""), Style::default().fg(t.fg_dim)),
                        Span::styled(diff_line.to_string(), Style::default().fg(color_hunk)),
                    ]),
                    area, row, buf,
                );
                row += 1;
                continue;
            }

            let (gutter, color) = if diff_line.starts_with('+') {
                let g = format!("{:>5} ", new_line);
                new_line += 1;
                (g, color_add)
            } else if diff_line.starts_with('-') {
                let g = format!("{:>5} ", old_line);
                old_line += 1;
                (g, color_remove)
            } else {
                let g = format!("{:>5} ", new_line);
                old_line += 1;
                new_line += 1;
                (g, Color::Reset)
            };

            render_at(
                Line::from(vec![
                    Span::styled(gutter, Style::default().fg(t.fg_dim)),
                    Span::styled(diff_line.to_string(), Style::default().fg(color)),
                ]),
                area, row, buf,
            );
            row += 1;
        }
```

- [ ] **Step 7: Update diff view header to merged format**

Replace the diff header rendering (lines 75-86 in the `render_diff` method):

```rust
        render_at(
            Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(edit.file.clone(), Style::default().fg(color_header_value).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::styled(" \u{2502} ", Style::default().fg(t.separator)),
                Span::styled("diff", Style::default().fg(t.accent_warm)),
                Span::styled(" \u{2502} ", Style::default().fg(t.separator)),
                Span::styled(format!("+{}", edit.lines_added), Style::default().fg(color_add)),
                Span::styled(" ", Style::default()),
                Span::styled(format!("-{}", edit.lines_removed), Style::default().fg(color_remove)),
            ]),
            area, row, buf,
        );
        row += 1;
```

Remove the separate diff footer block (the "+N  -N" rendering at the bottom of `render_diff`).

- [ ] **Step 8: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 9: Commit**

```bash
git add src/tui/widgets/file_view.rs src/tui/widgets/preview.rs
git commit -m "feat(tui): preview pane polish -- merged header, scrollbar, diff line numbers, stronger tint"
```

---

### Task 7: Interaction Feedback -- Toasts and Mouse Routing

**Files:**
- Modify: `src/tui/event_loop.rs`
- Modify: `src/tui/input.rs`

- [ ] **Step 1: Add toast triggers for restore action**

In `src/tui/event_loop.rs`, in the `Action::Restore` handler (around line 230-260), after the successful restore, add:

After `let _ = restore_log.append(...)`:
```rust
                                        app.show_toast(
                                            format!("restored {}", edit.file),
                                            crate::tui::app::ToastStyle::Success,
                                        );
```

- [ ] **Step 2: Add toast trigger for undo restore**

In the `Action::UndoRestore` handler (around line 264-287), after the undo loop completes, add:

```rust
                                app.show_toast(
                                    "restore undone".to_string(),
                                    crate::tui::app::ToastStyle::Info,
                                );
```

- [ ] **Step 3: Add toast trigger for checkpoint**

In the `Action::Checkpoint` handler (around line 221-226), after `app.checkpoint_ids.push(id)`:

```rust
                                    app.show_toast(
                                        format!("checkpoint #{}", id),
                                        crate::tui::app::ToastStyle::Success,
                                    );
```

- [ ] **Step 4: Add playback flash trigger**

In `src/tui/input.rs`, in `apply_action`, in the `Action::TogglePlay` handler:

From:
```rust
        Action::TogglePlay => app.toggle_play(),
```
To:
```rust
        Action::TogglePlay => {
            app.toggle_play();
            app.playback_flash = Some(std::time::Instant::now());
        }
```

- [ ] **Step 5: Add track flash on scrub**

In `apply_action`, update `Action::ScrubLeft` and `Action::ScrubRight`:

From:
```rust
        Action::ScrubLeft => app.scrub_left(),
        Action::ScrubRight => app.scrub_right(),
```
To:
```rust
        Action::ScrubLeft => {
            let prev_file = app.current_edit().map(|e| e.file.clone());
            app.scrub_left();
            let new_file = app.current_edit().map(|e| e.file.clone());
            if prev_file != new_file {
                if let Some(f) = new_file {
                    app.track_flash = Some((f, std::time::Instant::now()));
                }
            }
        }
        Action::ScrubRight => {
            let prev_file = app.current_edit().map(|e| e.file.clone());
            app.scrub_right();
            let new_file = app.current_edit().map(|e| e.file.clone());
            if prev_file != new_file {
                if let Some(f) = new_file {
                    app.track_flash = Some((f, std::time::Instant::now()));
                }
            }
        }
```

- [ ] **Step 6: Add mouse-aware scroll routing**

In `src/tui/event_loop.rs`, replace the mouse scroll handling (around line 296-309):

From:
```rust
                Event::Mouse(mouse) => {
                    use crossterm::event::MouseEventKind;
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            app.timeline_zoom = (app.timeline_zoom * 1.2).min(20.0);
                        }
                        MouseEventKind::ScrollDown => {
                            app.timeline_zoom = (app.timeline_zoom / 1.2).max(1.0);
                            if app.timeline_zoom <= 1.01 {
                                app.timeline_zoom = 1.0;
                                app.timeline_scroll = 0;
                            }
                        }
                        _ => {}
                    }
                }
```
To:
```rust
                Event::Mouse(mouse) => {
                    use crossterm::event::MouseEventKind;
                    match mouse.kind {
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                            let is_up = matches!(mouse.kind, MouseEventKind::ScrollUp);

                            // Determine which pane the mouse is over.
                            let in_preview = app.last_layout.as_ref()
                                .map(|lo| {
                                    mouse.row >= lo.preview.y
                                        && mouse.row < lo.preview.y + lo.preview.height
                                        && mouse.column >= lo.preview.x
                                        && mouse.column < lo.preview.x + lo.preview.width
                                })
                                .unwrap_or(false);

                            let in_timeline = app.last_layout.as_ref()
                                .map(|lo| {
                                    mouse.row >= lo.timeline.y
                                        && mouse.row < lo.timeline.y + lo.timeline.height
                                })
                                .unwrap_or(false);

                            if in_preview {
                                // Scroll preview content.
                                if is_up && app.preview_scroll > 0 {
                                    app.preview_scroll = app.preview_scroll.saturating_sub(3);
                                    app.preview_scroll_target = app.preview_scroll;
                                } else if !is_up {
                                    app.preview_scroll += 3;
                                    app.preview_scroll_target = app.preview_scroll;
                                }
                            } else if in_timeline {
                                // Zoom timeline.
                                if is_up {
                                    app.timeline_zoom = (app.timeline_zoom * 1.2).min(20.0);
                                } else {
                                    app.timeline_zoom = (app.timeline_zoom / 1.2).max(1.0);
                                    if app.timeline_zoom <= 1.01 {
                                        app.timeline_zoom = 1.0;
                                        app.timeline_scroll = 0;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
```

- [ ] **Step 7: Add toast trigger for toggle restore edits**

In `src/tui/input.rs`, update the `Action::ToggleRestoreEdits` handler:

From:
```rust
        Action::ToggleRestoreEdits => {
            app.show_restore_edits = !app.show_restore_edits;
        }
```
To:
```rust
        Action::ToggleRestoreEdits => {
            app.show_restore_edits = !app.show_restore_edits;
            let msg = if app.show_restore_edits {
                "restore edits: visible"
            } else {
                "restore edits: hidden"
            };
            app.show_toast(msg.to_string(), crate::tui::app::ToastStyle::Info);
        }
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 9: Commit**

```bash
git add src/tui/event_loop.rs src/tui/input.rs
git commit -m "feat(tui): add toast notifications, playback flash, track flash, mouse-aware scrolling"
```

---

### Task 8: Implement SoloAgent and Remaining Polish

**Files:**
- Modify: `src/tui/input.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/widgets/timeline.rs`
- Modify: `src/tui/widgets/preview.rs` (empty state)

- [ ] **Step 1: Implement SoloAgent action**

In `src/tui/input.rs`, replace the no-op `Action::SoloAgent` handler:

From:
```rust
        Action::SoloAgent(_n) => {
            // Implementation deferred to integration phase -- sets a flag on App.
        }
```
To:
```rust
        Action::SoloAgent(n) => {
            // Collect unique agent IDs sorted by first appearance.
            let mut seen = std::collections::HashSet::new();
            let mut agents: Vec<String> = Vec::new();
            for edit in &app.edits {
                if let Some(ref agent_id) = edit.agent_id {
                    if seen.insert(agent_id.clone()) {
                        agents.push(agent_id.clone());
                    }
                }
            }

            let idx = (n as usize).saturating_sub(1);
            if let Some(agent_id) = agents.get(idx) {
                if app.solo_agent.as_ref() == Some(agent_id) {
                    // Toggle off.
                    app.solo_agent = None;
                } else {
                    app.solo_agent = Some(agent_id.clone());
                }
            }
        }
```

- [ ] **Step 2: Apply solo_agent filter in timeline visible_tracks**

In `src/tui/widgets/timeline.rs`, update the `visible_tracks` method to also filter by `solo_agent`:

From:
```rust
    fn visible_tracks(&self) -> Vec<&crate::tui::TrackInfo> {
        self.app
            .tracks
            .iter()
            .filter(|t| {
                if let Some(solo) = &self.app.solo_track {
                    return &t.filename == solo;
                }
                !self.app.muted_tracks.contains(&t.filename)
            })
            .collect()
    }
```
To:
```rust
    fn visible_tracks(&self) -> Vec<&crate::tui::TrackInfo> {
        self.app
            .tracks
            .iter()
            .filter(|t| {
                // File-level solo filter.
                if let Some(solo) = &self.app.solo_track {
                    return &t.filename == solo;
                }
                // Skip muted tracks.
                if self.app.muted_tracks.contains(&t.filename) {
                    return false;
                }
                // Agent-level solo filter: only show tracks with edits from this agent.
                if let Some(ref solo_agent) = self.app.solo_agent {
                    return t.edit_indices.iter().any(|&idx| {
                        self.app.edits.get(idx)
                            .and_then(|e| e.agent_id.as_ref())
                            .map(|a| a == solo_agent)
                            .unwrap_or(false)
                    });
                }
                true
            })
            .collect()
    }
```

- [ ] **Step 3: Refresh the empty state**

In `src/tui/widgets/preview.rs`, replace the `render_empty_state` function's logo with a smaller version:

From:
```rust
    let logo = [
        r"       _ _          _                          ",
        r"__   _(_) |__   ___| |_ _ __ __ _  ___ ___ _ __",
        r"\ \ / / | '_ \ / _ \ __| '__/ _` |/ __/ _ \ '__|",
        r" \ V /| | |_) |  __/ |_| | | (_| | (_|  __/ |  ",
        r"  \_/ |_|_.__/ \___|\__|_|  \__,_|\___\___|_|  ",
    ];
```
To:
```rust
    let logo = [
        r"        _ _         _                        ",
        r" __   _(_) |__  ___| |_ _ __ __ _  ___ ___ _ __",
        r" \_/ |_|_.__/\___|\__|_| \__,_|\___\___|_|  ",
    ];
```

Change the logo color:

From:
```rust
    let color_warm = theme.accent_warm;
```
To:
```rust
    let color_warm = theme.fg_dim;
```

And update the reference from `color_warm` in the logo rendering line (the `buf.set_string` call).

- [ ] **Step 4: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 5: Run all tests**

Run: `cargo test --lib 2>&1 | tail -5`
Expected: All tests pass. The theme tests should pass since we didn't change theme.rs.

- [ ] **Step 6: Commit**

```bash
git add src/tui/input.rs src/tui/app.rs src/tui/widgets/timeline.rs src/tui/widgets/preview.rs
git commit -m "feat(tui): implement solo-agent filter, refresh empty state"
```

---

### Task 9: Consistent Inner Padding for All Panels

**Files:**
- Modify: `src/tui/widgets/blast_radius_panel.rs`
- Modify: `src/tui/widgets/sentinel_panel.rs`
- Modify: `src/tui/widgets/watchdog_panel.rs`

Note: If Task 3 already added inner padding to these files (the `area.x + 1` changes), this task is already done. Verify and skip if so.

- [ ] **Step 1: Verify padding was applied in Task 3**

Check each panel's render method to confirm content starts at `area.x + 1`. If Task 3 already made this change, mark this task complete.

If not, add a padding wrapper at the top of each panel's `render` method:

```rust
        let area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compilation.

- [ ] **Step 3: Commit (if changes were needed)**

```bash
git add src/tui/widgets/blast_radius_panel.rs src/tui/widgets/sentinel_panel.rs src/tui/widgets/watchdog_panel.rs
git commit -m "fix(tui): consistent 1-char inner padding for sidebar panels"
```

---

### Task 10: Final Integration Test and Cleanup

**Files:**
- All modified files

- [ ] **Step 1: Full compilation check**

Run: `cargo check 2>&1`
Expected: Clean compilation with zero errors.

- [ ] **Step 2: Run all library tests**

Run: `cargo test --lib 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test integration_tests 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all-targets 2>&1 | tail -20`
Expected: No errors. Fix any new warnings introduced by our changes.

- [ ] **Step 5: Fix any clippy warnings**

Address any warnings from Step 4 (unused imports, unnecessary clones, etc.).

- [ ] **Step 6: Final commit if fixes were needed**

```bash
git add -A
git commit -m "chore(tui): fix clippy warnings from visual overhaul"
```

- [ ] **Step 7: Verify git log shows clean commit history**

Run: `git log --oneline -10`
Expected: Series of well-named commits from Tasks 1-9.
