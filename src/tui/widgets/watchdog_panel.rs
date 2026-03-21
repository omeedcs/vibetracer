use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::analysis::watchdog::WatchdogAlert;

const COLOR_HEADER: Color = Color::Rgb(196, 120, 91);
const COLOR_DEFAULT: Color = Color::Rgb(160, 168, 183);
const COLOR_DIM: Color = Color::Rgb(58, 62, 71);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);
const COLOR_LABEL: Color = Color::Rgb(138, 117, 96);
const COLOR_CRITICAL: Color = Color::Rgb(220, 100, 80);

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
}

impl<'a> WatchdogPanel<'a> {
    pub fn new(alerts: &'a [WatchdogAlert]) -> Self {
        Self { alerts }
    }
}

impl Widget for WatchdogPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "CONSTANT MODIFIED"
        render_at(
            Line::from(vec![Span::styled(
                "CONSTANT MODIFIED",
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

        if self.alerts.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  no alerts",
                        Style::default().fg(COLOR_DIM),
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
                COLOR_CRITICAL
            } else {
                COLOR_DEFAULT
            };

            // File
            if row >= max_y {
                return;
            }
            render_at(
                Line::from(vec![
                    Span::styled("| ", Style::default().fg(COLOR_DIM)),
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
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled("pattern: ", Style::default().fg(COLOR_LABEL)),
                    Span::styled(
                        alert.constant_pattern.clone(),
                        Style::default().fg(COLOR_DIM),
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
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled("expected: ", Style::default().fg(COLOR_LABEL)),
                    Span::styled(alert.expected.clone(), Style::default().fg(COLOR_DEFAULT)),
                    Span::styled("  actual: ", Style::default().fg(COLOR_LABEL)),
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
                    Span::styled("|   ", Style::default().fg(COLOR_DIM)),
                    Span::styled("severity: ", Style::default().fg(COLOR_LABEL)),
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
