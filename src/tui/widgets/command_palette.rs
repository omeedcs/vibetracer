use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

const MAX_WIDTH: u16 = 60;
const MAX_VISIBLE: usize = 12;

// ---------------------------------------------------------------------------
// PaletteEntry
// ---------------------------------------------------------------------------

/// A single action that can be invoked from the command palette.
#[derive(Clone, Debug)]
pub struct PaletteEntry {
    /// Unique action identifier.
    pub id: String,
    /// Display name (e.g. "Toggle Blast Radius").
    pub label: String,
    /// Optional keybinding hint (e.g. "b").
    pub shortcut: Option<String>,
    /// Grouping category (e.g. "Navigation", "View", "Analysis").
    pub category: String,
}

// ---------------------------------------------------------------------------
// CommandPalette (state)
// ---------------------------------------------------------------------------

/// State for the command palette overlay.  Not a widget itself -- pass a
/// reference to [`CommandPaletteWidget`] for rendering.
pub struct CommandPalette {
    /// Whether the palette is currently shown.
    pub visible: bool,
    /// Current search text.
    pub input: String,
    /// All registered palette entries.
    pub entries: Vec<PaletteEntry>,
    /// Indices into `entries` that match the current input.
    pub filtered: Vec<usize>,
    /// Index into `filtered` for the currently highlighted entry.
    pub selected: usize,
    /// Most-recently-used entry IDs (front = most recent).
    pub recent: Vec<String>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            input: String::new(),
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            recent: Vec::new(),
        }
    }

    /// Show the palette, clearing any previous search state.
    pub fn open(&mut self) {
        self.visible = true;
        self.input.clear();
        self.selected = 0;
        self.filter();
    }

    /// Hide the palette.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Replace the search text entirely and refilter.
    pub fn set_input(&mut self, input: String) {
        self.input = input;
        self.selected = 0;
        self.filter();
    }

    /// Append a character to the search text and refilter.
    pub fn push_char(&mut self, c: char) {
        self.input.push(c);
        self.selected = 0;
        self.filter();
    }

    /// Remove the last character from the search text and refilter.
    pub fn pop_char(&mut self) {
        self.input.pop();
        self.selected = 0;
        self.filter();
    }

    /// Move selection up by one.
    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down by one.
    pub fn select_down(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    /// Confirm the currently selected entry.  Returns its `id`, records it as
    /// recently used, and closes the palette.  Returns `None` when there is no
    /// selection.
    pub fn confirm(&mut self) -> Option<String> {
        let idx = *self.filtered.get(self.selected)?;
        let id = self.entries[idx].id.clone();

        // Update MRU -- move to front, dedup.
        self.recent.retain(|r| r != &id);
        self.recent.insert(0, id.clone());

        self.close();
        Some(id)
    }

    /// Register a new palette entry.
    pub fn register(&mut self, entry: PaletteEntry) {
        self.entries.push(entry);
        // Keep filtered list in sync when entries change while invisible.
        if !self.visible {
            self.filter();
        }
    }

    /// Recompute `filtered` based on the current `input`.
    ///
    /// When the input is empty the full list is returned with recently-used
    /// entries floated to the top.  Otherwise a case-insensitive substring
    /// match is used against both label and category.
    pub fn filter(&mut self) {
        let query = self.input.to_lowercase();

        if query.is_empty() {
            // Start with MRU entries (preserving MRU order), then everything else.
            let mut seen = vec![false; self.entries.len()];
            let mut result: Vec<usize> = Vec::with_capacity(self.entries.len());

            for recent_id in &self.recent {
                if let Some(idx) = self.entries.iter().position(|e| &e.id == recent_id) {
                    if !seen[idx] {
                        seen[idx] = true;
                        result.push(idx);
                    }
                }
            }

            for idx in 0..self.entries.len() {
                if !seen[idx] {
                    result.push(idx);
                }
            }

            self.filtered = result;
        } else {
            self.filtered = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.label.to_lowercase().contains(&query)
                        || e.category.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Clamp selection.
        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandPaletteWidget (rendering)
// ---------------------------------------------------------------------------

/// Renders the [`CommandPalette`] state as a centered overlay.
pub struct CommandPaletteWidget<'a> {
    pub palette: &'a CommandPalette,
    pub theme_bg: Color,
    pub theme_fg: Color,
    pub theme_fg_dim: Color,
    pub theme_accent: Color,
    pub theme_separator: Color,
}

impl<'a> CommandPaletteWidget<'a> {
    pub fn new(palette: &'a CommandPalette) -> Self {
        Self {
            palette,
            theme_bg: Color::Rgb(15, 17, 21),
            theme_fg: Color::Rgb(160, 168, 183),
            theme_fg_dim: Color::Rgb(58, 62, 71),
            theme_accent: Color::Rgb(90, 122, 158),
            theme_separator: Color::Rgb(42, 46, 55),
        }
    }

    /// Apply colors from a [`Theme`](crate::theme::Theme).
    pub fn with_theme(mut self, theme: &crate::theme::Theme) -> Self {
        self.theme_bg = theme.bg;
        self.theme_fg = theme.fg;
        self.theme_fg_dim = theme.fg_dim;
        self.theme_accent = theme.accent_blue;
        self.theme_separator = theme.separator;
        self
    }
}

impl Widget for CommandPaletteWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.palette.visible {
            return;
        }

        let pal = self.palette;

        // ── dimensions ──────────────────────────────────────────────
        let width = MAX_WIDTH.min(area.width.saturating_sub(10));
        // 1 row for input + 1 separator + up to MAX_VISIBLE entries + category
        // headers.  Compute the actual row count from visible entries.
        let (visible_rows, row_meta) = self.build_row_list();
        // +3: border top/bottom + input line
        let content_height = 1 + 1 + visible_rows as u16; // input + separator + rows
        let height = (content_height + 2).min(area.height); // +2 for block borders

        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;

        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .title(" command palette ")
            .borders(Borders::ALL)
            .style(Style::default().bg(self.theme_bg).fg(self.theme_separator));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut row_y = inner.y;
        let max_y = inner.y + inner.height;

        // ── input line ──────────────────────────────────────────────
        if row_y < max_y {
            let cursor_char = if pal.input.is_empty() { "_" } else { "" };
            let prompt = Line::from(vec![
                Span::styled(
                    " > ",
                    Style::default().fg(self.theme_accent).bg(self.theme_bg),
                ),
                Span::styled(
                    pal.input.as_str(),
                    Style::default().fg(self.theme_fg).bg(self.theme_bg),
                ),
                Span::styled(
                    cursor_char,
                    Style::default().fg(self.theme_fg_dim).bg(self.theme_bg),
                ),
            ]);
            prompt.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;
        }

        // ── separator ──────────────────────────────────────────────
        if row_y < max_y {
            let sep: String = "\u{2500}".repeat(inner.width as usize);
            let line = Line::from(Span::styled(
                sep,
                Style::default().fg(self.theme_separator).bg(self.theme_bg),
            ));
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;
        }

        // ── entry rows ─────────────────────────────────────────────
        for meta in &row_meta {
            if row_y >= max_y {
                break;
            }
            let row_rect = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };

            match meta {
                RowMeta::CategoryHeader(cat) => {
                    let line = Line::from(Span::styled(
                        format!("  {cat}"),
                        Style::default().fg(self.theme_fg_dim).bg(self.theme_bg),
                    ));
                    line.render(row_rect, buf);
                }
                RowMeta::Entry {
                    entry_idx,
                    selected,
                } => {
                    let entry = &pal.entries[*entry_idx];
                    let bg = if *selected {
                        self.theme_separator
                    } else {
                        self.theme_bg
                    };
                    let fg = if *selected {
                        self.theme_fg
                    } else {
                        self.theme_fg
                    };

                    let shortcut_text = entry
                        .shortcut
                        .as_deref()
                        .unwrap_or("");
                    let label_max = (inner.width as usize)
                        .saturating_sub(4) // leading padding
                        .saturating_sub(shortcut_text.len())
                        .saturating_sub(2); // trailing padding

                    let display_label = truncate_str(&entry.label, label_max);
                    let padding = (inner.width as usize)
                        .saturating_sub(4)
                        .saturating_sub(display_label.len())
                        .saturating_sub(shortcut_text.len())
                        .saturating_sub(1);

                    let line = Line::from(vec![
                        Span::styled(
                            "  ",
                            Style::default().fg(fg).bg(bg),
                        ),
                        Span::styled(
                            if *selected { "> " } else { "  " },
                            Style::default().fg(self.theme_accent).bg(bg),
                        ),
                        Span::styled(
                            display_label,
                            Style::default().fg(fg).bg(bg),
                        ),
                        Span::styled(
                            " ".repeat(padding),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            shortcut_text.to_string(),
                            Style::default().fg(self.theme_fg_dim).bg(bg),
                        ),
                        Span::styled(
                            " ",
                            Style::default().bg(bg),
                        ),
                    ]);
                    line.render(row_rect, buf);
                }
            }
            row_y += 1;
        }
    }
}

