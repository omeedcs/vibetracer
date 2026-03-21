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

const MAX_WIDTH: u16 = 50;
const MAX_HEIGHT: u16 = 24;

/// All keybinding entries: (key, description).
const KEYBINDINGS: &[(&str, &str)] = &[
    ("q", "quit"),
    ("space", "toggle play / pause"),
    ("left", "scrub backward"),
    ("right", "scrub forward"),
    ("shift+left", "jump to previous checkpoint"),
    ("shift+right", "jump to next checkpoint"),
    ("1-5", "set playback speed"),
    ("r", "rewind to start"),
    ("R", "rewind file to last checkpoint"),
    ("u", "undo last rewind"),
    ("x", "cut range"),
    ("c", "create checkpoint"),
    ("s", "solo selected track"),
    ("m", "mute selected track"),
    ("g", "group by intent"),
    ("e", "toggle equation lens"),
    ("b", "toggle blast radius"),
    ("i", "toggle sentinels"),
    ("d", "toggle schema diff mode"),
    ("f", "toggle refactor tracker"),
    ("w", "toggle watchdog"),
    ("tab", "cycle focus"),
    ("/", "search"),
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
