# Full NLE Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform vibetracer's TUI from functional to "whoa" -- reconstructed file preview with syntax highlighting, overhauled timeline with zoom/clip segments, and smooth 60fps playback.

**Architecture:** Three sequential workstreams that each produce independently shippable improvements. Workstream 1 (preview pane) adds `syntect` and a new `file_view` widget. Workstream 2 (timeline) rewrites the timeline widget with zoom, clip segments, and time scale. Workstream 3 (playback) adds frame-rate-aware rendering and smooth scroll animation.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28, syntect (new dependency)

**Design Doc:** `~/.gstack/projects/omeedcs-vibetracer/omeedtehrani-main-design-20260325-155520.md`

---

## File Structure

| File | Responsibility | Status |
|------|---------------|--------|
| `Cargo.toml` | Add `syntect` dependency | Modify |
| `src/tui/syntax.rs` | Syntect initialization, theme mapping, highlight caching | **NEW** |
| `src/tui/widgets/file_view.rs` | Reconstructed file view widget with syntax highlighting | **NEW** |
| `src/tui/widgets/preview.rs` | Delegate to file_view or diff based on preview mode | Modify |
| `src/tui/widgets/timeline.rs` | Rewrite for clip segments, zoom, time scale, full-height playhead | Modify |
| `src/tui/widgets/status_bar.rs` | Add edit timestamp, scrubbing indicator | Modify |
| `src/tui/widgets/mod.rs` | Export `file_view` module | Modify |
| `src/tui/app.rs` | Add preview_mode, scroll state, timeline zoom/scroll, snapshot cache | Modify |
| `src/tui/input.rs` | Add new keybindings (d, j/k, +/-, 0) and actions | Modify |
| `src/tui/event_loop.rs` | Frame-rate-aware rendering, smooth scroll interpolation, snapshot access | Modify |
| `src/tui/layout.rs` | Adjust timeline height for time scale bar | Modify |
| `src/tui/mod.rs` | Export `syntax` module | Modify |
| `src/lib.rs` | No changes needed (tui module already exported) | - |

---

## Task 1: Add syntect dependency and create syntax highlighting module

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tui/syntax.rs`
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Add syntect to Cargo.toml**

```toml
# Add to [dependencies] section after `sha2`:
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "regex-onig"] }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors (syntect pulls in cleanly).

- [ ] **Step 3: Create `src/tui/syntax.rs` with highlighter struct**

This module wraps syntect and provides a simple API: give it a filename and file content, get back styled lines compatible with ratatui. It also maps vibetracer theme colors into syntect scope overrides.

```rust
use ratatui::style::Color as RatColor;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, Style, StyleModifier, Theme, ThemeItem,
    ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Wraps syntect for syntax highlighting with vibetracer theme integration.
pub struct Highlighter {
    syntax_set: SyntaxSet,
}

/// A single styled character range within a line.
pub struct StyledSegment {
    pub text: String,
    pub fg: RatColor,
    pub bold: bool,
    pub italic: bool,
}

/// A line of syntax-highlighted text.
pub type HighlightedLine = Vec<StyledSegment>;

impl Highlighter {
    /// Create a new highlighter with syntect's default syntax definitions.
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// Build a syntect Theme from vibetracer's active color scheme.
    /// Maps 5 scope colors from the vibetracer palette; all other scopes
    /// use `default_fg` as the foreground.
    fn build_theme(theme: &crate::theme::Theme) -> Theme {
        let default_fg = to_syn_color(theme.fg);
        let settings = ThemeSettings {
            foreground: Some(default_fg),
            background: Some(SynColor { r: 0, g: 0, b: 0, a: 0 }), // transparent
            ..Default::default()
        };

        let scopes = vec![
            theme_item("keyword", theme.accent_warm, false),
            theme_item("storage", theme.accent_warm, false),
            theme_item("string", theme.accent_green, false),
            theme_item("comment", theme.fg_dim, true),
            theme_item("entity.name.function", theme.accent_blue, false),
            theme_item("entity.name.type", theme.accent_purple, false),
            theme_item("entity.name.class", theme.accent_purple, false),
            theme_item("constant.numeric", theme.accent_warm, false),
            theme_item("variable.parameter", theme.fg, false),
            theme_item("punctuation", theme.fg_muted, false),
        ];

        Theme {
            name: Some("vibetracer".to_string()),
            author: None,
            settings,
            scopes,
        }
    }

    /// Detect the syntax for a filename, falling back to plain text.
    fn detect_syntax(&self, filename: &str) -> &SyntaxReference {
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        self.syntax_set
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }

    /// Highlight file content and return a vec of styled lines.
    /// Each line is a vec of `StyledSegment`s.
    pub fn highlight(
        &self,
        filename: &str,
        content: &str,
        theme: &crate::theme::Theme,
    ) -> Vec<HighlightedLine> {
        let syntax = self.detect_syntax(filename);
        let syn_theme = Self::build_theme(theme);
        let mut h = syntect::easy::HighlightLines::new(syntax, &syn_theme);
        let mut result = Vec::new();

        for line in syntect::util::LinesWithEndings::from(content) {
            let ranges = h
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let segments: Vec<StyledSegment> = ranges
                .into_iter()
                .map(|(style, text)| StyledSegment {
                    text: text.trim_end_matches('\n').to_string(),
                    fg: syn_to_rat_color(style.foreground),
                    bold: style.font_style.contains(FontStyle::BOLD),
                    italic: style.font_style.contains(FontStyle::ITALIC),
                })
                .collect();
            result.push(segments);
        }
        result
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a ratatui Color::Rgb to a syntect Color.
fn to_syn_color(c: RatColor) -> SynColor {
    match c {
        RatColor::Rgb(r, g, b) => SynColor { r, g, b, a: 255 },
        _ => SynColor { r: 160, g: 168, b: 183, a: 255 }, // fallback: neutral gray
    }
}

/// Convert a syntect Color back to a ratatui Color.
fn syn_to_rat_color(c: SynColor) -> RatColor {
    RatColor::Rgb(c.r, c.g, c.b)
}

/// Build a ThemeItem for a given scope selector string.
fn theme_item(scope_str: &str, color: RatColor, italic: bool) -> ThemeItem {
    let selector = ScopeSelectors::from_str(scope_str).unwrap_or_default();
    let font_style = if italic {
        Some(FontStyle::ITALIC)
    } else {
        None
    };
    ThemeItem {
        scope: selector,
        style: StyleModifier {
            foreground: Some(to_syn_color(color)),
            background: None,
            font_style,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighter_creates_without_panic() {
        let _h = Highlighter::new();
    }

    #[test]
    fn highlight_rust_code() {
        let h = Highlighter::new();
        let theme = crate::theme::Theme::dark();
        let lines = h.highlight("main.rs", "fn main() {\n    println!(\"hello\");\n}\n", &theme);
        assert_eq!(lines.len(), 3);
        // Each line should have at least one segment
        for line in &lines {
            assert!(!line.is_empty());
        }
    }

    #[test]
    fn highlight_unknown_extension_falls_back() {
        let h = Highlighter::new();
        let theme = crate::theme::Theme::dark();
        let lines = h.highlight("data.xyz", "some plain text\n", &theme);
        assert_eq!(lines.len(), 1);
    }
}
```

