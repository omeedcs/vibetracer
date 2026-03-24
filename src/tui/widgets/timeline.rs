use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

const TRACK_NAME_WIDTH: usize = 14;
const SEPARATOR: &str = " ";

/// Horizontal per-file track timeline widget.
pub struct TimelineWidget<'a> {
    pub app: &'a App,
}

impl<'a> TimelineWidget<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    /// Return the list of visible tracks after applying solo/mute filters.
    fn visible_tracks(&self) -> Vec<&crate::tui::TrackInfo> {
        self.app
            .tracks
            .iter()
            .filter(|t| {
                // If a track is soloed, only show that track.
                if let Some(solo) = &self.app.solo_track {
                    return &t.filename == solo;
                }
                // Skip muted tracks.
                !self.app.muted_tracks.contains(&t.filename)
            })
            .collect()
    }

    /// Shorten a filename for display: if longer than `TRACK_NAME_WIDTH` chars,
    /// use the basename; truncate further if still too long.
    fn display_name(filename: &str) -> String {
        if filename.len() <= TRACK_NAME_WIDTH {
            return format!("{:<width$}", filename, width = TRACK_NAME_WIDTH);
        }
        // Try basename.
        let base = std::path::Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);

        if base.len() <= TRACK_NAME_WIDTH {
            format!("{:<width$}", base, width = TRACK_NAME_WIDTH)
        } else {
            format!("{:.width$}", base, width = TRACK_NAME_WIDTH)
        }
    }

    /// Look up the agent color for an edit at a given global index.
    /// Returns None if the edit has no agent_id or the index is out of bounds.
    fn agent_color_for_edit(&self, edit_idx: usize) -> Option<Color> {
        let edit = self.app.edits.get(edit_idx)?;
        let agent_id = edit.agent_id.as_ref()?;
        let agent_colors = &self.app.theme.agent_colors;
        if agent_colors.is_empty() {
            return None;
        }
        // Simple hash: sum of bytes mod color count.
        let hash: usize = agent_id.bytes().map(|b| b as usize).sum();
        Some(agent_colors[hash % agent_colors.len()])
    }
}

impl Widget for TimelineWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let t = &self.app.theme;
        let color_header: Color = t.fg_muted;
        let color_track_name: Color = t.fg;
        let color_track_stale: Color = t.accent_red;
        let color_bar_edit: Color = t.bar_filled;
        let color_bar_empty: Color = t.bar_empty;
        let color_playhead: Color = t.accent_warm;
        let color_detached_playhead: Color = t.accent_purple;

        let mut row = area.y;
        let max_y = area.y + area.height;

        // ── header ───────────────────────────────────────────────────────────
        if row < max_y {
            let header = Line::from(vec![Span::styled(
                "tracks",
                Style::default().fg(color_header),
            )]);
            header.render(
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            row += 1;
        }

        let tracks = self.visible_tracks();
        let total_edits = self.app.edits.len();

        // ── bar width (area minus track name and separator) ──────────────────
        let name_and_sep = TRACK_NAME_WIDTH + SEPARATOR.len();
        let bar_width = (area.width as usize).saturating_sub(name_and_sep);

        // Empty state -- show a subtle waiting indicator instead of nothing.
        if tracks.is_empty() && row < max_y {
            row += 1; // skip a line
            if row < max_y {
                let waiting = "waiting for file changes...";
                let x = area.x + (area.width.saturating_sub(waiting.len() as u16)) / 2;
                buf.set_string(x, row, waiting, Style::default().fg(t.separator));
            }
            return;
        }

        // ── render each track row ─────────────────────────────────────────────
        for track in &tracks {
            if row >= max_y {
                break;
            }

            let is_detached = self.app.detached_files.contains(&track.filename);

            let (name_text, name_color) = if track.stale {
                let display = Self::display_name(&track.filename);
                let truncated: String = display.chars().take(TRACK_NAME_WIDTH.min(8)).collect();
                (format!("{} stale", truncated), color_track_stale)
            } else if is_detached {
                // Mark detached tracks with a subtle indicator.
                let display = Self::display_name(&track.filename);
                (display, color_detached_playhead)
            } else {
                (Self::display_name(&track.filename), color_track_name)
            };

            let mut spans: Vec<Span> = vec![
                Span::styled(
                    format!("{:<width$}", name_text, width = TRACK_NAME_WIDTH),
                    Style::default().fg(name_color),
                ),
                Span::raw(SEPARATOR),
            ];

            // Build the bar characters.
            if bar_width > 0 && total_edits > 0 {
                // Distribute edit indices across bar_width cells.
                for cell in 0..bar_width {
                    // Map each cell to an edit index range.
                    let edit_idx = if total_edits <= bar_width {
                        cell
                    } else {
                        cell * total_edits / bar_width
                    };
                    let has_edit = track.edit_indices.contains(&edit_idx);
                    let (ch, color) = if has_edit {
                        // Use agent color if the edit has an agent_id.
                        let agent_col = self.agent_color_for_edit(edit_idx);
                        ("\u{2588}", agent_col.unwrap_or(color_bar_edit)) // full block
                    } else {
                        ("\u{2591}", color_bar_empty) // light shade
                    };
                    spans.push(Span::styled(ch, Style::default().fg(color)));
                }
            }

            Line::from(spans).render(
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            row += 1;
        }

        // ── playhead line ─────────────────────────────────────────────────────
        if row < max_y {
            let playhead_col = if total_edits > 1 && bar_width > 0 {
                let ratio = self.app.playhead as f64 / (total_edits - 1) as f64;
                (ratio * (bar_width - 1) as f64).round() as usize
            } else {
                0
            };

            // Use accent_purple for the playhead if any file is detached.
            let ph_color = if self.app.detached_files.is_empty() {
                color_playhead
            } else {
                color_detached_playhead
            };

            let mut ph_spans: Vec<Span> = vec![
                Span::raw(format!("{:<width$}", "", width = TRACK_NAME_WIDTH)),
                Span::raw(SEPARATOR),
            ];

            for cell in 0..bar_width {
                let ch = if cell == playhead_col { "|" } else { "-" };
                ph_spans.push(Span::styled(ch, Style::default().fg(ph_color)));
            }

            Line::from(ph_spans).render(
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}
