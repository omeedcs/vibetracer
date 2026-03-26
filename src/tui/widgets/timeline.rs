use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::App;

const TRACK_NAME_WIDTH: usize = 20;
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
                // File-level solo filter.
                if let Some(solo) = &self.app.solo_track {
                    return &t.filename == solo;
                }
                // Skip muted tracks.
                if self.app.muted_tracks.contains(&t.filename) {
                    return false;
                }
                // Agent-level solo filter: only show tracks with edits from this agent.
                if let Some(ref solo_agent) = self.app.solo_agent {
                    return t.edit_indices.iter().any(|&idx| {
                        self.app.edits.get(idx)
                            .and_then(|e| e.agent_id.as_ref())
                            .map(|a| a == solo_agent)
                            .unwrap_or(false)
                    });
                }
                true
            })
            .collect()
    }

    /// Shorten a filename for display: tries parent/file.rs, then basename, then truncated.
    fn display_name(filename: &str) -> String {
        if filename.len() <= TRACK_NAME_WIDTH {
            return format!("{:>width$}", filename, width = TRACK_NAME_WIDTH);
        }
        let path = std::path::Path::new(filename);
        let base = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);
        let parent = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());

        let with_parent = if let Some(p) = parent {
            format!("{}/{}", p, base)
        } else {
            base.to_string()
        };

        if with_parent.len() <= TRACK_NAME_WIDTH {
            format!("{:>width$}", with_parent, width = TRACK_NAME_WIDTH)
        } else if base.len() <= TRACK_NAME_WIDTH {
            format!("{:>width$}", base, width = TRACK_NAME_WIDTH)
        } else {
            format!("{:>width$.width$}", base, width = TRACK_NAME_WIDTH)
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

    /// Map an edit index to a horizontal column based on zoom and scroll.
    fn edit_to_col(&self, edit_idx: usize, bar_width: usize, total_edits: usize) -> usize {
        if total_edits == 0 || bar_width == 0 {
            return 0;
        }
        if self.app.timeline_zoom <= 1.0 {
            // Fit-all mode
            if total_edits <= bar_width {
                edit_idx
            } else {
                edit_idx * bar_width / total_edits
            }
        } else {
            // Zoomed mode
            let pos = (edit_idx as f64 * self.app.timeline_zoom) as usize;
            pos.saturating_sub(self.app.timeline_scroll)
        }
    }

    /// Format a millisecond timestamp as elapsed time since session start.
    fn format_time(&self, ts_ms: i64) -> String {
        let session_start_ms = self.app.session_start * 1000;
        let offset_secs = ((ts_ms - session_start_ms).max(0) / 1000) as u64;
        let h = offset_secs / 3600;
        let m = (offset_secs % 3600) / 60;
        let s = offset_secs % 60;
        if h > 0 {
            format!("{h}:{m:02}:{s:02}")
        } else {
            format!("{m}:{s:02}")
        }
    }

    /// Detect conflict zones: edit indices where 2+ agents edited the same file
    /// within a 5-second (5000ms) window.
    fn conflict_edit_indices(&self) -> std::collections::HashSet<usize> {
        let mut conflicts = std::collections::HashSet::new();
        let edits = &self.app.edits;

        for i in 0..edits.len() {
            for j in (i + 1)..edits.len() {
                // Only check within 5s window
                if (edits[j].ts - edits[i].ts).unsigned_abs() > 5000 {
                    break;
                }
                // Same file, different agents
                if edits[i].file == edits[j].file {
                    let agent_i = edits[i].agent_id.as_deref();
                    let agent_j = edits[j].agent_id.as_deref();
                    if agent_i.is_some() && agent_j.is_some() && agent_i != agent_j {
                        conflicts.insert(i);
                        conflicts.insert(j);
                    }
                }
            }
        }

        conflicts
    }
}