- [ ] **Step 4: Export the module from `src/tui/mod.rs`**

Add `pub mod syntax;` after the existing module declarations:

```rust
pub mod syntax;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::syntax`
Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/tui/syntax.rs src/tui/mod.rs
git commit -m "feat: add syntect-based syntax highlighting module"
```

---

## Task 2: Add preview mode, scroll state, and snapshot cache to App

**Files:**
- Modify: `src/tui/app.rs`

- [ ] **Step 1: Add new types and fields to `App`**

Add a `PreviewMode` enum and new fields to `App`:

```rust
// Add above the App struct definition:

/// Which view mode the preview pane is in.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    /// Show reconstructed file content with syntax highlighting.
    File,
    /// Show the raw unified diff (legacy view).
    Diff,
}
```

Add these fields to the `App` struct (inside the `// v2 fields` section):

```rust
    /// Preview pane mode (file view vs diff view).
    pub preview_mode: PreviewMode,
    /// Vertical scroll offset in the preview pane.
    pub preview_scroll: usize,
    /// Target scroll position for smooth scroll animation.
    pub preview_scroll_target: usize,

    /// Cached file content for the current edit's after_hash.
    pub cached_content: Option<(String, String)>, // (hash, content)

    /// Timeline zoom level (1.0 = fit-all, >1.0 = zoomed in).
    pub timeline_zoom: f64,
    /// Timeline horizontal scroll offset (in edit indices).
    pub timeline_scroll: usize,
```

Update the `App::new()` defaults:

```rust
    preview_mode: PreviewMode::File,
    preview_scroll: 0,
    preview_scroll_target: 0,
    cached_content: None,
    timeline_zoom: 1.0,
    timeline_scroll: 0,
```

- [ ] **Step 2: Add helper method for retrieving cached content**

Add to `impl App`:

