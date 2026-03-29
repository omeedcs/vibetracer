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
use crate::tui::App;
use crate::tui::blame;
use crate::tui::syntax::{HighlightedLine, Highlighter};

const GUTTER_WIDTH: u16 = 6;
const BLAME_WIDTH: u16 = 18;
const ANNOTATION_WIDTH: u16 = 22;

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
        let highlighted = self
            .highlighter
            .highlight(self.filename, self.content, theme);
        let total_lines = highlighted.len();

        // Reserve 1 row for header, 0 for footer.
        let header_rows: u16 = 1;
        let footer_rows: u16 = 0;
        let body_height = area.height.saturating_sub(header_rows + footer_rows) as usize;

        let scroll = self.app.preview_scroll;

        let scroll_pct = if total_lines <= body_height {
            100
        } else {
            let max_scroll = total_lines.saturating_sub(body_height);
            if max_scroll == 0 { 100 } else { (scroll.min(max_scroll) * 100) / max_scroll }
        };

        // -- Header --
        let header_line = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(self.filename, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502} ", Style::default().fg(theme.separator)),
            Span::styled(format!("{} lines", total_lines), Style::default().fg(theme.fg_muted)),
            Span::styled(" \u{2502} ", Style::default().fg(theme.separator)),
            Span::styled(format!("{}%", scroll_pct), Style::default().fg(theme.fg_muted)),
        ]);
        header_line.render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        // -- Compute blame data if needed --
        let blame_data = if self.app.blame_visible {
            let filename = self.filename;
            Some(blame::compute_blame(&self.app.edits, filename, self.app.playhead))
        } else {
            None
        };

        // -- Body: syntax-highlighted lines with gutter --
        let right_col_width = if self.app.blame_visible {
            BLAME_WIDTH + 1 // +1 for separator
        } else if self.app.annotations_visible {
            ANNOTATION_WIDTH + 1
        } else {
            0
        };
        let content_width = area.width.saturating_sub(GUTTER_WIDTH + 1 + right_col_width);

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
            let gutter_bg = if is_changed {
                change_tint(theme.accent_green)
            } else {
                Color::Reset
            };
            let gutter_text = format!("{:>width$} ", line_num, width = (GUTTER_WIDTH - 1) as usize);
            let gutter_span = Span::styled(gutter_text, Style::default().fg(gutter_color).bg(gutter_bg));

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
                    width: area.width.saturating_sub(right_col_width),
                    height: 1,
                },
                buf,
            );

            // -- Blame/annotation right column --
            if right_col_width > 0 {
                let col_x = area.x + area.width.saturating_sub(right_col_width);
                let col_width = right_col_width.saturating_sub(1);

                buf.set_string(col_x, y, "\u{2502}", Style::default().fg(theme.separator));

                if self.app.blame_visible {
                    if let Some(ref blame_map) = blame_data {
                        if let Some(b) = blame_map.get(&line_num) {
                            let text = blame::format_blame(b, col_width as usize);
                            let agent_idx = b.agent_label.as_ref()
                                .map(|label| {
                                    let mut hash: usize = 0;
                                    for byte in label.bytes() {
                                        hash = hash.wrapping_mul(31).wrapping_add(byte as usize);
                                    }
                                    hash % theme.agent_colors.len()
                                })
                                .unwrap_or(0);
                            buf.set_string(col_x + 1, y, &text, Style::default().fg(theme.agent_colors[agent_idx]));
                        } else {
                            buf.set_string(col_x + 1, y, "original", Style::default().fg(theme.fg_dim));
                        }
                    }
                } else if self.app.annotations_visible && is_changed {
                    if let Some(edit) = self.app.current_edit() {
                        if let Some(ref intent) = edit.operation_intent {
                            let truncated: String = intent.chars().take(col_width as usize).collect();
                            buf.set_string(col_x + 1, y, &truncated, Style::default().fg(theme.accent_warm));
                        }
                    }
                }
            }
        }

        // -- Scrollbar (right edge) --
        if total_lines > body_height {
            let scrollbar_x = area.x + area.width - 1;
            let scrollbar_height = body_height;
            let thumb_size = ((body_height as f64 / total_lines as f64) * scrollbar_height as f64)
                .max(1.0) as usize;
            let max_scroll = total_lines.saturating_sub(body_height);
            let thumb_pos = if max_scroll == 0 {
                0
            } else {
                (scroll.min(max_scroll) * scrollbar_height.saturating_sub(thumb_size)) / max_scroll
            };

            for i in 0..scrollbar_height {
                let y = area.y + header_rows + i as u16;
                if y >= area.y + area.height {
                    break;
                }
                let (ch, color) = if i >= thumb_pos && i < thumb_pos + thumb_size {
                    ("\u{2503}", theme.accent_warm)
                } else {
                    ("\u{2502}", theme.bar_empty)
                };
                buf.set_string(scrollbar_x, y, ch, Style::default().fg(color));
            }
        }
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
            ((r as u16) * 55 / 255) as u8,
            ((g as u16) * 55 / 255) as u8,
            ((b as u16) * 55 / 255) as u8,
        ),
        _ => Color::Rgb(8, 33, 10),
    }
}
