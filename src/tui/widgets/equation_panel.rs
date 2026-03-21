use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::equation::detect::DetectedEquation;

const COLOR_HEADER: Color = Color::Rgb(188, 140, 255);
const COLOR_DEFAULT: Color = Color::Rgb(160, 168, 183);
const COLOR_DIM: Color = Color::Rgb(58, 62, 71);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);
const COLOR_LINE_NUM: Color = Color::Rgb(90, 101, 119);

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

/// Equation panel showing detected equations with line numbers.
pub struct EquationPanel<'a> {
    pub equations: &'a [DetectedEquation],
    pub selected: Option<usize>,
}

impl<'a> EquationPanel<'a> {
    pub fn new(equations: &'a [DetectedEquation], selected: Option<usize>) -> Self {
        Self { equations, selected }
    }
}

impl<'a> Widget for EquationPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Header: "EQUATIONS"
        render_at(
            Line::from(vec![Span::styled(
                "EQUATIONS",
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

        if self.equations.is_empty() {
            if row < max_y {
                render_at(
                    Line::from(vec![Span::styled(
                        "  no equations detected",
                        Style::default().fg(COLOR_DIM),
                    )]),
                    area,
                    row,
                    buf,
                );
            }
            return;
        }

        for (idx, eq) in self.equations.iter().enumerate() {
            if row >= max_y {
                return;
            }

            let is_selected = self.selected == Some(idx);

            let line_num_span = Span::styled(
                format!("L{}: ", eq.line),
                Style::default().fg(COLOR_LINE_NUM),
            );

            let latex_style = if is_selected {
                Style::default()
                    .fg(COLOR_HEADER)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DEFAULT)
            };

            let latex_span = Span::styled(eq.latex.clone(), latex_style);

            render_at(
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    line_num_span,
                    latex_span,
                ]),
                area,
                row,
                buf,
            );
            row += 1;
        }
    }
}
