use crate::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::blast_radius::DependencyStatus;

/// Render a single line at (area.x, y) if y < area.y + area.height.
fn render_at(line: Line, area: Rect, y: u16, buf: &mut Buffer) {
    if y >= area.y + area.height {
        return;
    }
    line.render(
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
        buf,
    );
}

/// Blast Radius panel showing dependency staleness for a source file.
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

impl Widget for BlastRadiusPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "BLAST RADIUS"
        render_at(
            Line::from(vec![Span::styled(
                "BLAST RADIUS",
                Style::default().fg(self.theme.accent_warm),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        if row >= max_y {
            return;
        }

        // Separator
        render_at(
            Line::from(vec![Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(self.theme.separator),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        if row >= max_y {
            return;
        }

        // Subheader: "{source_file} changed — N dependents:"
        let total =
            self.status.stale.len() + self.status.updated.len() + self.status.untouched.len();
        render_at(
            Line::from(vec![Span::styled(
                format!("{} changed -- {} dependents:", self.source_file, total),
                Style::default().fg(self.theme.fg),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        // Stale files: "!" prefix + muted red
        for file in &self.status.stale {
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("  ! ", Style::default().fg(self.theme.accent_red)),
                    Span::styled(file.clone(), Style::default().fg(self.theme.accent_red)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;
        }

        // Updated files: "." prefix + muted green
        for file in &self.status.updated {
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("  . ", Style::default().fg(self.theme.accent_green)),
                    Span::styled(file.clone(), Style::default().fg(self.theme.accent_green)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;
        }

        // Untouched files: "-" prefix + dim
        for file in &self.status.untouched {
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("  - ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled(file.clone(), Style::default().fg(self.theme.fg_dim)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;
        }

        // Empty state
        if total == 0 && row < max_y {
            render_at(
                Line::from(vec![Span::styled(
                    "  no dependents declared",
                    Style::default().fg(self.theme.fg_dim),
                )]),
                area,
                row,
                buf,
            );
        }
    }
}