```rust
    /// Get the file content for the current edit, using cache when possible.
    /// Returns (content, filename) or None if no edit is selected.
    pub fn current_file_content(&mut self, session_dir: &std::path::Path) -> Option<(String, String)> {
        let edit = self.edits.get(self.playhead)?;
        let hash = &edit.after_hash;
        let filename = edit.file.clone();

        // Check cache
        if let Some((cached_hash, cached_content)) = &self.cached_content {
            if cached_hash == hash {
                return Some((cached_content.clone(), filename));
            }
        }

        // Retrieve from store
        let store = crate::snapshot::store::SnapshotStore::new(
            session_dir.join("snapshots"),
        );
        let bytes = store.retrieve(hash).ok()?;
        let content = String::from_utf8_lossy(&bytes).to_string();
        self.cached_content = Some((hash.clone(), content.clone()));
        Some((content, filename))
    }

    /// Identify which line numbers were changed by the current edit's patch.
    /// Returns a set of 1-based line numbers that were added or modified.
    pub fn changed_lines_from_patch(&self) -> std::collections::HashSet<usize> {
        let mut changed = std::collections::HashSet::new();
        let edit = match self.current_edit() {
            Some(e) => e,
            None => return changed,
        };

        let mut current_line: usize = 0;
        for line in edit.patch.lines() {
            if line.starts_with("@@") {
                // Parse hunk header: @@ -old,count +new,count @@
                if let Some(plus_part) = line.split('+').nth(1) {
                    if let Some(line_num_str) = plus_part.split(',').next() {
                        if let Some(num_str) = line_num_str.split(' ').next() {
                            current_line = num_str.parse::<usize>().unwrap_or(1);
                        }
                    }
                }
            } else if line.starts_with('+') {
                changed.insert(current_line);
                current_line += 1;
            } else if line.starts_with('-') {
                // Removed line: don't advance current_line in the new file
            } else {
                // Context line
                current_line += 1;
            }
        }
        changed
    }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat: add preview mode, scroll state, timeline zoom, and snapshot cache to App"
```

---

## Task 3: Create the file_view widget

**Files:**
- Create: `src/tui/widgets/file_view.rs`
- Modify: `src/tui/widgets/mod.rs`

- [ ] **Step 1: Create `src/tui/widgets/file_view.rs`**

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::collections::HashSet;

use crate::event::EditKind;
use crate::tui::syntax::{HighlightedLine, Highlighter};
use crate::tui::App;

/// Width of the line-number gutter (e.g. " 123 ").
const GUTTER_WIDTH: u16 = 6;

/// A widget that renders syntax-highlighted file content at the current playhead.
pub struct FileView<'a> {
    pub app: &'a App,
    pub content: &'a str,
    pub filename: &'a str,
    pub highlighter: &'a Highlighter,
    pub changed_lines: &'a HashSet<usize>,
}

impl<'a> FileView<'a> {
    pub fn new(
        app: &'a App,
        content: &'a str,
        filename: &'a str,
        highlighter: &'a Highlighter,
        changed_lines: &'a HashSet<usize>,
    ) -> Self {
        Self {
            app,
            content,
            filename,
            highlighter,
            changed_lines,
        }
    }
}

impl Widget for FileView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < GUTTER_WIDTH + 4 {
            return;
        }

        let t = &self.app.theme;

        // Check for special edit kinds
        if let Some(edit) = self.app.current_edit() {
            if edit.kind == EditKind::Delete {
                render_deleted_state(area, buf, t, &edit.file);
                return;
            }
        }

        if self.content.is_empty() {
            render_empty_file(area, buf, t);
            return;
        }

        // Header line: filename
        let header = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(self.filename, Style::default().fg(t.fg)),
        ]);
        header.render(
            Rect { x: area.x, y: area.y, width: area.width, height: 1 },
            buf,
        );

        // Syntax highlight the content
        let highlighted = self.highlighter.highlight(self.filename, self.content, &self.app.theme);

        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        let scroll = self.app.preview_scroll;
        let visible_lines = content_area.height as usize;
        let code_width = (content_area.width - GUTTER_WIDTH) as usize;

        let gutter_style = Style::default().fg(t.fg_dim);
        let change_bg_add = color_with_alpha(t.accent_green, 40);
        let change_bg_del = color_with_alpha(t.accent_red, 40);

        // Are all lines "added"? (Create edits or missing before_hash)
        let all_added = self.app.current_edit()
            .map(|e| e.kind == EditKind::Create || e.before_hash.is_none())
            .unwrap_or(false);

        for (i, line_idx) in (scroll..scroll + visible_lines).enumerate() {
            let y = content_area.y + i as u16;
            if y >= content_area.y + content_area.height {
                break;
            }

            if line_idx >= highlighted.len() {
                // Past end of file -- render blank line with tilde
                let tilde = Line::from(vec![
                    Span::styled(format!("{:>width$} ", "~", width = GUTTER_WIDTH as usize - 1), gutter_style),
                ]);
                tilde.render(Rect { x: content_area.x, y, width: content_area.width, height: 1 }, buf);
                continue;
            }

            let line_num = line_idx + 1; // 1-based
            let is_changed = all_added || self.changed_lines.contains(&line_num);

            // Render gutter (line number)
            let gutter_text = format!("{:>width$} ", line_num, width = GUTTER_WIDTH as usize - 1);
            let gutter_color = if is_changed { t.accent_green } else { t.fg_dim };
            buf.set_string(
                content_area.x,
                y,
                &gutter_text,
                Style::default().fg(gutter_color),
            );

            // Apply subtle background tint for changed lines
            if is_changed {
                for x in content_area.x..content_area.x + content_area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_bg(change_bg_add);
                    }
                }
            }

            // Render syntax-highlighted segments
            let segments = &highlighted[line_idx];
            let mut col = content_area.x + GUTTER_WIDTH;
            let max_col = content_area.x + content_area.width;

            for seg in segments {
                if col >= max_col {
                    break;
                }
                let available = (max_col - col) as usize;
                let text: String = seg.text.chars().take(available).collect();
                let mut style = Style::default().fg(seg.fg);
                if seg.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if seg.italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                buf.set_string(col, y, &text, style);
                col += text.len() as u16;
            }
        }

        // Footer: line count and scroll position
        let total_lines = highlighted.len();
        let footer_y = area.y + area.height - 1;
        if footer_y > area.y + 1 && area.height > 3 {
            // Use the last row of the area for a subtle footer
            let pct = if total_lines <= visible_lines {
                100
            } else {
                ((scroll as f64 / (total_lines - visible_lines) as f64) * 100.0).min(100.0) as usize
            };
            let footer_text = format!(" {}/{} lines  {}%", scroll + 1, total_lines, pct);
            buf.set_string(
                area.x,
                footer_y,
                &footer_text,
                Style::default().fg(t.fg_dim),
            );
        }
    }
}