impl<'a> CommandPaletteWidget<'a> {
    /// Build the visible row list: category headers interleaved with entries,
    /// scrolled so the selected item is visible.  Returns the total number of
    /// rendered rows and the row metadata.
    fn build_row_list(&self) -> (usize, Vec<RowMeta>) {
        let pal = self.palette;
        if pal.filtered.is_empty() {
            return (0, Vec::new());
        }

        // Build a flat list of (optional category header, entry index) keeping
        // category header only for the first entry of each group.
        let mut full_rows: Vec<RowMeta> = Vec::new();
        let mut last_category: Option<&str> = None;

        for (filt_pos, &entry_idx) in pal.filtered.iter().enumerate() {
            let entry = &pal.entries[entry_idx];
            let cat = entry.category.as_str();
            if last_category != Some(cat) {
                full_rows.push(RowMeta::CategoryHeader(cat.to_string()));
                last_category = Some(cat);
            }
            full_rows.push(RowMeta::Entry {
                entry_idx,
                selected: filt_pos == pal.selected,
            });
        }

        // Determine the row index of the selected entry.
        let selected_row = full_rows
            .iter()
            .position(|r| matches!(r, RowMeta::Entry { selected: true, .. }))
            .unwrap_or(0);

        // Scroll window so selected entry is visible within MAX_VISIBLE rows.
        let max_rows = MAX_VISIBLE + count_headers_in_window(&full_rows, MAX_VISIBLE);
        let total = full_rows.len();

        let start = if total <= max_rows {
            0
        } else if selected_row < max_rows / 2 {
            0
        } else if selected_row + max_rows / 2 >= total {
            total.saturating_sub(max_rows)
        } else {
            selected_row.saturating_sub(max_rows / 2)
        };

        let end = (start + max_rows).min(total);
        let visible = full_rows[start..end].to_vec();
        let count = visible.len();
        (count, visible)
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Metadata for a single rendered row.
#[derive(Clone, Debug)]
enum RowMeta {
    CategoryHeader(String),
    Entry { entry_idx: usize, selected: bool },
}

/// Estimate how many category headers appear in a window of `n` entry rows.
fn count_headers_in_window(rows: &[RowMeta], n: usize) -> usize {
    let mut entries_seen = 0;
    let mut headers = 0;
    for row in rows {
        if entries_seen >= n {
            break;
        }
        match row {
            RowMeta::CategoryHeader(_) => headers += 1,
            RowMeta::Entry { .. } => entries_seen += 1,
        }
    }
    headers
}

/// Truncate a string to at most `max_len` characters, adding "..." if
/// truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<PaletteEntry> {
        vec![
            PaletteEntry {
                id: "toggle_blast".into(),
                label: "Toggle Blast Radius".into(),
                shortcut: Some("b".into()),
                category: "View".into(),
            },
            PaletteEntry {
                id: "toggle_sentinel".into(),
                label: "Toggle Sentinels".into(),
                shortcut: Some("i".into()),
                category: "View".into(),
            },
            PaletteEntry {
                id: "goto_latest".into(),
                label: "Go to Latest".into(),
                shortcut: None,
                category: "Navigation".into(),
            },
            PaletteEntry {
                id: "restore_file".into(),
                label: "Restore File".into(),
                shortcut: Some("R".into()),
                category: "Actions".into(),
            },
        ]
    }

