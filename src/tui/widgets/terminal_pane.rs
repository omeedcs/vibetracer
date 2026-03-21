use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub struct TerminalPane<'a> {
    lines: &'a [String],
    focused: bool,
}

impl<'a> TerminalPane<'a> {
    pub fn new(lines: &'a [String], focused: bool) -> Self {
        Self { lines, focused }
    }
}

impl Widget for TerminalPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.focused {
            Color::Rgb(138, 117, 96)
        } else {
            Color::Rgb(42, 46, 55)
        };

        // Title bar
        let title = if self.focused {
            "terminal (active)"
        } else {
            "terminal"
        };
        buf.set_string(area.x, area.y, title, Style::default().fg(border_color));

        // Render output lines
        let content_start = area.y + 1;
        let available_height = area.height.saturating_sub(1) as usize;
        let start = self.lines.len().saturating_sub(available_height);

        for (i, line) in self.lines[start..].iter().enumerate() {
            let y = content_start + i as u16;
            if y >= area.y + area.height {
                break;
            }
            // Strip ANSI escape sequences for clean rendering
            let clean = strip_ansi(line);
            let truncated = if clean.len() > area.width as usize {
                clean[..area.width as usize].to_string()
            } else {
                clean
            };
            buf.set_string(
                area.x,
                y,
                truncated,
                Style::default().fg(Color::Rgb(160, 168, 183)),
            );
        }
    }
}

/// Basic ANSI escape sequence stripping.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if let Some(next) = chars.next() {
                if next == '[' {
                    // CSI sequence — skip until letter
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() || c == 'm' {
                            break;
                        }
                    }
                }
            }
        } else if c == '\r' {
            // Skip carriage returns
        } else {
            result.push(c);
        }
    }
    result
}