/// Render a "file deleted" placeholder.
fn render_deleted_state(area: Rect, buf: &mut Buffer, theme: &crate::theme::Theme, filename: &str) {
    if area.height < 3 {
        return;
    }
    let msg = format!("{} (deleted)", filename);
    let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, &msg, Style::default().fg(theme.accent_red));
}

/// Render an "empty file" placeholder.
fn render_empty_file(area: Rect, buf: &mut Buffer, theme: &crate::theme::Theme) {
    if area.height < 3 {
        return;
    }
    let msg = "(empty file)";
    let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, msg, Style::default().fg(theme.fg_dim));
}

/// Create a muted version of a color for background tinting.
/// This produces a dark-tinted version suitable for change highlighting.
fn color_with_alpha(color: Color, intensity: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            // Blend toward black at the given intensity (0-255)
            let scale = intensity as f32 / 255.0;
            Color::Rgb(
                (r as f32 * scale) as u8,
                (g as f32 * scale) as u8,
                (b as f32 * scale) as u8,
            )
        }
        _ => Color::Rgb(20, 30, 20), // fallback dark green tint
    }
}
```

- [ ] **Step 2: Export from `src/tui/widgets/mod.rs`**

Add `pub mod file_view;` to the module declarations:

```rust
pub mod file_view;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/tui/widgets/file_view.rs src/tui/widgets/mod.rs
git commit -m "feat: add file_view widget with syntax highlighting and change markers"
```

---

## Task 4: Wire file_view into the preview pane and add keybindings

**Files:**
- Modify: `src/tui/widgets/preview.rs`
- Modify: `src/tui/input.rs`
- Modify: `src/tui/event_loop.rs`

- [ ] **Step 1: Update `PreviewPane` to delegate based on preview mode**

Replace the `Widget` impl for `PreviewPane` in `src/tui/widgets/preview.rs`. The widget now checks `app.preview_mode` and either renders the existing diff view or delegates to `FileView`. Since `FileView` needs a `Highlighter` and content that come from outside, we add fields to `PreviewPane`:

Add new fields to `PreviewPane`:

```rust
pub struct PreviewPane<'a> {
    pub app: &'a App,
    pub file_content: Option<(&'a str, &'a str)>, // (content, filename)
    pub highlighter: Option<&'a crate::tui::syntax::Highlighter>,
    pub changed_lines: &'a std::collections::HashSet<usize>,
}
```

Update the `new` constructor:

```rust
impl<'a> PreviewPane<'a> {
    pub fn new(
        app: &'a App,
        file_content: Option<(&'a str, &'a str)>,
        highlighter: Option<&'a crate::tui::syntax::Highlighter>,
        changed_lines: &'a std::collections::HashSet<usize>,
    ) -> Self {
        Self { app, file_content, highlighter, changed_lines }
    }
}
```

Update the `Widget::render` impl to check mode:

```rust
impl Widget for PreviewPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        match self.app.preview_mode {
            crate::tui::app::PreviewMode::File => {
                if let (Some((content, filename)), Some(highlighter)) =
                    (self.file_content, self.highlighter)
                {
                    super::file_view::FileView::new(
                        self.app,
                        content,
                        filename,
                        highlighter,
                        self.changed_lines,
                    )
                    .render(area, buf);
                } else {
                    // Fallback to empty state if no content available
                    render_empty_state(area, buf, &self.app.theme);
                }
            }
            crate::tui::app::PreviewMode::Diff => {
                // Existing diff rendering logic (keep all existing code from here)
                self.render_diff(area, buf);
            }
        }
    }
}
```

Extract the current diff rendering into a `render_diff` method on the impl to keep it accessible:

```rust
impl PreviewPane<'_> {
    fn render_diff(self, area: Rect, buf: &mut Buffer) {
        // ... move the entire existing render body here (the diff rendering code)
    }
}
```

- [ ] **Step 2: Add new actions to `src/tui/input.rs`**

Add these variants to the `Action` enum:

```rust
    TogglePreviewMode,
    ScrollPreviewUp,
    ScrollPreviewDown,
    ZoomTimelineIn,
    ZoomTimelineOut,
    ZoomTimelineReset,
