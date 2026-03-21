use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::sentinels::SentinelViolation;

const COLOR_HEADER: Color = Color::Rgb(158, 90, 90);
const COLOR_DEFAULT: Color = Color::Rgb(160, 168, 183);
const COLOR_DIM: Color = Color::Rgb(58, 62, 71);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);
const COLOR_LABEL: Color = Color::Rgb(138, 117, 96);

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

/// Sentinel panel showing invariant violations.
pub struct SentinelPanel<'a> {
    pub violations: &'a [SentinelViolation],
}

impl<'a> SentinelPanel<'a> {
    pub fn new(violations: &'a [SentinelViolation]) -> Self {
        Self { violations }
    }
}

impl Widget for SentinelPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "SENTINEL"
        render_at(
            Line::from(vec![Span::styled(
                "SENTINEL",
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

        if self.violations.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  no violations",
                        Style::default().fg(COLOR_DIM),
                    )]),
                    area,
                    row,
                    buf,
                );
            }
            return;
        }

        for violation in self.violations {
            // Rule name
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("| ", Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        violation.rule_name.clone(),
                        Style::default().fg(COLOR_HEADER),
                    ),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Description
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        violation.description.clone(),
                        Style::default().fg(COLOR_DEFAULT),
                    ),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // value_a vs value_b
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled("a: ", Style::default().fg(COLOR_LABEL)),
                    Span::styled(
                        violation.value_a.clone(),
                        Style::default().fg(COLOR_DEFAULT),
                    ),
                    Span::styled("  b: ", Style::default().fg(COLOR_LABEL)),
                    Span::styled(
                        violation.value_b.clone(),
                        Style::default().fg(COLOR_DEFAULT),
                    ),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Assertion text
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled("assert: ", Style::default().fg(COLOR_LABEL)),
                    Span::styled(
                        violation.assertion.clone(),
                        Style::default().fg(COLOR_DEFAULT),
                    ),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Blank line between violations
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled("|", Style::default().fg(COLOR_DIM))]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
        }
    }
}
