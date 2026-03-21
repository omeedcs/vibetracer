use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

const COLOR_HEADER: Color = Color::Rgb(138, 143, 152);
const COLOR_HEADER_VALUE: Color = Color::Rgb(160, 168, 183);
const COLOR_INTENT: Color = Color::Rgb(188, 160, 100);
const COLOR_ADD: Color = Color::Rgb(90, 158, 111);
const COLOR_REMOVE: Color = Color::Rgb(158, 90, 90);
const COLOR_HUNK: Color = Color::Rgb(90, 122, 158);
const COLOR_EMPTY: Color = Color::Rgb(90, 101, 119);

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

/// A widget that renders the diff preview for the current edit.
pub struct PreviewPane<'a> {
    pub app: &'a App,
}

impl<'a> PreviewPane<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl Widget for PreviewPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let edit = match self.app.current_edit() {
            Some(e) => e,
            None => {
                // Empty state.
                render_at(
                    Line::from(vec![Span::styled(
                        "no edits yet",
                        Style::default().fg(COLOR_EMPTY),
                    )]),
                    area,
                    area.y,
                    buf,
                );
                return;
            }
        };

        let max_y = area.y + area.height;
        let mut row = area.y;

        // Header: "edit #{id} {filename}"
        render_at(
            Line::from(vec![
                Span::styled("edit #", Style::default().fg(COLOR_HEADER)),
                Span::styled(edit.id.to_string(), Style::default().fg(COLOR_HEADER_VALUE)),
                Span::styled("  ", Style::default().fg(COLOR_HEADER)),
                Span::styled(edit.file.clone(), Style::default().fg(COLOR_HEADER_VALUE)),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        // Intent line (if available).
        if row < max_y {
            if let Some(intent) = &edit.intent {
                render_at(
                    Line::from(vec![
                        Span::styled("intent: ", Style::default().fg(COLOR_INTENT)),
                        Span::styled(intent.clone(), Style::default().fg(COLOR_INTENT)),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
        }

        // Diff lines.
        for diff_line in edit.patch.lines() {
            if row >= max_y {
                break;
            }
            let color = if diff_line.starts_with('+') {
                COLOR_ADD
            } else if diff_line.starts_with('-') {
                COLOR_REMOVE
            } else if diff_line.starts_with("@@") {
                COLOR_HUNK
            } else {
                Color::Reset
            };

            render_at(
                Line::from(vec![Span::styled(
                    diff_line.to_string(),
                    Style::default().fg(color),
                )]),
                area,
                row,
                buf,
            );
            row += 1;
        }

        // Footer: "+{added} -{removed}"
        if row < max_y {
            render_at(
                Line::from(vec![
                    Span::styled(
                        format!("+{}", edit.lines_added),
                        Style::default().fg(COLOR_ADD),
                    ),
                    Span::styled("  ", Style::default().fg(COLOR_HEADER)),
                    Span::styled(
                        format!("-{}", edit.lines_removed),
                        Style::default().fg(COLOR_REMOVE),
                    ),
                ]),
                area,
                row,
                buf,
            );
        }
    }
}