```

Add key mappings in `map_key`:

```rust
        // Preview mode toggle
        (KeyCode::Char('d'), KeyModifiers::NONE) => Action::TogglePreviewMode,

        // Preview scroll (when preview is focused)
        (KeyCode::Char('j'), KeyModifiers::NONE) => Action::ScrollPreviewDown,
        (KeyCode::Char('k'), KeyModifiers::NONE) => Action::ScrollPreviewUp,
        (KeyCode::Up, KeyModifiers::NONE) => Action::ScrollPreviewUp,
        (KeyCode::Down, KeyModifiers::NONE) => Action::ScrollPreviewDown,

        // Timeline zoom
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), KeyModifiers::NONE) => Action::ZoomTimelineIn,
        (KeyCode::Char('-'), KeyModifiers::NONE) => Action::ZoomTimelineOut,
        (KeyCode::Char('0'), KeyModifiers::NONE) => Action::ZoomTimelineReset,
```

Add handlers in `apply_action`:

```rust
        Action::TogglePreviewMode => {
            app.preview_mode = match app.preview_mode {
                PreviewMode::File => PreviewMode::Diff,
                PreviewMode::Diff => PreviewMode::File,
            };
        }

        Action::ScrollPreviewUp => {
            if app.focused_pane == Pane::Preview && app.preview_scroll > 0 {
                app.preview_scroll -= 1;
                app.preview_scroll_target = app.preview_scroll;
            }
        }
        Action::ScrollPreviewDown => {
            if app.focused_pane == Pane::Preview {
                app.preview_scroll += 1;
                app.preview_scroll_target = app.preview_scroll;
            }
        }

        Action::ZoomTimelineIn => {
            app.timeline_zoom = (app.timeline_zoom * 1.5).min(20.0);
        }
        Action::ZoomTimelineOut => {
            app.timeline_zoom = (app.timeline_zoom / 1.5).max(1.0);
        }
        Action::ZoomTimelineReset => {
            app.timeline_zoom = 1.0;
            app.timeline_scroll = 0;
        }
```

Note: The `Up`/`Down`/`j`/`k` keys now need focus-gating. When `focused_pane` is `Preview`, they scroll. When it's `Timeline`, `Up`/`Down` could be used for track selection in the future. For now, the `apply_action` handler checks `focused_pane`.

- [ ] **Step 3: Update the event loop to pass content and highlighter to PreviewPane**

In `src/tui/event_loop.rs`, the `Highlighter` needs to be created once at the start of the event loop and the file content retrieved from the snapshot store.

At the top of `run_event_loop`, add:

```rust
    let highlighter = crate::tui::syntax::Highlighter::new();
    let empty_changed: std::collections::HashSet<usize> = std::collections::HashSet::new();
```

In the render closure where `PreviewPane` is currently constructed, change:

```rust
            // Before the render closure, compute content and changed lines:
            let file_content_data: Option<(String, String)> = app.current_file_content(session_dir);
            let changed_lines = app.changed_lines_from_patch();

            // Inside terminal.draw:
            let content_ref = file_content_data.as_ref().map(|(c, f)| (c.as_str(), f.as_str()));
            widgets::preview::PreviewPane::new(
                app,
                content_ref,
                Some(&highlighter),
                &changed_lines,
            ).render(lo.preview, buf);
```

- [ ] **Step 4: Auto-center scroll on changed region when playhead moves**

Add tracking for the last playhead position and auto-scroll logic. In the event loop, after processing scrub actions and before rendering:

```rust
    let mut last_playhead: usize = app.playhead;

    // ... in the loop, after input handling:
    if app.playhead != last_playhead {
        // Playhead moved -- auto-center on the first changed line
        let changed = app.changed_lines_from_patch();
        if let Some(&first_changed) = changed.iter().min() {
            // Target: center the first changed line in the visible area
            let visible = 20; // approximate; actual comes from render area
            app.preview_scroll_target = first_changed.saturating_sub(visible / 2);
            app.preview_scroll = app.preview_scroll_target; // instant for now (smooth scroll in WS3)
        }
        app.cached_content = None; // invalidate cache on playhead move
        last_playhead = app.playhead;
    }
```

- [ ] **Step 5: Verify it compiles and existing tests pass**

Run: `cargo test`
Expected: All 104 existing tests pass. No regressions.

- [ ] **Step 6: Commit**

```bash
git add src/tui/widgets/preview.rs src/tui/input.rs src/tui/event_loop.rs
git commit -m "feat: wire file_view into preview pane with syntax highlighting and d/j/k keybindings"
```

---

## Task 5: Rewrite the timeline widget with zoom, clip segments, time scale, and full-height playhead

**Files:**
- Modify: `src/tui/widgets/timeline.rs`
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Update layout to give timeline more height for the time scale bar**

In `src/tui/layout.rs`, increase the timeline height by 1 row to accommodate the time scale bar:

```rust
    let timeline_height = if area.height < 15 {
        4  // was 3
    } else if area.height < 25 {
        6  // was 5
    } else {
        10 // was 8
    };
