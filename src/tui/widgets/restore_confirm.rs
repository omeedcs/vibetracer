use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

const MAX_WIDTH: u16 = 60;
const MAX_HEIGHT: u16 = 20;

/// A centered modal overlay for confirming a restore operation.
///
/// Shows the list of files that will be restored, any conflict warnings
/// for coupled files, and instructions to confirm or cancel.
pub struct RestoreConfirmDialog<'a> {
    /// (filename, change summary) pairs for each file being restored.
    pub files: &'a [(String, String)],
    /// Coupled file warnings (e.g. "api.rs may also need restoring").
    pub conflicts: &'a [String],
    /// Theme colors.
    pub bg: Color,
    pub fg: Color,
    pub fg_muted: Color,
    pub border: Color,
    pub accent_warn: Color,
    pub accent_green: Color,
}

impl<'a> RestoreConfirmDialog<'a> {
    pub fn new(files: &'a [(String, String)], conflicts: &'a [String]) -> Self {
        Self {
            files,
            conflicts,
            bg: Color::Rgb(15, 17, 21),
            fg: Color::Rgb(160, 168, 183),
            fg_muted: Color::Rgb(90, 101, 119),
            border: Color::Rgb(42, 46, 55),
            accent_warn: Color::Rgb(196, 120, 91),
            accent_green: Color::Rgb(90, 158, 111),
        }
    }

    /// Set colors from an App's theme.
    pub fn with_theme(mut self, theme: &crate::theme::Theme) -> Self {
        self.bg = theme.bg;
        self.fg = theme.fg;
        self.fg_muted = theme.fg_muted;
        self.border = theme.separator;
        self.accent_warn = theme.accent_warm;
        self.accent_green = theme.accent_green;
        self
    }
}

impl Widget for RestoreConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Compute dialog height: title(1) + border(2) + files + gap + conflicts + gap + instructions(1)
        let content_height = self.files.len() as u16
            + if self.conflicts.is_empty() {
                0
            } else {
                1 + self.conflicts.len() as u16
            }
            + 2; // blank line + instructions line
        let height = (content_height + 2).min(MAX_HEIGHT).min(area.height); // +2 for borders
        let width = MAX_WIDTH.min(area.width);

        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;

        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        // Clear the area.
        Clear.render(overlay_area, buf);

        // Draw bordered block.
        let block = Block::default()
            .title(" restore ")
            .borders(Borders::ALL)
            .style(Style::default().bg(self.bg).fg(self.border));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let mut row = inner.y;
        let max_y = inner.y + inner.height;

        // File list
        for (file, summary) in self.files {
            if row >= max_y {
                break;
            }
            let line = Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    truncate_str(file, (inner.width as usize).saturating_sub(4)),
                    Style::default().fg(self.fg),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(summary.as_str(), Style::default().fg(self.fg_muted)),
            ]);
            line.render(
                Rect {
                    x: inner.x,
                    y: row,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row += 1;
        }

        // Conflict warnings
        if !self.conflicts.is_empty() {
            if row < max_y {
                row += 1; // blank line
            }
            for conflict in self.conflicts {
                if row >= max_y {
                    break;
                }
                let line = Line::from(vec![
                    Span::styled("  ! ", Style::default().fg(self.accent_warn)),
                    Span::styled(conflict.as_str(), Style::default().fg(self.accent_warn)),
                ]);
                line.render(
                    Rect {
                        x: inner.x,
                        y: row,
                        width: inner.width,
                        height: 1,
                    },
                    buf,
                );
                row += 1;
            }
        }

        // Instructions
        if row < max_y {
            row += 1; // blank line
        }
        if row < max_y {
            let line = Line::from(vec![
                Span::styled("  Enter", Style::default().fg(self.accent_green)),
                Span::styled(" confirm   ", Style::default().fg(self.fg_muted)),
                Span::styled("Esc", Style::default().fg(self.fg_muted)),
                Span::styled(" cancel", Style::default().fg(self.fg_muted)),
            ]);
            line.render(
                Rect {
                    x: inner.x,
                    y: row,
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
