//! Startup intro animation for the vibetracer TUI.
//!
//! Plays a fast (~1.5s) cockpit power-on sequence before the main UI appears.

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::io;
use std::thread;
use std::time::Duration;

/// A single frame of the intro animation.
struct IntroFrame {
    lines: Vec<(Style, String)>,
}

impl IntroFrame {
    fn render_centered(&self, area: Rect, buf: &mut Buffer, bg: Color) {
        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg);
                    cell.set_char(' ');
                }
            }
        }

        let total_height = self.lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(total_height) / 2;

        for (i, (style, text)) in self.lines.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            let text_width = text.chars().count() as u16;
            let x = area.x + area.width.saturating_sub(text_width) / 2;
            buf.set_string(x, y, text, *style);
        }
    }
}

/// Play the intro animation on the given terminal.
pub fn play_intro(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bg: Color,
    fg: Color,
    accent: Color,
    dim: Color,
    green: Color,
) -> anyhow::Result<()> {
    let logo_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let tagline_style = Style::default().fg(fg);
    let dim_style = Style::default().fg(dim);
    let green_style = Style::default().fg(green);
    let box_style = Style::default().fg(dim);

    // Frame 1: Scanline
    terminal.draw(|frame| {
        let area = frame.area();
        let buf = frame.buffer_mut();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg);
                    cell.set_char(' ');
                }
            }
        }
        let mid_y = area.y + area.height / 2;
        let scanline = "\u{2500}".repeat(area.width as usize);
        buf.set_string(area.x, mid_y, &scanline, Style::default().fg(accent));
    })?;
    thread::sleep(Duration::from_millis(120));

    // Frame 2: Letters appear spaced out
    terminal.draw(|frame| {
        let area = frame.area();
        let f = IntroFrame {
            lines: vec![
                (logo_style, "v  i  b  e  t  r  a  c  e  r".to_string()),
            ],
        };
        f.render_centered(area, frame.buffer_mut(), bg);
    })?;
    thread::sleep(Duration::from_millis(200));

    // Frame 3: Logo with box
    terminal.draw(|frame| {
        let area = frame.area();
        let w = 44;
        let border = "\u{2500}".repeat(w - 2);
        let f = IntroFrame {
            lines: vec![
                (box_style, format!("\u{250c}{}\u{2510}", border)),
                (logo_style, "v i b e t r a c e r".to_string()),
                (box_style, format!("\u{2514}{}\u{2518}", border)),
            ],
        };
        f.render_centered(area, frame.buffer_mut(), bg);
    })?;
    thread::sleep(Duration::from_millis(200));

    // Frame 4: Tagline
    terminal.draw(|frame| {
        let area = frame.area();
        let w = 44;
        let border = "\u{2500}".repeat(w - 2);
        let f = IntroFrame {
            lines: vec![
                (box_style, format!("\u{250c}{}\u{2510}", border)),
                (logo_style, "v i b e t r a c e r".to_string()),
                (dim_style, String::new()),
                (tagline_style, "trace  .  replay  .  rewind".to_string()),
                (box_style, format!("\u{2514}{}\u{2518}", border)),
            ],
        };
        f.render_centered(area, frame.buffer_mut(), bg);
    })?;
    thread::sleep(Duration::from_millis(250));

    // Frame 5: System initialization
    let init_lines = vec![
        ("daemon", "ok"),
        ("snapshot store", "ok"),
        ("analysis engines", "ok"),
        ("claude integration", "ok"),
    ];

    for (i, (_label, _status)) in init_lines.iter().enumerate() {
        terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            let w = 44;
            let border = "\u{2500}".repeat(w - 2);

            let mut lines: Vec<(Style, String)> = vec![
                (box_style, format!("\u{250c}{}\u{2510}", border)),
                (logo_style, "v i b e t r a c e r".to_string()),
                (dim_style, String::new()),
                (tagline_style, "trace  .  replay  .  rewind".to_string()),
                (box_style, format!("\u{2514}{}\u{2518}", border)),
                (dim_style, String::new()),
            ];

            // Render all previous init lines plus current
            for j in 0..=i {
                let (l, s) = init_lines[j];
                let dots_len = 30_usize.saturating_sub(l.len() + s.len() + 6);
                let dots = ".".repeat(dots_len);
                // We'll render this as a plain string but the color is determined separately
                lines.push((dim_style, format!("[=] {} {} {}", l, dots, s)));
            }

            // Render centered
            let total_height = lines.len() as u16;
            let start_y = area.y + area.height.saturating_sub(total_height) / 2;

            // Fill bg
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_bg(bg);
                        cell.set_char(' ');
                    }
                }
            }

            for (idx, (style, text)) in lines.iter().enumerate() {
                let y = start_y + idx as u16;
                if y >= area.y + area.height {
                    break;
                }
                let text_width = text.chars().count() as u16;
                let x = area.x + area.width.saturating_sub(text_width) / 2;

                // For init lines, render the "ok" part in green
                if text.starts_with("[=]") {
                    let ok_pos = text.rfind(" ok").or_else(|| text.rfind(" ready"));
                    if let Some(pos) = ok_pos {
                        let (prefix, suffix) = text.split_at(pos + 1);
                        buf.set_string(x, y, prefix, dim_style);
                        let suffix_x = x + prefix.chars().count() as u16;
                        buf.set_string(suffix_x, y, suffix, green_style);
                    } else {
                        buf.set_string(x, y, text, *style);
                    }
                } else {
                    buf.set_string(x, y, text, *style);
                }
            }
        })?;
        thread::sleep(Duration::from_millis(80));
    }

    // Frame 6: Final "cockpit ready" line + brief flash
    terminal.draw(|frame| {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let w = 44;
        let border = "\u{2500}".repeat(w - 2);

        let mut lines: Vec<(Style, String)> = vec![
            (box_style, format!("\u{250c}{}\u{2510}", border)),
            (logo_style, "v i b e t r a c e r".to_string()),
            (dim_style, String::new()),
            (tagline_style, "trace  .  replay  .  rewind".to_string()),
            (box_style, format!("\u{2514}{}\u{2518}", border)),
            (dim_style, String::new()),
        ];

        let all_init = vec![
            ("daemon", "ok"),
            ("snapshot store", "ok"),
            ("analysis engines", "ok"),
            ("claude integration", "ok"),
            ("cockpit", "ready"),
        ];

        for (l, s) in &all_init {
            let dots_len = 30_usize.saturating_sub(l.len() + s.len() + 6);
            let dots = ".".repeat(dots_len);
            lines.push((dim_style, format!("[=] {} {} {}", l, dots, s)));
        }

        let total_height = lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(total_height) / 2;

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg);
                    cell.set_char(' ');
                }
            }
        }

        for (idx, (style, text)) in lines.iter().enumerate() {
            let y = start_y + idx as u16;
            if y >= area.y + area.height {
                break;
            }
            let text_width = text.chars().count() as u16;
            let x = area.x + area.width.saturating_sub(text_width) / 2;

            if text.starts_with("[=]") {
                let ok_pos = text.rfind(" ok").or_else(|| text.rfind(" ready"));
                if let Some(pos) = ok_pos {
                    let (prefix, suffix) = text.split_at(pos + 1);
                    buf.set_string(x, y, prefix, dim_style);
                    let suffix_x = x + prefix.chars().count() as u16;
                    let color = if suffix.contains("ready") {
                        Style::default().fg(accent).add_modifier(Modifier::BOLD)
                    } else {
                        green_style
                    };
                    buf.set_string(suffix_x, y, suffix, color);
                } else {
                    buf.set_string(x, y, text, *style);
                }
            } else {
                buf.set_string(x, y, text, *style);
            }
        }
    })?;
    thread::sleep(Duration::from_millis(350));

    Ok(())
}
