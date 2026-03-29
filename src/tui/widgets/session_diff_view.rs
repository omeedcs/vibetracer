use crate::theme::Theme;
use crate::tui::session_diff::SessionDiff;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

const MAX_WIDTH: u16 = 72;
const MAX_HEIGHT: u16 = 30;

/// Full-screen overlay showing the results of a session diff.
pub struct SessionDiffView<'a> {
    pub diff: &'a SessionDiff,
    pub theme: &'a Theme,
    pub selected: usize,
}

impl<'a> SessionDiffView<'a> {
    pub fn new(diff: &'a SessionDiff, theme: &'a Theme, selected: usize) -> Self {
        Self {
            diff,
            theme,
            selected,
        }
    }
}

impl Widget for SessionDiffView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = MAX_WIDTH.min(area.width.saturating_sub(4));
        // Header (2 lines) + separator + file rows + separator + agent line + separator + keybindings
        let file_rows = self.diff.file_changes.len().min(20);
        let agent_summary = self.diff.agent_summary();
        let has_agents = !agent_summary.is_empty();
        let content_rows = 2 // header + column header
            + 1              // separator
            + file_rows
            + 1              // separator
            + if has_agents { 1 } else { 0 } // agent line
            + if has_agents { 1 } else { 0 } // separator after agents
            + 1; // keybindings
        let height = ((content_rows as u16) + 2).min(MAX_HEIGHT).min(area.height);

        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;

        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .title(" session diff ")
            .borders(Borders::ALL)
            .style(Style::default().bg(self.theme.bg).fg(self.theme.separator));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut row_y = inner.y;
        let max_y = inner.y + inner.height;
        let iw = inner.width as usize;

        // ── header line: SESSION DIFF  #from -> #to  (N edits, +A -R) ────────
        if row_y < max_y {
            let header = format!(
                " #{} -> #{}  ({} edits, +{} -{})",
                self.diff.from_edit,
                self.diff.to_edit,
                self.diff.edit_count,
                self.diff.total_added,
                self.diff.total_removed,
            );
            let line = Line::from(vec![
                Span::styled(
                    " SESSION DIFF ",
                    Style::default()
                        .fg(self.theme.bg)
                        .bg(self.theme.accent_blue),
                ),
                Span::styled(header, Style::default().fg(self.theme.fg)),
            ]);
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;
        }

        // ── separator ────────────────────────────────────────────────────────
        if row_y < max_y {
            render_separator(inner.x, row_y, inner.width, self.theme.separator, buf);
            row_y += 1;
        }

        // ── column headers ───────────────────────────────────────────────────
        if row_y < max_y {
            let line = build_file_row("FILE", "EDITS", "ADDED", "REMOVED", iw, self.theme.fg_muted);
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;
        }

        // ── file rows ────────────────────────────────────────────────────────
        for (i, fc) in self.diff.file_changes.iter().enumerate() {
            if row_y >= max_y {
                break;
            }

            let is_selected = i == self.selected;
            let bg = if is_selected {
                self.theme.separator
            } else {
                self.theme.bg
            };

            let added_str = format!("+{}", fc.lines_added);
            let removed_str = format!("-{}", fc.lines_removed);
            let edits_str = format!("{}", fc.edits);

            let name_col = iw.saturating_sub(24);
            let display_name = truncate_path(&fc.file, name_col);
            let name_pad = name_col.saturating_sub(display_name.len());

            let line = Line::from(vec![
                Span::styled(
                    if is_selected { " > " } else { "   " },
                    Style::default().fg(self.theme.accent_warm).bg(bg),
                ),
                Span::styled(
                    display_name,
                    Style::default().fg(self.theme.fg).bg(bg),
                ),
                Span::styled(
                    " ".repeat(name_pad),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>5}", edits_str),
                    Style::default().fg(self.theme.fg_muted).bg(bg),
                ),
                Span::styled(
                    format!("{:>8}", added_str),
                    Style::default().fg(self.theme.accent_green).bg(bg),
                ),
                Span::styled(
                    format!("{:>8}", removed_str),
                    Style::default().fg(self.theme.accent_red).bg(bg),
                ),
            ]);
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;
        }

        // ── separator ────────────────────────────────────────────────────────
        if row_y < max_y {
            render_separator(inner.x, row_y, inner.width, self.theme.separator, buf);
            row_y += 1;
        }

        // ── agent summary ────────────────────────────────────────────────────
        if has_agents && row_y < max_y {
            let agent_parts: Vec<String> = agent_summary
                .iter()
                .map(|(name, count)| format!("{} ({})", name, count))
                .collect();
            let agent_text = format!(" Agents: {}", agent_parts.join(", "));
            let line = Line::from(Span::styled(
                agent_text,
                Style::default().fg(self.theme.fg_muted),
            ));
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
            row_y += 1;

            if row_y < max_y {
                render_separator(inner.x, row_y, inner.width, self.theme.separator, buf);
                row_y += 1;
            }
        }

        // ── keybindings ──────────────────────────────────────────────────────
        if row_y < max_y {
            let line = Line::from(vec![
                Span::styled(" Esc", Style::default().fg(self.theme.fg)),
                Span::styled(":close", Style::default().fg(self.theme.fg_muted)),
                Span::styled("  Up/Down", Style::default().fg(self.theme.fg)),
                Span::styled(":select", Style::default().fg(self.theme.fg_muted)),
            ]);
            line.render(
                Rect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}

/// Render a horizontal separator line of box-drawing characters.
fn render_separator(x: u16, y: u16, width: u16, color: ratatui::style::Color, buf: &mut Buffer) {
    let sep: String = "\u{2500}".repeat(width as usize);
    let line = Line::from(Span::styled(sep, Style::default().fg(color)));
    line.render(
        Rect {
            x,
            y,
            width,
            height: 1,
        },
        buf,
    );
}

/// Build a formatted file-row line with aligned columns.
fn build_file_row(
    name: &str,
    edits: &str,
    added: &str,
    removed: &str,
    total_width: usize,
    color: ratatui::style::Color,
) -> Line<'static> {
    let name_col = total_width.saturating_sub(24);
    let display = if name.len() > name_col {
        format!("{}...", &name[..name_col.saturating_sub(3)])
    } else {
        name.to_string()
    };
    let pad = name_col.saturating_sub(display.len());

    Line::from(vec![
        Span::styled(
            format!("   {}", display),
            Style::default().fg(color),
        ),
        Span::styled(" ".repeat(pad), Style::default()),
        Span::styled(format!("{:>5}", edits), Style::default().fg(color)),
        Span::styled(format!("{:>8}", added), Style::default().fg(color)),
        Span::styled(format!("{:>8}", removed), Style::default().fg(color)),
    ])
}

/// Truncate a file path to fit within `max_len`, keeping the rightmost portion.
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else if max_len > 3 {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    } else {
        path[path.len() - max_len..].to_string()
    }
}
