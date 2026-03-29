use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::{App, PlaybackState};
use crate::tui::app::Mode;

/// Duration (in seconds) to flash the theme name after a theme change.
const THEME_FLASH_SECS: u64 = 2;

/// A single-line dense status bar widget (htop-style).
pub struct StatusBar<'a> {
    pub app: &'a App,
}

impl<'a> StatusBar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    fn elapsed_str(&self) -> String {
        let now = chrono::Utc::now().timestamp();
        let secs = (now - self.app.session_start).max(0) as u64;
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        if h > 0 {
            format!("{h}h{m:02}m")
        } else {
            format!("{m}m{s:02}s")
        }
    }

    fn current_agent_label(&self) -> Option<&str> {
        self.app
            .current_edit()
            .and_then(|e| e.agent_label.as_deref())
    }

    fn theme_flash_active(&self) -> bool {
        self.app
            .theme_flash
            .map(|t| t.elapsed().as_secs() < THEME_FLASH_SECS)
            .unwrap_or(false)
    }

    fn unique_file_count(&self) -> usize {
        self.app.tracks.len()
    }

    fn unique_agent_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for edit in &self.app.edits {
            if let Some(ref agent_id) = edit.agent_id {
                seen.insert(agent_id.as_str());
            }
        }
        seen.len()
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.app.theme;

        let color_sep: Color = t.separator;
        let color_val: Color = t.fg;
        let color_dim: Color = t.fg_muted;
        let color_live: Color = t.accent_green;
        let color_paused: Color = t.fg_muted;
        let color_speed: Color = t.accent_purple;
        let color_accent: Color = t.accent_warm;

        let sep = Span::styled(" \u{2502} ", Style::default().fg(color_sep));

        // ── left side: mode + metrics ────────────────────────────────────────
        let mode_color = match self.app.mode {
            Mode::Normal => t.accent_green,
            Mode::Timeline => t.accent_blue,
            Mode::Inspect => t.accent_warm,
            Mode::Search => t.accent_purple,
        };

        let mut left_spans = vec![
            Span::styled(
                format!(" {} ", self.app.mode.label()),
                Style::default().fg(t.bg).bg(mode_color),
            ),
            sep.clone(),
        ];

        // Playback state
        let pb_flashing = self.app.playback_flash
            .map(|t| t.elapsed().as_millis() < 500)
            .unwrap_or(false);

        match &self.app.playback {
            PlaybackState::Live => {
                let c = if pb_flashing { color_accent } else { color_live };
                left_spans.push(Span::styled("live", Style::default().fg(c)));
            }
            PlaybackState::Paused => {
                let c = if pb_flashing { color_accent } else { color_paused };
                left_spans.push(Span::styled("paused", Style::default().fg(c)));
            }
            PlaybackState::Playing { speed } => {
                let c = if pb_flashing { color_accent } else { color_speed };
                left_spans.push(Span::styled(format!("{speed}x"), Style::default().fg(c)));
            }
        }

        left_spans.push(sep.clone());

        // Core counters
        let edit_count = self.app.edits.len();
        let file_count = self.unique_file_count();
        let agent_count = self.unique_agent_count();

        left_spans.push(Span::styled(
            format!("{edit_count} edits"),
            Style::default().fg(color_val),
        ));
        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            format!("{file_count} files"),
            Style::default().fg(color_val),
        ));

        if agent_count > 0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("{agent_count} agents"),
                Style::default().fg(color_val),
            ));
        }

        // Agent label for current edit
        if let Some(agent) = self.current_agent_label() {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                agent.to_string(),
                Style::default().fg(color_accent),
            ));
        }

        // Edit position
        if !self.app.edits.is_empty() {
            left_spans.push(Span::styled(
                format!(" #{}/{}", self.app.playhead + 1, self.app.edits.len()),
                Style::default().fg(color_dim),
            ));
        }

        // Preview mode indicator
        if self.app.preview_mode == crate::tui::app::PreviewMode::Diff {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled("diff", Style::default().fg(color_accent)));
        }

        // Command view indicator
        if self.app.command_view {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled("cmds", Style::default().fg(color_accent)));
        }

        // Search filter indicator (when filter is locked in Normal mode)
        if self.app.mode == Mode::Normal && !self.app.search_input.is_empty() {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("/{}", self.app.search_input),
                Style::default().fg(t.accent_purple),
            ));
        }

        // Search mode: show live input
        if self.app.mode == Mode::Search {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled("/", Style::default().fg(t.accent_purple)));
            left_spans.push(Span::styled(
                self.app.search_input.as_str(),
                Style::default().fg(color_val),
            ));
            left_spans.push(Span::styled("\u{2588}", Style::default().fg(t.accent_purple)));
        }

        // ── right side ───────────────────────────────────────────────────────
        let mut right_spans: Vec<Span> = Vec::new();

        // Toast notification
        if self.app.toast_active() {
            if let Some(ref msg) = self.app.toast_message {
                let toast_color = match self.app.toast_style {
                    crate::tui::app::ToastStyle::Info => t.fg,
                    crate::tui::app::ToastStyle::Success => t.accent_green,
                    crate::tui::app::ToastStyle::Warning => t.accent_red,
                };
                right_spans.push(Span::styled(msg.clone(), Style::default().fg(toast_color)));
                right_spans.push(sep.clone());
            }
        }

        // Theme flash
        if self.theme_flash_active() {
            right_spans.push(Span::styled(
                self.app.theme_name.as_str(),
                Style::default().fg(color_accent),
            ));
            right_spans.push(sep.clone());
        }

        // Elapsed time (right-aligned)
        let elapsed = self.elapsed_str();
        right_spans.push(Span::styled(elapsed, Style::default().fg(color_dim)));

        // ── compose and render ───────────────────────────────────────────────
        let left_line = Line::from(left_spans);
        let right_line = Line::from(right_spans);

        let right_width: u16 = right_line
            .spans
            .iter()
            .map(|s| s.content.len() as u16)
            .sum();

        let left_width: u16 = left_line.spans.iter().map(|s| s.content.len() as u16).sum();
        let available = area.width.saturating_sub(right_width + 2);

        let left_line = if left_width > available {
            let mut total: u16 = 0;
            let mut truncated_spans = Vec::new();
            for span in left_line.spans {
                let span_len = span.content.len() as u16;
                if total + span_len > available {
                    let remaining = available.saturating_sub(total) as usize;
                    if remaining > 0 {
                        let truncated: String = span.content.chars().take(remaining).collect();
                        truncated_spans.push(Span::styled(truncated, span.style));
                    }
                    break;
                }
                total += span_len;
                truncated_spans.push(span);
            }
            Line::from(truncated_spans)
        } else {
            left_line
        };

        left_line.render(area, buf);

        if area.width >= right_width {
            let right_area = Rect {
                x: area.x + area.width - right_width,
                y: area.y,
                width: right_width,
                height: 1,
            };
            right_line.render(right_area, buf);
        }
    }
}
