use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::refactor_tracker::RenameStatus;

const COLOR_HEADER: Color = Color::Rgb(90, 101, 119);
const COLOR_DEFAULT: Color = Color::Rgb(160, 168, 183);
const COLOR_DIM: Color = Color::Rgb(58, 62, 71);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);
const COLOR_PROGRESS_FILLED: Color = Color::Rgb(90, 158, 111);

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

/// Refactor panel showing rename propagation status.
pub struct RefactorPanel<'a> {
    pub rename_status: Option<&'a RenameStatus>,
}

impl<'a> RefactorPanel<'a> {
    pub fn new(rename_status: Option<&'a RenameStatus>) -> Self {
        Self { rename_status }
    }
}

impl Widget for RefactorPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "REFACTOR"
        render_at(
            Line::from(vec![Span::styled(
                "REFACTOR",
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

        let status = match self.rename_status {
            Some(s) => s,
            None => {
                if row < max_y {
                    render_at(
                        Line::from(vec![Span::styled(
                            "  no active rename",
                            Style::default().fg(COLOR_DIM),
                        )]),
                        area,
                        row,
                        buf,
                    );
                }
                return;
            }
        };

        // Rename: "old_name -> new_name"
        if row >= max_y {
            return;
        }
        render_at(
            Line::from(vec![
                Span::styled(status.old_name.clone(), Style::default().fg(COLOR_DEFAULT)),
                Span::styled(" -> ", Style::default().fg(COLOR_DIM)),
                Span::styled(
                    status.new_name.clone(),
                    Style::default().fg(COLOR_PROGRESS_FILLED),
                ),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        if row >= max_y {
            return;
        }

        // Progress bar
        let bar_width = (area.width as usize).saturating_sub(4).max(1);
        let filled = if status.total_sites > 0 {
            (status.updated_new_refs * bar_width) / status.total_sites
        } else {
            bar_width
        };
        let empty = bar_width.saturating_sub(filled);

        let mut bar_spans = vec![Span::styled("  [", Style::default().fg(COLOR_DIM))];
        if filled > 0 {
            bar_spans.push(Span::styled(
                "\u{2588}".repeat(filled),
                Style::default().fg(COLOR_PROGRESS_FILLED),
            ));
        }
        if empty > 0 {
            bar_spans.push(Span::styled(
                "\u{2591}".repeat(empty),
                Style::default().fg(COLOR_DIM),
            ));
        }
        bar_spans.push(Span::styled("]", Style::default().fg(COLOR_DIM)));

        render_at(Line::from(bar_spans), area, row, buf);
        row += 1;

        if row >= max_y {
            return;
        }

        // Progress text
        render_at(
            Line::from(vec![Span::styled(
                format!(
                    "  {}/{} sites updated",
                    status.updated_new_refs, status.total_sites
                ),
                Style::default().fg(COLOR_DIM),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        // Remaining files list
        if !status.remaining_files.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  remaining:",
                        Style::default().fg(COLOR_DIM),
                    )]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
            for file in &status.remaining_files {
                if row >= max_y {
                    return;
                }
                render_at(
                    Line::from(vec![
                        Span::styled("    - ", Style::default().fg(COLOR_DIM)),
                        Span::styled(file.clone(), Style::default().fg(COLOR_DEFAULT)),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
        }
    }
}
