use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

const COLOR_HEADER: Color = Color::Rgb(90, 101, 119);
const COLOR_TRACK_NAME: Color = Color::Rgb(138, 143, 152);
const COLOR_TRACK_STALE: Color = Color::Rgb(158, 90, 90);
const COLOR_BAR_EDIT: Color = Color::Rgb(90, 101, 119);
const COLOR_BAR_EMPTY: Color = Color::Rgb(26, 29, 34);
const COLOR_PLAYHEAD: Color = Color::Rgb(138, 117, 96);

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
}

impl<'a> Widget for TimelineWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let mut row = area.y;
        let max_y = area.y + area.height;

        // ── header ───────────────────────────────────────────────────────────
        if row < max_y {
            let header = Line::from(vec![Span::styled(
                "tracks",
                Style::default().fg(COLOR_HEADER),
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

        // ── render each track row ─────────────────────────────────────────────
        for track in &tracks {
            if row >= max_y {
                break;
            }

            let (name_text, name_color) = if track.stale {
                (
                    format!(
                        "{} stale",
                        &Self::display_name(&track.filename)[..TRACK_NAME_WIDTH.min(8)]
                    ),
                    COLOR_TRACK_STALE,
                )
            } else {
                (Self::display_name(&track.filename), COLOR_TRACK_NAME)
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
                        ("\u{2588}", COLOR_BAR_EDIT) // full block
                    } else {
                        ("\u{2591}", COLOR_BAR_EMPTY) // light shade
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

            let mut ph_spans: Vec<Span> = vec![
                Span::raw(format!("{:<width$}", "", width = TRACK_NAME_WIDTH)),
                Span::raw(SEPARATOR),
            ];

            for cell in 0..bar_width {
                let ch = if cell == playhead_col { "|" } else { "-" };
                ph_spans.push(Span::styled(ch, Style::default().fg(COLOR_PLAYHEAD)));
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
