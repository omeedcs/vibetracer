use crate::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::sentinels::SentinelViolation;

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
    pub theme: &'a Theme,
}

impl<'a> SentinelPanel<'a> {
    pub fn new(violations: &'a [SentinelViolation], theme: &'a Theme) -> Self {
        Self { violations, theme }
    }
}

impl Widget for SentinelPanel<'_> {
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

        // Header: "SENTINEL"
        render_at(
            Line::from(vec![Span::styled(
                "SENTINEL",
                Style::default().fg(self.theme.accent_red),
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

        if self.violations.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  no violations",
                        Style::default().fg(self.theme.fg_dim),
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
                    Span::styled("| ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled(
                        violation.rule_name.clone(),
                        Style::default().fg(self.theme.accent_red),
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
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled(
                        violation.description.clone(),
                        Style::default().fg(self.theme.fg),
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
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled("a: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(
                        violation.value_a.clone(),
                        Style::default().fg(self.theme.fg),
                    ),
                    Span::styled("  b: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(
                        violation.value_b.clone(),
                        Style::default().fg(self.theme.fg),
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
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled("assert: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(
                        violation.assertion.clone(),
                        Style::default().fg(self.theme.fg),
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
                    Line::from(vec![Span::styled(
                        "|",
                        Style::default().fg(self.theme.fg_dim),
                    )]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
        }
    }
}
