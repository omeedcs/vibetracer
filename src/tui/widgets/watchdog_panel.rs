use crate::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::watchdog::WatchdogAlert;

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

/// Watchdog panel showing constant modification alerts.
pub struct WatchdogPanel<'a> {
    pub alerts: &'a [WatchdogAlert],
    pub theme: &'a Theme,
}

impl<'a> WatchdogPanel<'a> {
    pub fn new(alerts: &'a [WatchdogAlert], theme: &'a Theme) -> Self {
        Self { alerts, theme }
    }
}

impl Widget for WatchdogPanel<'_> {
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

        // Header: "CONSTANT MODIFIED"
        render_at(
            Line::from(vec![Span::styled(
                "CONSTANT MODIFIED",
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

        if self.alerts.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  no alerts",
                        Style::default().fg(self.theme.fg_dim),
                    )]),
                    area,
                    row,
                    buf,
                );
            }
            return;
        }

        for alert in self.alerts {
            let is_critical = alert.severity.to_lowercase() == "critical";
            let text_color = if is_critical {
                self.theme.accent_red
            } else {
                self.theme.fg
            };

            // File
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("| ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled(alert.file.clone(), Style::default().fg(text_color)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Pattern
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled("pattern: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(
                        alert.constant_pattern.clone(),
                        Style::default().fg(self.theme.fg_dim),
                    ),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Expected vs actual
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled("expected: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(alert.expected.clone(), Style::default().fg(self.theme.fg)),
                    Span::styled("  actual: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(alert.actual.clone(), Style::default().fg(text_color)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Severity
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("|   ", Style::default().fg(self.theme.fg_dim)),
                    Span::styled("severity: ", Style::default().fg(self.theme.accent_warm)),
                    Span::styled(alert.severity.clone(), Style::default().fg(text_color)),
                ]),
                area,
                row,
                buf,
            );
            row += 1;

            // Blank line between alerts
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
