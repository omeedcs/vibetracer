use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::blast_radius::DependencyStatus;

const COLOR_HEADER: Color = Color::Rgb(138, 117, 96);
const COLOR_DEFAULT: Color = Color::Rgb(160, 168, 183);
const COLOR_DIM: Color = Color::Rgb(58, 62, 71);
const COLOR_STALE: Color = Color::Rgb(158, 90, 90);
const COLOR_UPDATED: Color = Color::Rgb(90, 158, 111);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);

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
}

impl<'a> BlastRadiusPanel<'a> {
    pub fn new(source_file: &'a str, status: &'a DependencyStatus) -> Self {
        Self {
            source_file,
            status,
        }
    }
}

impl Widget for BlastRadiusPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "BLAST RADIUS"
        render_at(
            Line::from(vec![Span::styled(
                "BLAST RADIUS",
                Style::default().fg(COLOR_HEADER),
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
                Style::default().fg(COLOR_SEPARATOR),
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
                Style::default().fg(COLOR_DEFAULT),
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
                    Span::styled("  ! ", Style::default().fg(COLOR_STALE)),
                    Span::styled(file.clone(), Style::default().fg(COLOR_STALE)),
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
                    Span::styled("  . ", Style::default().fg(COLOR_UPDATED)),
                    Span::styled(file.clone(), Style::default().fg(COLOR_UPDATED)),
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
                    Span::styled("  - ", Style::default().fg(COLOR_DIM)),
                    Span::styled(file.clone(), Style::default().fg(COLOR_DIM)),
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
                    Style::default().fg(COLOR_DIM),
                )]),
                area,
                row,
                buf,
            );
        }
    }
}