```

- [ ] **Step 2: Rewrite `TimelineWidget::render()`**

Replace the entire render implementation in `src/tui/widgets/timeline.rs`. The new version:
- Draws a time scale bar on the first row
- Renders clip segments with proportional width when zoomed in
- Uses a full-height playhead spanning all tracks
- Shows an agent color legend when multiple agents are present

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

const TRACK_NAME_WIDTH: usize = 14;
const SEPARATOR: &str = " ";

pub struct TimelineWidget<'a> {
    pub app: &'a App,
}

impl<'a> TimelineWidget<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

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

    fn display_name(filename: &str) -> String {
        if filename.len() <= TRACK_NAME_WIDTH {
            return format!("{:<width$}", filename, width = TRACK_NAME_WIDTH);
        }
        let base = std::path::Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);
        if base.len() <= TRACK_NAME_WIDTH {
            format!("{:<width$}", base, width = TRACK_NAME_WIDTH)
        } else {
            format!("{:.width$}", base, width = TRACK_NAME_WIDTH)
        }
    }

    fn agent_color_for_edit(&self, edit_idx: usize) -> Option<Color> {
        let edit = self.app.edits.get(edit_idx)?;
        let agent_id = edit.agent_id.as_ref()?;
        let agent_colors = &self.app.theme.agent_colors;
        if agent_colors.is_empty() {
            return None;
        }
        let hash: usize = agent_id.bytes().map(|b| b as usize).sum();
        Some(agent_colors[hash % agent_colors.len()])
    }

    /// Map an edit index to a horizontal cell position within the bar.
    fn edit_to_col(&self, edit_idx: usize, bar_width: usize, total_edits: usize) -> usize {
        if total_edits == 0 || bar_width == 0 {
            return 0;
        }
        let zoom = self.app.timeline_zoom;
        let scroll = self.app.timeline_scroll;

        if zoom <= 1.0 {
            // Fit-all: compress all edits into available width
            if total_edits <= bar_width {
                edit_idx
            } else {
                edit_idx * bar_width / total_edits
            }
        } else {
            // Zoomed: each edit gets (zoom) cells, offset by scroll
            let pos = (edit_idx as f64 * zoom) as usize;
            pos.saturating_sub(scroll)
        }
    }

    /// Format a millisecond timestamp as HH:MM:SS relative to session start.
    fn format_time(&self, ts_ms: i64) -> String {
        let session_start_ms = self.app.session_start * 1000;
        let offset_secs = ((ts_ms - session_start_ms).max(0) / 1000) as u64;
        let h = offset_secs / 3600;
        let m = (offset_secs % 3600) / 60;
        let s = offset_secs % 60;
        if h > 0 {
            format!("{h}:{m:02}:{s:02}")
        } else {
            format!("{m}:{s:02}")
        }
    }
}

impl Widget for TimelineWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let t = &self.app.theme;
        let tracks = self.visible_tracks();
        let total_edits = self.app.edits.len();
        let name_and_sep = TRACK_NAME_WIDTH + SEPARATOR.len();
        let bar_width = (area.width as usize).saturating_sub(name_and_sep);

        let mut row = area.y;
        let max_y = area.y + area.height;

        // ── time scale bar ────────────────────────────────────────────────
        if row < max_y && total_edits > 1 {
            let mut time_spans: Vec<Span> = vec![
                Span::styled(
                    format!("{:<width$}", "", width = TRACK_NAME_WIDTH),
                    Style::default(),
                ),
                Span::raw(SEPARATOR),
            ];

            // Place timestamps at regular intervals across the bar
            let interval = if bar_width > 0 { (bar_width / 5).max(1) } else { 1 };
            let mut time_str = String::new();
            for col in 0..bar_width {
                if col % interval == 0 && total_edits > 0 {
                    // Map this column back to an edit index
                    let edit_idx = if self.app.timeline_zoom <= 1.0 {
                        (col * total_edits / bar_width).min(total_edits - 1)
                    } else {
                        let idx = (col + self.app.timeline_scroll) as f64 / self.app.timeline_zoom;
                        (idx as usize).min(total_edits - 1)
                    };
                    let ts = self.app.edits.get(edit_idx).map(|e| e.ts).unwrap_or(0);
                    let label = self.format_time(ts);
                    // Only add if there's room
                    if col + label.len() <= bar_width {
                        time_str.push_str(&label);
                        // Pad to next interval
                        let pad = interval.saturating_sub(label.len());
                        time_str.extend(std::iter::repeat(' ').take(pad));
                    } else {
                        time_str.push(' ');
                    }
                } else if time_str.len() <= col {
                    time_str.push(' ');
                }
            }
            time_str.truncate(bar_width);
            time_spans.push(Span::styled(time_str, Style::default().fg(t.fg_dim)));

            Line::from(time_spans).render(
                Rect { x: area.x, y: row, width: area.width, height: 1 },
                buf,
            );
            row += 1;
        }

        // ── empty state ──────────────────────────────────────────────────
        if tracks.is_empty() && row < max_y {
            row += 1;
            if row < max_y {
                let waiting = "waiting for file changes...";
                let x = area.x + (area.width.saturating_sub(waiting.len() as u16)) / 2;
                buf.set_string(x, row, waiting, Style::default().fg(t.separator));
            }
            return;
        }

        // ── track rows ───────────────────────────────────────────────────
        let track_start_row = row;
        for track in &tracks {
            if row >= max_y.saturating_sub(1) {
                break; // Reserve last row for playhead
            }

            let is_detached = self.app.detached_files.contains(&track.filename);

            let (name_text, name_color) = if track.stale {
                let display = Self::display_name(&track.filename);
                let truncated: String = display.chars().take(TRACK_NAME_WIDTH.min(8)).collect();
                (format!("{} stale", truncated), t.accent_red)
            } else if is_detached {
                (Self::display_name(&track.filename), t.accent_purple)
            } else {
                (Self::display_name(&track.filename), t.fg)
            };

            let mut spans: Vec<Span> = vec![
                Span::styled(
                    format!("{:<width$}", name_text, width = TRACK_NAME_WIDTH),
                    Style::default().fg(name_color),
                ),
                Span::raw(SEPARATOR),
            ];

            // Build bar for this track
            if bar_width > 0 && total_edits > 0 {
                let mut bar_chars: Vec<(char, Color)> = vec![('\u{2591}', t.bar_empty); bar_width];

                for &edit_idx in &track.edit_indices {
                    let col = self.edit_to_col(edit_idx, bar_width, total_edits);
                    if col < bar_width {
                        let color = self.agent_color_for_edit(edit_idx).unwrap_or(t.bar_filled);
                        bar_chars[col] = ('\u{2588}', color);
                    }
                }

                for (ch, color) in &bar_chars {
                    spans.push(Span::styled(
                        ch.to_string(),
                        Style::default().fg(*color),
                    ));
                }
            }

            Line::from(spans).render(
                Rect { x: area.x, y: row, width: area.width, height: 1 },
                buf,
            );
            row += 1;
        }

        // ── playhead line (full-height indicator) ────────────────────────
        if row < max_y && total_edits > 0 {
            let playhead_col = self.edit_to_col(self.app.playhead, bar_width, total_edits);

            let ph_color = if self.app.detached_files.is_empty() {
                t.accent_warm
            } else {
                t.accent_purple
            };

            // Draw playhead on all track rows as a vertical line
            let ph_x = area.x + name_and_sep as u16 + playhead_col as u16;
            if ph_x < area.x + area.width {
                for y in track_start_row..row {
                    if let Some(cell) = buf.cell_mut((ph_x, y)) {
                        cell.set_fg(ph_color);
                        cell.set_symbol("\u{2502}"); // thin vertical line
                    }
                }
            }

            // Playhead position row with timestamp
            let mut ph_spans: Vec<Span> = vec![
                Span::raw(format!("{:<width$}", "", width = TRACK_NAME_WIDTH)),
                Span::raw(SEPARATOR),
            ];

            for cell in 0..bar_width {
                let ch = if cell == playhead_col { "\u{2502}" } else { "\u{2500}" };
                ph_spans.push(Span::styled(ch, Style::default().fg(ph_color)));
            }

            // Timestamp label next to playhead
            if let Some(edit) = self.app.edits.get(self.app.playhead) {
                let ts_label = format!(" {}", self.format_time(edit.ts));
                ph_spans.push(Span::styled(ts_label, Style::default().fg(t.fg_dim)));
            }

            Line::from(ph_spans).render(
                Rect { x: area.x, y: row, width: area.width, height: 1 },
                buf,
            );
            row += 1;
        }

        // ── agent legend (only when multiple agents) ─────────────────────
        if row < max_y {
            let mut agents: Vec<(String, Color)> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for edit in &self.app.edits {
                if let Some(ref agent_id) = edit.agent_id {
                    if seen.insert(agent_id.clone()) {
                        let label = edit.agent_label.as_deref().unwrap_or(agent_id.as_str());
                        let hash: usize = agent_id.bytes().map(|b| b as usize).sum();
                        let color = t.agent_colors[hash % t.agent_colors.len()];
                        agents.push((label.to_string(), color));
                    }
                }
            }

            if agents.len() > 1 {
                let mut legend_spans: Vec<Span> = vec![
                    Span::styled(
                        format!("{:<width$}", "agents", width = TRACK_NAME_WIDTH),
                        Style::default().fg(t.fg_dim),
                    ),
                    Span::raw(SEPARATOR),
                ];
                for (i, (label, color)) in agents.iter().enumerate() {
                    if i > 0 {
                        legend_spans.push(Span::styled(" ", Style::default()));
                    }
                    legend_spans.push(Span::styled("\u{2588}", Style::default().fg(*color)));
                    legend_spans.push(Span::styled(
                        format!(" {}", label),
                        Style::default().fg(t.fg_muted),
                    ));
                }
                Line::from(legend_spans).render(
                    Rect { x: area.x, y: row, width: area.width, height: 1 },
                    buf,
                );
            }
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/widgets/timeline.rs src/tui/layout.rs
git commit -m "feat: rewrite timeline with time scale, clip segments, zoom, full-height playhead, agent legend"
```

