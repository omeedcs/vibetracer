use std::collections::VecDeque;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

const BLOCKS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

/// Map a value within `[min, max]` to one of the 8 block characters.
fn value_to_block(value: f64, min: f64, max: f64) -> char {
    if max <= min {
        return BLOCKS[0];
    }
    let normalized = (value - min) / (max - min);
    let idx = (normalized * 7.0).round() as usize;
    BLOCKS[idx.min(7)]
}

// ── Sparkline widget ────────────────────────────────────────────────────────

/// A minimal sparkline widget that renders a `Vec<f64>` as unicode block
/// characters.  Auto-scales to the data range and pads or truncates to fit
/// the target width.
pub struct Sparkline {
    data: Vec<f64>,
    color: Color,
}

impl Sparkline {
    pub fn new(data: Vec<f64>, color: Color) -> Self {
        Self { data, color }
    }
}

impl Widget for Sparkline {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let width = area.width as usize;
        let line = format_sparkline(&self.data, width);
        let style = Style::default().fg(self.color);

        for (i, ch) in line.chars().enumerate() {
            if i >= width {
                break;
            }
            buf.set_string(area.x + i as u16, area.y, ch.to_string(), style);
        }
    }
}

// ── SparklineBuffer ─────────────────────────────────────────────────────────

/// Fixed-capacity ring buffer for streaming sparkline data.  When full, the
/// oldest value is dropped on each `push`.
pub struct SparklineBuffer {
    buf: VecDeque<f64>,
    capacity: usize,
}

impl SparklineBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(value);
    }

    /// Return the current contents as a contiguous slice pair.  Because
    /// `VecDeque::make_contiguous` requires `&mut self`, we expose a helper
    /// that returns a `Vec` reference instead.
    pub fn data(&self) -> Vec<f64> {
        self.buf.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

// ── Helper ──────────────────────────────────────────────────────────────────

/// Render `data` as a sparkline string of exactly `width` characters.
///
/// If the data is longer than `width`, only the last `width` values are used
/// (most-recent window).  If shorter, the string is left-padded with spaces.
pub fn format_sparkline(data: &[f64], width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    // Take the trailing window when data exceeds width.
    let slice = if data.len() > width {
        &data[data.len() - width..]
    } else {
        data
    };

    if slice.is_empty() {
        return " ".repeat(width);
    }

    let min = slice.iter().copied().fold(f64::INFINITY, f64::min);
    let max = slice.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let pad = width.saturating_sub(slice.len());
    let mut out = " ".repeat(pad);

    for &v in slice {
        out.push(value_to_block(v, min, max));
    }

    out
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_empty_data() {
        let s = format_sparkline(&[], 10);
        assert_eq!(s.chars().count(), 10);
        assert!(s.chars().all(|c| c == ' '));
    }

    #[test]
    fn format_single_value() {
        let s = format_sparkline(&[42.0], 5);
        assert_eq!(s.chars().count(), 5);
        // Single value maps to lowest block (range is zero).
        assert_eq!(s.chars().last().unwrap(), BLOCKS[0]);
    }

    #[test]
    fn format_ascending() {
        let data: Vec<f64> = (0..8).map(|i| i as f64).collect();
        let s = format_sparkline(&data, 8);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 8);
        assert_eq!(chars[0], BLOCKS[0]);
        assert_eq!(chars[7], BLOCKS[7]);
    }

    #[test]
    fn format_truncates_to_width() {
        let data: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let s = format_sparkline(&data, 5);
        assert_eq!(s.chars().count(), 5);
    }

    #[test]
    fn format_pads_short_data() {
        let s = format_sparkline(&[1.0, 2.0], 6);
        assert_eq!(s.chars().count(), 6);
        assert_eq!(&s[..4], "    ");
    }

    #[test]
    fn ring_buffer_capacity() {
        let mut rb = SparklineBuffer::new(3);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        rb.push(4.0);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.data(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn ring_buffer_empty() {
        let rb = SparklineBuffer::new(5);
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }
}
