use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

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

/// A widget that renders the preview pane, delegating to either the file view
/// (syntax-highlighted source) or the diff view based on `app.preview_mode`.
pub struct PreviewPane<'a> {
    pub app: &'a App,
    pub file_content: Option<(&'a str, &'a str)>, // (content, filename)
    pub highlighter: Option<&'a crate::tui::syntax::Highlighter>,
    pub changed_lines: &'a std::collections::HashSet<usize>,
}

impl<'a> PreviewPane<'a> {
    pub fn new(
        app: &'a App,
        file_content: Option<(&'a str, &'a str)>,
        highlighter: Option<&'a crate::tui::syntax::Highlighter>,
        changed_lines: &'a std::collections::HashSet<usize>,
    ) -> Self {
        Self {
            app,
            file_content,
            highlighter,
            changed_lines,
        }
    }
}

impl PreviewPane<'_> {
    /// Render the existing diff view (unified diff with colored +/- lines).
    fn render_diff(&self, area: Rect, buf: &mut Buffer) {
        let edit = match self.app.current_edit() {
            Some(e) => e,
            None => {
                render_empty_state(area, buf, &self.app.theme);
                return;
            }
        };

        let t = &self.app.theme;
        let color_header_value: Color = t.fg;
        let color_intent: Color = t.accent_warm;
        let color_add: Color = t.accent_green;
        let color_remove: Color = t.accent_red;
        let color_hunk: Color = t.accent_blue;

        let max_y = area.y + area.height;
        let mut row = area.y;

        // Header: merged format with filename, diff label, and line counts
        render_at(
            Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(edit.file.clone(), Style::default().fg(color_header_value).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::styled(" \u{2502} ", Style::default().fg(t.separator)),
                Span::styled("diff", Style::default().fg(t.accent_warm)),
                Span::styled(" \u{2502} ", Style::default().fg(t.separator)),
                Span::styled(format!("+{}", edit.lines_added), Style::default().fg(color_add)),
                Span::styled(" ", Style::default()),
                Span::styled(format!("-{}", edit.lines_removed), Style::default().fg(color_remove)),
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
                        Span::styled("intent: ", Style::default().fg(color_intent)),
                        Span::styled(intent.clone(), Style::default().fg(color_intent)),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }
        }

        // Diff lines with line numbers.
        let mut old_line: usize = 0;
        let mut new_line: usize = 0;

        for diff_line in edit.patch.lines() {
            if row >= max_y {
                break;
            }

            if diff_line.starts_with("@@") {
                if let Some(minus_pos) = diff_line.find('-') {
                    let after_minus = &diff_line[minus_pos + 1..];
                    let num_str: String = after_minus.chars().take_while(|c| c.is_ascii_digit()).collect();
                    old_line = num_str.parse::<usize>().unwrap_or(0);
                }
                if let Some(plus_pos) = diff_line.find('+') {
                    let after_plus = &diff_line[plus_pos + 1..];
                    let num_str: String = after_plus.chars().take_while(|c| c.is_ascii_digit()).collect();
                    new_line = num_str.parse::<usize>().unwrap_or(0);
                }

                render_at(
                    Line::from(vec![
                        Span::styled(format!("{:>5} ", ""), Style::default().fg(t.fg_dim)),
                        Span::styled(diff_line.to_string(), Style::default().fg(color_hunk)),
                    ]),
                    area, row, buf,
                );
                row += 1;
                continue;
            }

            let (gutter, color) = if diff_line.starts_with('+') {
                let g = format!("{:>5} ", new_line);
                new_line += 1;
                (g, color_add)
            } else if diff_line.starts_with('-') {
                let g = format!("{:>5} ", old_line);
                old_line += 1;
                (g, color_remove)
            } else {
                let g = format!("{:>5} ", new_line);
                old_line += 1;
                new_line += 1;
                (g, Color::Reset)
            };

            render_at(
                Line::from(vec![
                    Span::styled(gutter, Style::default().fg(t.fg_dim)),
                    Span::styled(diff_line.to_string(), Style::default().fg(color)),
                ]),
                area, row, buf,
            );
            row += 1;
        }
    }
}

impl Widget for PreviewPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        if self.app.current_edit().is_none() {
            render_empty_state(area, buf, &self.app.theme);
            return;
        }

        match self.app.preview_mode {
            crate::tui::app::PreviewMode::File => {
                if let (Some((content, filename)), Some(highlighter)) =
                    (self.file_content, self.highlighter)
                {
                    super::file_view::FileView::new(
                        self.app,
                        content,
                        filename,
                        highlighter,
                        self.changed_lines,
                    )
                    .render(area, buf);
                } else {
                    render_empty_state(area, buf, &self.app.theme);
                }
            }
            crate::tui::app::PreviewMode::Diff => {
                self.render_diff(area, buf);
            }
        }
    }
}

/// Render the empty/welcome state when no edits have been tracked yet.
fn render_empty_state(area: Rect, buf: &mut Buffer, theme: &crate::theme::Theme) {
    if area.height < 5 || area.width < 30 {
        return;
    }

    let logo = [
        r"       _ _          _                          ",
        r"__   _(_) |__   ___| |_ _ __ __ _  ___ ___ _ __",
        r"\ \ / / | '_ \ / _ \ __| '__/ _` |/ __/ _ \ '__|",
        r" \ V /| | |_) |  __/ |_| | | (_| | (_|  __/ |  ",
        r"  \_/ |_|_.__/ \___|\__|_|  \__,_|\___\___|_|  ",
    ];

    let hints = [
        ("", ""),
        ("waiting for edits", ""),
        ("", ""),
        ("start coding in another pane", "vibetracer will"),
        ("track every change automatically", ""),
        ("", ""),
        ("left/right", "scrub through edits"),
        ("Space", "play / pause replay"),
        ("R", "restore to playhead"),
        ("c", "create checkpoint"),
        ("g", "toggle command view"),
        ("t", "cycle theme"),
        ("?", "all keybindings"),
    ];

    let color_warm = theme.accent_warm;
    let color_subtle = theme.fg_dim;
    let color_dim = theme.separator;
    let color_empty = theme.fg_muted;

    // Center vertically
    let total_height = logo.len() + 2 + hints.len();
    let start_y = area.y + area.height.saturating_sub(total_height as u16) / 2;

    // Render logo
    for (i, line) in logo.iter().enumerate() {
        let y = start_y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
        buf.set_string(x, y, *line, Style::default().fg(color_warm));
    }

    // Render hints
    let hints_start = start_y + logo.len() as u16 + 2;
    for (i, (key, desc)) in hints.iter().enumerate() {
        let y = hints_start + i as u16;
        if y >= area.y + area.height {
            break;
        }

        if key.is_empty() && desc.is_empty() {
            continue;
        }

        if desc.is_empty() {
            // It's a section label
            let x = area.x + area.width.saturating_sub(key.len() as u16) / 2;
            let color = if key.contains("waiting") {
                color_subtle
            } else {
                color_dim
            };
            buf.set_string(x, y, *key, Style::default().fg(color));
        } else {
            // Key + description
            let text = format!("{:>14}  {}", key, desc);
            let x = area.x + area.width.saturating_sub(text.len() as u16) / 2;
            // Render key part brighter, desc part dimmer
            buf.set_string(
                x,
                y,
                format!("{:>14}", key),
                Style::default().fg(color_empty),
            );
            buf.set_string(x + 16, y, *desc, Style::default().fg(color_subtle));
        }
    }
}