---

## Task 6: Update status bar with edit timestamp and scrubbing indicator

**Files:**
- Modify: `src/tui/widgets/status_bar.rs`

- [ ] **Step 1: Add edit timestamp and preview mode indicator to status bar**

In the `render` method, after the agent label section (around line 103), add:

```rust
        // Edit timestamp
        if let Some(edit) = self.app.current_edit() {
            let session_start_ms = self.app.session_start * 1000;
            let offset_secs = ((edit.ts - session_start_ms).max(0) / 1000) as u64;
            let m = offset_secs / 60;
            let s = offset_secs % 60;
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("@{}m{:02}s", m, s),
                Style::default().fg(color_value),
            ));

            // Edit number
            left_spans.push(Span::styled(
                format!(" #{}/{}", self.app.playhead + 1, self.app.edits.len()),
                Style::default().fg(color_default),
            ));
        }

        // Preview mode indicator
        if self.app.preview_mode == crate::tui::app::PreviewMode::Diff {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled("diff", Style::default().fg(color_accent)));
        }
```

- [ ] **Step 2: Verify it compiles and tests pass**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets/status_bar.rs
git commit -m "feat: add edit timestamp, position counter, and preview mode to status bar"
```

---

## Task 7: Frame-rate-aware rendering and smooth scroll animation

**Files:**
- Modify: `src/tui/event_loop.rs`

- [ ] **Step 1: Switch to adaptive poll interval**

Replace the fixed 100ms poll with an adaptive interval based on playback state:

```rust
    // At the top of the loop:
    let poll_duration = match &app.playback {
        PlaybackState::Playing { .. } => Duration::from_millis(16),  // ~60fps
        _ => Duration::from_millis(100),                              // idle
    };
