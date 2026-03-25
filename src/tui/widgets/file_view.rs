use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::collections::HashSet;

use crate::event::EditKind;
use crate::theme::Theme;
use crate::tui::syntax::{HighlightedLine, Highlighter};
use crate::tui::App;

const GUTTER_WIDTH: u16 = 6;

/// A widget that renders syntax-highlighted file content at the current playhead
/// position, with line numbers, change markers, and scroll support.
pub struct FileView<'a> {
    pub app: &'a App,
    pub content: &'a str,
    pub filename: &'a str,
    pub highlighter: &'a Highlighter,
    pub changed_lines: &'a HashSet<usize>,
}

impl<'a> FileView<'a> {
    pub fn new(
        app: &'a App,
        content: &'a str,
        filename: &'a str,
        highlighter: &'a Highlighter,
        changed_lines: &'a HashSet<usize>,
    ) -> Self {
        Self {
            app,
            content,
            filename,
            highlighter,
            changed_lines,
        }
    }
}

impl Widget for FileView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let theme = &self.app.theme;

        // Handle deleted file state.
        if let Some(edit) = self.app.current_edit() {
            if edit.kind == EditKind::Delete {
                render_deleted_state(area, buf, theme, self.filename);
                return;
            }
        }

        // Handle empty content.
        if self.content.is_empty() {
            render_empty_file(area, buf, theme);
            return;
        }

        // Determine whether all lines should be treated as added.
        let all_added = if let Some(edit) = self.app.current_edit() {
            edit.kind == EditKind::Create || edit.before_hash.is_none()
        } else {
            false
        };

        // Highlight the content.
        let highlighted = self.highlighter.highlight(self.filename, self.content, theme);
        let total_lines = highlighted.len();

        // Reserve 1 row for header and 1 for footer.
        let header_rows: u16 = 1;
        let footer_rows: u16 = 1;
        let body_height = area.height.saturating_sub(header_rows + footer_rows) as usize;

        // -- Header --
        let header_line = Line::from(vec![Span::styled(
            format!(" {}", self.filename),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )]);
        header_line.render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        // -- Body: syntax-highlighted lines with gutter --
        let scroll = self.app.preview_scroll;
        let content_width = area.width.saturating_sub(GUTTER_WIDTH + 1); // +1 for separator space

        for row_idx in 0..body_height {
            let line_idx = scroll + row_idx; // 0-based index into the file
            let y = area.y + header_rows + row_idx as u16;

            if y >= area.y + area.height.saturating_sub(footer_rows) {
                break;
            }

            if line_idx >= total_lines {
                // Past end of file: render tilde in gutter.
                let gutter_span = Span::styled(
                    format!("{:>width$} ", "~", width = (GUTTER_WIDTH - 1) as usize),
                    Style::default().fg(theme.fg_dim),
                );
                Line::from(vec![gutter_span]).render(
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
                continue;
            }

            let line_num = line_idx + 1; // 1-based line number
            let is_changed = all_added || self.changed_lines.contains(&line_num);

            // Gutter: line number
            let gutter_color = if is_changed {
                theme.accent_green
            } else {
                theme.fg_dim
            };
            let gutter_text = format!("{:>width$} ", line_num, width = (GUTTER_WIDTH - 1) as usize);
            let gutter_span = Span::styled(gutter_text, Style::default().fg(gutter_color));

            // Build the content spans from highlighted segments.
            let hl_line: &HighlightedLine = &highlighted[line_idx];
            let bg_color = if is_changed {
                change_tint(theme.accent_green)
            } else {
                Color::Reset
            };

            let mut spans = vec![gutter_span];
            for seg in hl_line {
                let mut style = Style::default().fg(seg.fg);
                if is_changed {
                    style = style.bg(bg_color);
                }
                if seg.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if seg.italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(seg.text.clone(), style));
            }

            // If the line is changed, fill the remaining width with the tint background.
            if is_changed && content_width > 0 {
                let text_width: usize = hl_line.iter().map(|s| s.text.len()).sum();
                let remaining = (content_width as usize).saturating_sub(text_width);
                if remaining > 0 {
                    spans.push(Span::styled(
                        " ".repeat(remaining),
                        Style::default().bg(bg_color),
                    ));
                }
            }

            Line::from(spans).render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }

        // -- Footer --
        let footer_y = area.y + area.height - 1;
        let scroll_pct = if total_lines <= body_height {
            100
        } else {
            let max_scroll = total_lines.saturating_sub(body_height);
            if max_scroll == 0 {
                100
            } else {
                (scroll.min(max_scroll) * 100) / max_scroll
            }
        };
        let footer_text = format!(" {} lines  {}%", total_lines, scroll_pct);
        let footer_line = Line::from(vec![Span::styled(
            footer_text,
            Style::default().fg(theme.fg_dim),
        )]);
        footer_line.render(
            Rect {
                x: area.x,
                y: footer_y,
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

/// Render the deleted-file state: "filename (deleted)" centered in accent_red.
fn render_deleted_state(area: Rect, buf: &mut Buffer, theme: &Theme, filename: &str) {
    if area.height == 0 {
        return;
    }
    let text = format!("{} (deleted)", filename);
    let y = area.y + area.height / 2;
    let x = area.x + area.width.saturating_sub(text.len() as u16) / 2;
    buf.set_string(x, y, &text, Style::default().fg(theme.accent_red));
}

/// Render the empty-file state: "(empty file)" centered in fg_dim.
fn render_empty_file(area: Rect, buf: &mut Buffer, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let text = "(empty file)";
    let y = area.y + area.height / 2;
    let x = area.x + area.width.saturating_sub(text.len() as u16) / 2;
    buf.set_string(x, y, text, Style::default().fg(theme.fg_dim));
}

/// Produce a muted tint color for changed-line backgrounds.
///
/// Blends the given color toward black at ~40/255 intensity, yielding a
/// subtle background tint.
fn change_tint(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16) * 40 / 255) as u8,
            ((g as u16) * 40 / 255) as u8,
            ((b as u16) * 40 / 255) as u8,
        ),
        _ => Color::Rgb(6, 24, 7), // fallback muted green
    }
}
