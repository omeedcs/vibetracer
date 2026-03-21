use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::{App, PlaybackState};

/// A single-line status bar widget.
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
            format!("{h}h{m:02}m{s:02}s")
        } else {
            format!("{m}m{s:02}s")
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.app.theme;

        let color_default: Color = t.fg_muted;
        let color_separator: Color = t.separator;
        let color_value: Color = t.fg;
        let color_connected: Color = t.accent_green;
        let color_watching: Color = t.fg_muted;
        let color_live: Color = t.accent_green;
        let color_paused: Color = t.fg_muted;
        let color_speed: Color = t.accent_purple;

        let sep = Span::styled(" | ", Style::default().fg(color_separator));

        // ── left side ────────────────────────────────────────────────────────
        let elapsed = self.elapsed_str();
        let edit_count = self.app.edits.len();
        let ckpt_count = self.app.checkpoint_ids.len();

        let left_spans = vec![
            Span::styled("vibetracer", Style::default().fg(color_default)),
            sep.clone(),
            Span::styled(elapsed, Style::default().fg(color_value)),
            sep.clone(),
            Span::styled(
                format!("{edit_count} edits"),
                Style::default().fg(color_value),
            ),
            sep.clone(),
            Span::styled(
                format!("{ckpt_count} ckpts"),
                Style::default().fg(color_value),
            ),
        ];

        // ── right side ───────────────────────────────────────────────────────
        let (conn_text, conn_color) = if self.app.connected {
            ("connected", color_connected)
        } else {
            ("watching", color_watching)
        };

        let (pb_text, pb_color) = match &self.app.playback {
            PlaybackState::Live => ("live", color_live),
            PlaybackState::Paused => ("paused", color_paused),
            PlaybackState::Playing { speed } => {
                let _ = speed;
                ("", color_speed)
            }
        };

        let mut right_spans: Vec<Span> = vec![
            Span::styled(conn_text, Style::default().fg(conn_color)),
            sep.clone(),
        ];

        match &self.app.playback {
            PlaybackState::Playing { speed } => {
                right_spans.push(Span::styled(
                    format!("{speed}x"),
                    Style::default().fg(color_speed),
                ));
            }
            _ => {
                right_spans.push(Span::styled(pb_text, Style::default().fg(pb_color)));
            }
        }

        // ── compose and render ───────────────────────────────────────────────
        let left_line = Line::from(left_spans);
        let right_line = Line::from(right_spans);

        // Measure right side width so we can right-align it.
        let right_width: u16 = right_line
            .spans
            .iter()
            .map(|s| s.content.len() as u16)
            .sum();

        // Render left side.
        left_line.render(area, buf);

        // Render right side flush to the right edge.
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