    #[test]
    fn filter_empty_input_returns_all() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        assert_eq!(cp.filtered.len(), 4);
    }

    #[test]
    fn filter_narrows_results() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        cp.set_input("blast".into());
        assert_eq!(cp.filtered.len(), 1);
        assert_eq!(cp.entries[cp.filtered[0]].id, "toggle_blast");
    }

    #[test]
    fn filter_is_case_insensitive() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        cp.set_input("TOGGLE".into());
        assert_eq!(cp.filtered.len(), 2);
    }

    #[test]
    fn confirm_returns_id_and_records_recent() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        let id = cp.confirm();
        assert!(id.is_some());
        assert!(!cp.visible);
        assert_eq!(cp.recent.len(), 1);
    }

    #[test]
    fn recent_entries_float_to_top() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        // Confirm "restore_file" first.
        cp.open();
        cp.selected = 3; // restore_file is 4th
        let id = cp.confirm().unwrap();
        assert_eq!(id, "restore_file");

        // Re-open: restore_file should be first.
        cp.open();
        assert_eq!(cp.entries[cp.filtered[0]].id, "restore_file");
    }

    #[test]
    fn select_up_down_clamps() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        cp.select_up();
        assert_eq!(cp.selected, 0);
        cp.select_down();
        cp.select_down();
        cp.select_down();
        cp.select_down();
        cp.select_down(); // past the end
        assert_eq!(cp.selected, 3);
    }

    #[test]
    fn push_and_pop_char() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        cp.push_char('r');
        cp.push_char('e');
        assert_eq!(cp.input, "re");
        // "Restore File" matches
        assert!(cp.filtered.iter().any(|&i| cp.entries[i].id == "restore_file"));

        cp.pop_char();
        assert_eq!(cp.input, "r");
        cp.pop_char();
        assert_eq!(cp.input, "");
        assert_eq!(cp.filtered.len(), 4);
    }

    #[test]
    fn confirm_on_empty_returns_none() {
        let mut cp = CommandPalette::new();
        cp.open();
        assert!(cp.confirm().is_none());
    }

    #[test]
    fn filter_matches_category() {
        let mut cp = CommandPalette::new();
        for e in sample_entries() {
            cp.register(e);
        }
        cp.open();
        cp.set_input("navigation".into());
        assert_eq!(cp.filtered.len(), 1);
        assert_eq!(cp.entries[cp.filtered[0]].id, "goto_latest");
    }
}