```

Then use `poll_duration` in the `ct_event::poll()` call:

```rust
    if ct_event::poll(poll_duration)? {
```

- [ ] **Step 2: Add smooth scroll interpolation**

After input handling and before rendering, add exponential decay interpolation:

```rust
    // Smooth scroll interpolation
    if app.preview_scroll != app.preview_scroll_target {
        let diff = app.preview_scroll_target as f64 - app.preview_scroll as f64;
        let step = (diff * 0.15).round() as isize;
        if step.unsigned_abs() < 1 {
            // Close enough -- snap to target
            app.preview_scroll = app.preview_scroll_target;
        } else {
            app.preview_scroll = (app.preview_scroll as isize + step).max(0) as usize;
        }
    }
```

- [ ] **Step 3: Add frame-rate-aware playback advancement**

Replace the implicit single-step-per-frame playback with wall-clock-based advancement. Add a `last_play_advance` timestamp:

```rust
    let mut last_play_advance = std::time::Instant::now();

    // In the loop, after input handling:
    if let PlaybackState::Playing { speed } = &app.playback {
        let interval = Duration::from_millis(500 / (*speed as u64).max(1));
        if last_play_advance.elapsed() >= interval {
            app.scrub_right();
            last_play_advance = std::time::Instant::now();
        }
    }
```

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/event_loop.rs
git commit -m "feat: adaptive frame rate, smooth scroll interpolation, wall-clock playback"
```

---

## Task 8: Enable mouse events for timeline zoom

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/event_loop.rs`

- [ ] **Step 1: Enable mouse capture in terminal setup**

In `src/tui/mod.rs`, in the `run_tui_with_options` function, after `execute!(stdout, EnterAlternateScreen)?;`, add:

```rust
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
```

And before `execute!(terminal.backend_mut(), LeaveAlternateScreen)?;`, add:

```rust
    execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture)?;
```

- [ ] **Step 2: Handle mouse scroll events in the event loop**

In `src/tui/event_loop.rs`, in the event matching section, add a handler for mouse events:

```rust
                Event::Mouse(mouse) => {
                    use crossterm::event::{MouseEventKind, MouseButton};
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            // Zoom timeline in
                            app.timeline_zoom = (app.timeline_zoom * 1.2).min(20.0);
                        }
                        MouseEventKind::ScrollDown => {
                            // Zoom timeline out
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

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs src/tui/event_loop.rs
git commit -m "feat: enable mouse scroll for timeline zoom"
```

---

## Task 9: Final integration pass and manual verification

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: All tests pass (104+ including new syntax module tests).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -W clippy::all`
Expected: No warnings.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues. If any, run `cargo fmt` and commit.

- [ ] **Step 4: Build release binary and check size**

Run: `cargo build --release && ls -la target/release/vibetracer`
Expected: Binary exists. Note size increase from syntect (~2MB is acceptable).

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore: clippy fixes, fmt cleanup for NLE polish"
```