impl Widget for TimelineWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let t = &self.app.theme;
        let color_track_name: Color = t.fg;
        let color_bar_edit: Color = t.bar_filled;
        let color_bar_empty: Color = t.bar_empty;
        let color_playhead: Color = t.accent_warm;
        let color_detached_playhead: Color = t.accent_purple;

        let mut row = area.y;
        let max_y = area.y + area.height;

        let tracks = self.visible_tracks();
        let total_edits = self.app.edits.len();

        // ── bar width (area minus track name and separator) ──────────────────
        let name_and_sep = TRACK_NAME_WIDTH + SEPARATOR.len();
        let bar_width = (area.width as usize).saturating_sub(name_and_sep);
        let conflict_indices = self.conflict_edit_indices();

        // Empty state -- show a subtle waiting indicator instead of nothing.
        if tracks.is_empty() {
            if row < max_y {
                row += 1; // skip a line
            }
            if row < max_y {
                let waiting = "waiting for file changes...";
                let x = area.x + (area.width.saturating_sub(waiting.len() as u16)) / 2;
                buf.set_string(x, row, waiting, Style::default().fg(t.separator));
            }
            return;
        }

        // ── 1. Time scale bar ──────────────────────────────────────────────────
        if row < max_y {
            let bar_x = area.x + name_and_sep as u16;
            // Place timestamps at bar_width / 5 intervals (at least 1).
            let interval = if bar_width >= 5 { bar_width / 5 } else { 1 };
            let mut col = 0;
            while col < bar_width {
                // Reverse-map this column back to an edit index.
                let edit_idx = if total_edits == 0 {
                    0
                } else if self.app.timeline_zoom <= 1.0 {
                    if total_edits <= bar_width {
                        col.min(total_edits.saturating_sub(1))
                    } else {
                        (col * total_edits / bar_width).min(total_edits.saturating_sub(1))
                    }
                } else {
                    let shifted = col + self.app.timeline_scroll;
                    let idx = (shifted as f64 / self.app.timeline_zoom) as usize;
                    idx.min(total_edits.saturating_sub(1))
                };

                let label = if total_edits > 0 {
                    let ts_ms = self.app.edits[edit_idx].ts;
                    self.format_time(ts_ms)
                } else {
                    "0:00".to_string()
                };

                // Write the label prefixed with a tick mark (┬), starting at this column,
                // truncated to bar boundary.
                let write_x = bar_x + col as u16;
                let prefixed = format!("\u{252C}{}", label);
                for (i, ch) in prefixed.chars().enumerate() {
                    let cx = write_x + i as u16;
                    if cx < area.x + area.width {
                        buf.set_string(cx, row, ch.to_string(), Style::default().fg(t.fg_dim));
                    }
                }

                col += interval.max(1);
            }
            row += 1;
        }

        // ── 2. Track rows ──────────────────────────────────────────────────────
        // Record which rows are track rows so we can overlay the playhead later.
        let track_row_start = row;

        for track in &tracks {
            if row >= max_y {
                break;
            }

            let is_detached = self.app.detached_files.contains(&track.filename);

            let (name_text, name_color) = if track.stale {
                let display = Self::display_name(&track.filename);
                let trimmed = display.trim_start();
                let padded = format!("{:>width$}", format!("{}*", trimmed), width = TRACK_NAME_WIDTH);
                (padded, t.fg_dim)
            } else if is_detached {
                // Mark detached tracks with a subtle indicator.
                let display = Self::display_name(&track.filename);
                (display, color_detached_playhead)
            } else {
                (Self::display_name(&track.filename), color_track_name)
            };

            // Determine active/flash state for background highlight.
            let is_active = self.app.current_edit()
                .map(|e| e.file == track.filename)
                .unwrap_or(false);

            let is_flashing = self.app.track_flash
                .as_ref()
                .map(|(f, t)| f == &track.filename && t.elapsed().as_millis() < 300)
                .unwrap_or(false);

            let row_bg = if is_active || is_flashing {
                Some(change_tint(t.accent_warm))
            } else {
                None
            };

            // Build name span with optional background.
            let name_style = if let Some(bg) = row_bg {
                Style::default().fg(name_color).bg(bg)
            } else {
                Style::default().fg(name_color)
            };

            // Build separator span with optional background.
            let sep_style = if let Some(bg) = row_bg {
                Style::default().bg(bg)
            } else {
                Style::default()
            };

            let mut spans: Vec<Span> = vec![
                Span::styled(
                    format!("{:<width$}", name_text, width = TRACK_NAME_WIDTH),
                    name_style,
                ),
                Span::styled(SEPARATOR, sep_style),
            ];

            // Build the bar as a Vec<(char, Color)> initialized to empty.
            let mut bar: Vec<(char, Color)> = vec![('\u{2500}', color_bar_empty); bar_width];

            if bar_width > 0 && total_edits > 0 {
                for &edit_idx in &track.edit_indices {
                    let col = self.edit_to_col(edit_idx, bar_width, total_edits);
                    if col < bar_width {
                        let agent_col = self.agent_color_for_edit(edit_idx);
                        if conflict_indices.contains(&edit_idx) {
                            bar[col] = ('!', t.accent_red);
                        } else {
                            bar[col] = ('\u{258C}', agent_col.unwrap_or(color_bar_edit));
                        }
                    }
                }
            }

            for &(ch, color) in &bar {
                let bar_style = if let Some(bg) = row_bg {
                    Style::default().fg(color).bg(bg)
                } else {
                    Style::default().fg(color)
                };
                spans.push(Span::styled(ch.to_string(), bar_style));
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

        let track_row_end = row; // exclusive

        // ── Compute playhead column ────────────────────────────────────────────
        let playhead_col = if total_edits > 0 && bar_width > 0 {
            self.edit_to_col(self.app.playhead, bar_width, total_edits)
                .min(bar_width.saturating_sub(1))
        } else {
            0
        };

        // Use accent_purple for the playhead if any file is detached.
        let ph_color = if self.app.detached_files.is_empty() {
            color_playhead
        } else {
            color_detached_playhead
        };

        // ── 3. Full-height playhead overlay on track rows ──────────────────────
        {
            let ph_x = area.x + name_and_sep as u16 + playhead_col as u16;
            if ph_x < area.x + area.width {
                for track_row in track_row_start..track_row_end {
                    buf.set_string(ph_x, track_row, "\u{2503}", Style::default().fg(ph_color));
                }
            }
        }

        // ── 4. Playhead row ────────────────────────────────────────────────────
        if row < max_y {
            let mut ph_spans: Vec<Span> = vec![
                Span::raw(format!("{:<width$}", "", width = TRACK_NAME_WIDTH)),
                Span::raw(SEPARATOR),
            ];

            for cell in 0..bar_width {
                let ch = if cell == playhead_col {
                    "\u{2503}"
                } else if cell >= playhead_col.saturating_sub(1) && cell <= playhead_col + 1 {
                    "\u{2501}"
                } else {
                    "\u{2500}"
                };
                ph_spans.push(Span::styled(ch, Style::default().fg(ph_color)));
            }

            // Append formatted timestamp of the current edit.
            if total_edits > 0 {
                let ts_ms = self.app.edits[self.app.playhead.min(total_edits - 1)].ts;
                let ts_label = format!(" {}", self.format_time(ts_ms));
                ph_spans.push(Span::styled(ts_label, Style::default().fg(ph_color)));
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
            row += 1;
        }

        // ── 5. Agent legend (only when >1 unique agent) ────────────────────────
        if row < max_y {
            // Collect unique (agent_label, color) pairs.
            let mut seen = std::collections::HashSet::new();
            let mut agents: Vec<(String, Color)> = Vec::new();
            for edit in &self.app.edits {
                if let (Some(agent_id), Some(agent_label)) = (&edit.agent_id, &edit.agent_label) {
                    if seen.insert(agent_id.clone()) {
                        let agent_colors = &self.app.theme.agent_colors;
                        let hash: usize = agent_id.bytes().map(|b| b as usize).sum();
                        let color = if agent_colors.is_empty() {
                            color_bar_edit
                        } else {
                            agent_colors[hash % agent_colors.len()]
                        };
                        agents.push((agent_label.clone(), color));
                    }
                }
            }

            if agents.len() > 1 {
                let mut legend_spans: Vec<Span> =
                    vec![Span::styled("agents  ", Style::default().fg(t.fg_muted))];
                for (label, color) in &agents {
                    legend_spans.push(Span::styled(
                        "\u{2588}\u{2588}",
                        Style::default().fg(*color),
                    ));
                    legend_spans.push(Span::styled(
                        format!(" {}  ", label),
                        Style::default().fg(t.fg_dim),
                    ));
                }

                Line::from(legend_spans).render(
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
}

fn change_tint(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16) * 40 / 255) as u8,
            ((g as u16) * 40 / 255) as u8,
            ((b as u16) * 40 / 255) as u8,
        ),
        _ => Color::Rgb(10, 8, 6),
    }
}
