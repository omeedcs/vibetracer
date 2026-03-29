use crate::theme::Theme;
use crate::tui::bookmarks::Bookmark;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

const MAX_WIDTH: u16 = 50;
const MAX_VISIBLE: usize = 12;

/// Popup overlay widget that shows all bookmarks and lets the user select one.
pub struct BookmarkListWidget<'a> {
    pub bookmarks: &'a [Bookmark],
    pub selected: usize,
    pub theme: &'a Theme,
}

impl<'a> BookmarkListWidget<'a> {
    pub fn new(bookmarks: &'a [Bookmark], selected: usize, theme: &'a Theme) -> Self {
        Self {
            bookmarks,
            selected,
            theme,
        }
    }
}

impl Widget for BookmarkListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme;

        // ── dimensions ──────────────────────────────────────────────
        let width = MAX_WIDTH.min(area.width.saturating_sub(10));
        let entry_count = self.bookmarks.len().min(MAX_VISIBLE);
        // title(1) + separator(1) + entries + separator(1) + keybindings(1) + block borders(2)
        let content_height = 1 + 1 + entry_count as u16 + 1 + 1;
        let height = (content_height + 2).min(area.height);

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
            .title(" bookmarks ")
            .borders(Borders::ALL)
            .style(Style::default().bg(th.bg).fg(th.separator));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut row_y = inner.y;
        let max_y = inner.y + inner.height;

        // ── title ──────────────────────────────────────────────────
        if row_y < max_y {
            let title = Line::from(Span::styled(
                " BOOKMARKS",
                Style::default().fg(th.fg).bg(th.bg),
            ));
            title.render(
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

        // ── separator ─────────────────────────────────────────────
        if row_y < max_y {
            let sep: String = "\u{2500}".repeat(inner.width as usize);
            let line = Line::from(Span::styled(
                sep,
                Style::default().fg(th.separator).bg(th.bg),
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

        // ── empty state ───────────────────────────────────────────
        if self.bookmarks.is_empty() {
            if row_y < max_y {
                let msg = Line::from(Span::styled(
                    "  no bookmarks yet  (M to create)",
                    Style::default().fg(th.fg_muted).bg(th.bg),
                ));
                msg.render(
                    Rect {
                        x: inner.x,
                        y: row_y,
                        width: inner.width,
                        height: 1,
                    },
                    buf,
                );
            }
            return;
        }

        // ── compute scroll window ─────────────────────────────────
        let total = self.bookmarks.len();
        let visible = total.min(MAX_VISIBLE);
        let start = if total <= visible {
            0
        } else if self.selected < visible / 2 {
            0
        } else if self.selected + visible / 2 >= total {
            total.saturating_sub(visible)
        } else {
            self.selected.saturating_sub(visible / 2)
        };
        let end = (start + visible).min(total);

        // ── bookmark entries ──────────────────────────────────────
        for i in start..end {
            if row_y >= max_y {
                break;
            }

            let bm = &self.bookmarks[i];
            let is_selected = i == self.selected;

            let bg = if is_selected { th.separator } else { th.bg };
            let fg = th.fg;

            let prefix = if is_selected { " > " } else { "   " };
            let index_str = format!("#{:<5}", bm.edit_index);

            // Truncate label to fit
            let prefix_len = 3; // " > " or "   "
            let index_len = index_str.len();
            let padding = 2;
            let label_max = (inner.width as usize)
                .saturating_sub(prefix_len)
                .saturating_sub(index_len)
                .saturating_sub(padding);
            let label = truncate_str(&bm.label, label_max);

            let right_pad = (inner.width as usize)
                .saturating_sub(prefix_len)
                .saturating_sub(index_len)
                .saturating_sub(label.len())
                .saturating_sub(1);

            let line = Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default()
                        .fg(if is_selected { th.accent_warm } else { th.fg_dim })
                        .bg(bg),
                ),
                Span::styled(index_str, Style::default().fg(th.fg_muted).bg(bg)),
                Span::styled(
                    format!(" {}", label),
                    Style::default().fg(fg).bg(bg),
                ),
                Span::styled(" ".repeat(right_pad), Style::default().bg(bg)),
            ]);

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

        // ── bottom separator ──────────────────────────────────────
        if row_y < max_y {
            let sep: String = "\u{2500}".repeat(inner.width as usize);
            let line = Line::from(Span::styled(
                sep,
                Style::default().fg(th.separator).bg(th.bg),
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

        // ── keybindings hint ──────────────────────────────────────
        if row_y < max_y {
            let hint = Line::from(vec![
                Span::styled(" Enter", Style::default().fg(th.fg).bg(th.bg)),
                Span::styled(":jump  ", Style::default().fg(th.fg_muted).bg(th.bg)),
                Span::styled("d", Style::default().fg(th.fg).bg(th.bg)),
                Span::styled(":delete  ", Style::default().fg(th.fg_muted).bg(th.bg)),
                Span::styled("Esc", Style::default().fg(th.fg).bg(th.bg)),
                Span::styled(":close", Style::default().fg(th.fg_muted).bg(th.bg)),
            ]);
            hint.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}

/// Truncate a string to at most `max_len` characters, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
