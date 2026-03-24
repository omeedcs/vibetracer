use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

const COLOR_KEY: Color = Color::Rgb(90, 101, 119);
const COLOR_DESC: Color = Color::Rgb(160, 168, 183);
const COLOR_BG: Color = Color::Rgb(15, 17, 21);
const COLOR_BORDER: Color = Color::Rgb(30, 34, 42);
const COLOR_SECTION: Color = Color::Rgb(138, 117, 96);

const MAX_WIDTH: u16 = 52;
const MAX_HEIGHT: u16 = 30;

/// All keybinding entries: (key, description).
/// Empty key + description = section header.
const KEYBINDINGS: &[(&str, &str)] = &[
    // ── playback ──
    ("", "-- playback --"),
    ("space", "toggle play / pause"),
    ("left / right", "scrub timeline"),
    ("shift+left", "scrub file backward"),
    ("shift+right", "scrub file forward"),
    ("a", "reattach file to global"),
    // ── editing ──
    ("", "-- actions --"),
    ("R", "restore file to playhead"),
    ("u", "undo last restore"),
    ("c", "create checkpoint"),
    ("x", "toggle restore edits"),
    // ── view ──
    ("", "-- view --"),
    ("g", "toggle command view"),
    ("s", "solo track"),
    ("m", "mute track"),
    ("1-9", "solo agent N"),
    ("t", "cycle theme"),
    // ── panels ──
    ("", "-- panels --"),
    ("b", "toggle blast radius"),
    ("i", "toggle sentinels"),
    ("w", "toggle watchdog"),
    ("tab", "cycle focus"),
    // ── meta ──
    ("", "-- meta --"),
    ("q", "quit"),
    ("Q", "quit and stop daemon"),
    ("?", "show this help"),
];

/// A centered overlay that renders all keybindings.
pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Compute centered bounding box.
        let width = MAX_WIDTH.min(area.width);
        let height = MAX_HEIGHT.min(area.height);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;

        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        // Clear the area first.
        Clear.render(overlay_area, buf);

        // Draw the bordered block.
        let block = Block::default()
            .title(" keybindings ")
            .borders(Borders::ALL)
            .style(Style::default().bg(COLOR_BG).fg(COLOR_BORDER));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Render keybinding rows inside the block.
        let key_col_width: u16 = 18;
        let gap: u16 = 2;

        for (i, (key, desc)) in KEYBINDINGS.iter().enumerate() {
            let row_y = inner.y + i as u16;
            if row_y >= inner.y + inner.height {
                break;
            }

            let row_area = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };

            if key.is_empty() {
                // Section header
                let line = Line::from(vec![
                    Span::raw(format!("{:width$}", "", width = (key_col_width + gap) as usize)),
                    Span::styled(*desc, Style::default().fg(COLOR_SECTION)),
                ]);
                line.render(row_area, buf);
            } else {
                // Right-align key in the first column.
                let key_str = format!("{:>width$}", key, width = key_col_width as usize);

                let line = Line::from(vec![
                    Span::styled(key_str, Style::default().fg(COLOR_KEY)),
                    Span::raw(format!("{:width$}", "", width = gap as usize)),
                    Span::styled(*desc, Style::default().fg(COLOR_DESC)),
                ]);

                line.render(row_area, buf);
            }
        }
    }
}
