use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::tui::{App, PlaybackState};

const COLOR_DEFAULT: Color = Color::Rgb(138, 143, 152);
const COLOR_SEPARATOR: Color = Color::Rgb(42, 46, 55);
const COLOR_VALUE: Color = Color::Rgb(160, 168, 183);
const COLOR_CONNECTED: Color = Color::Rgb(90, 158, 111);
const COLOR_WATCHING: Color = Color::Rgb(138, 143, 152);
const COLOR_LIVE: Color = Color::Rgb(90, 158, 111);
const COLOR_PAUSED: Color = Color::Rgb(138, 143, 152);
const COLOR_SPEED: Color = Color::Rgb(188, 140, 255);

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
        let sep = Span::styled(" | ", Style::default().fg(COLOR_SEPARATOR));

        // ── left side ────────────────────────────────────────────────────────
        let elapsed = self.elapsed_str();
        let edit_count = self.app.edits.len();
        let ckpt_count = self.app.checkpoint_ids.len();

        let left_spans = vec![
            Span::styled("vibetracer", Style::default().fg(COLOR_DEFAULT)),
            sep.clone(),
            Span::styled(elapsed, Style::default().fg(COLOR_VALUE)),
            sep.clone(),
            Span::styled(
                format!("{edit_count} edits"),
                Style::default().fg(COLOR_VALUE),
            ),
            sep.clone(),
            Span::styled(
                format!("{ckpt_count} ckpts"),
                Style::default().fg(COLOR_VALUE),
            ),
        ];

        // ── right side ───────────────────────────────────────────────────────
        let (conn_text, conn_color) = if self.app.connected {
            ("connected", COLOR_CONNECTED)
        } else {
            ("watching", COLOR_WATCHING)
        };

        let (pb_text, pb_color) = match &self.app.playback {
            PlaybackState::Live => ("live", COLOR_LIVE),
            PlaybackState::Paused => ("paused", COLOR_PAUSED),
            PlaybackState::Playing { speed } => {
                // We'll handle the speed separately below.
                let _ = speed;
                ("", COLOR_SPEED)
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
                    Style::default().fg(COLOR_SPEED),
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
