use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::claude_log::{ConversationTurn, ToolCall};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// ConversationState
// ---------------------------------------------------------------------------

/// Persistent state for the conversation panel (scroll, selection, expansion).
pub struct ConversationState {
    pub scroll: usize,
    pub selected_turn: Option<usize>,
    /// Which turns have their tool-call tree expanded (all by default).
    pub expanded_turns: HashSet<usize>,
}

impl ConversationState {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            selected_turn: None,
            expanded_turns: HashSet::new(),
        }
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_add(n);
    }

    pub fn select_next(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        self.selected_turn = Some(match self.selected_turn {
            Some(i) if i + 1 < max => i + 1,
            Some(_) => max - 1,
            None => 0,
        });
    }

    pub fn select_prev(&mut self) {
        self.selected_turn = match self.selected_turn {
            Some(i) if i > 0 => Some(i - 1),
            Some(_) => Some(0),
            None => None,
        };
    }

    pub fn toggle_expand(&mut self, idx: usize) {
        if !self.expanded_turns.remove(&idx) {
            self.expanded_turns.insert(idx);
        }
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConversationPanel (widget)
// ---------------------------------------------------------------------------

/// A vertical conversation timeline showing user prompts, tool-call trees, and
/// assistant responses.
pub struct ConversationPanel<'a> {
    pub turns: &'a [ConversationTurn],
    pub scroll: usize,
    pub theme: &'a Theme,
    pub selected_turn: Option<usize>,
}

impl<'a> ConversationPanel<'a> {
    pub fn new(
        turns: &'a [ConversationTurn],
        scroll: usize,
        theme: &'a Theme,
        selected_turn: Option<usize>,
    ) -> Self {
        Self {
            turns,
            scroll,
            theme,
            selected_turn,
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl Widget for ConversationPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let t = self.theme;
        let max_width = area.width as usize;

        // Pre-build all visual lines for every turn.
        let mut all_lines: Vec<(Line<'_>, Option<usize>)> = Vec::new();

        for (turn_idx, turn) in self.turns.iter().enumerate() {
            let ts = format_hms(turn.timestamp);
            let is_selected = self.selected_turn == Some(turn_idx);

            // ── USER line ──────────────────────────────────────────────
            {
                let prompt_budget = max_width
                    .saturating_sub(ts.len() + 2 /* spaces */ + 6 /* " USER " */);
                let prompt = truncate(&turn.user_prompt, prompt_budget);

                let spans = vec![
                    Span::styled(ts.clone(), Style::default().fg(t.fg_dim)),
                    Span::styled("  USER   ", Style::default().fg(t.fg)),
                    Span::styled(
                        format!("\"{}\"", prompt),
                        Style::default().fg(t.fg_muted),
                    ),
                ];
                all_lines.push((Line::from(spans), Some(turn_idx)));
            }

            // ── CLAUDE [thinking Xs] ───────────────────────────────────
            {
                let duration_secs = turn.duration_ms as f64 / 1000.0;
                let thinking_label = if duration_secs >= 0.1 {
                    format!("[thinking {:.1}s]", duration_secs)
                } else {
                    "[thinking]".to_string()
                };
                let spans = vec![
                    Span::styled(ts.clone(), Style::default().fg(t.fg_dim)),
                    Span::styled("  CLAUDE ", Style::default().fg(t.accent_blue)),
                    Span::styled(thinking_label, Style::default().fg(t.accent_blue)),
                ];
                all_lines.push((Line::from(spans), Some(turn_idx)));
            }

            // ── Tool call tree ─────────────────────────────────────────
            let tool_count = turn.tool_calls.len();
            for (tc_idx, tc) in turn.tool_calls.iter().enumerate() {
                let is_last = tc_idx + 1 == tool_count;
                let branch = if is_last { "\u{2514}\u{2500} " } else { "\u{251C}\u{2500} " };

                let indent = "  ";
                let line = render_tool_call(tc, branch, indent, t, max_width, is_selected);
                all_lines.push((line, Some(turn_idx)));
            }

            // ── CLAUDE [complete] "response..." ────────────────────────
            {
                let response_budget = max_width
                    .saturating_sub(ts.len() + 2 + 9 /* " CLAUDE " */ + 12 /* "[complete] " */);
                let response = truncate(&turn.assistant_text, response_budget);

                let spans = vec![
                    Span::styled(ts, Style::default().fg(t.fg_dim)),
                    Span::styled("  CLAUDE ", Style::default().fg(t.accent_blue)),
                    Span::styled("[complete]  ", Style::default().fg(t.fg_dim)),
                    Span::styled(
                        format!("\"{}\"", response),
                        Style::default().fg(t.fg_muted),
                    ),
                ];
                all_lines.push((Line::from(spans), Some(turn_idx)));
            }
        }

        // ── Apply scroll and render ────────────────────────────────────
        let visible = all_lines
            .into_iter()
            .skip(self.scroll)
            .take(area.height as usize);

        for (row_offset, (line, maybe_turn)) in visible.enumerate() {
            let y = area.y + row_offset as u16;

            let is_selected = maybe_turn
                .map(|ti| self.selected_turn == Some(ti))
                .unwrap_or(false);

            if is_selected {
                // Paint a subtle highlight across the full row.
                let bg = selection_bg(t.accent_warm);
                for x in area.x..area.x + area.width {
                    buf[(x, y)].set_style(Style::default().bg(bg));
                }
            }

            line.render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render a single tool-call line with tree-drawing characters.
fn render_tool_call<'a>(
    tc: &ToolCall,
    branch: &str,
    indent: &str,
    t: &Theme,
    _max_width: usize,
    _is_selected: bool,
) -> Line<'a> {
    let name_lower = tc.tool_name.to_lowercase();

    let (tool_color, detail) = if name_lower.contains("edit") || name_lower.contains("write") {
        let path = tc.file_path.clone().unwrap_or_default();
        let added = tc.lines_added.unwrap_or(0);
        let removed = tc.lines_removed.unwrap_or(0);
        let detail = format_edit_counts(added, removed);
        (t.accent_warm, (path, Some(detail)))
    } else if name_lower.contains("read") {
        let path = tc.file_path.clone().unwrap_or_default();
        (t.fg_dim, (path, None))
    } else if name_lower.contains("grep") {
        let path = tc.file_path.clone().unwrap_or_default();
        let summary = extract_match_count(&tc.result_summary);
        let display = if path.is_empty() {
            format!("\"{}\"", tc.result_summary)
        } else {
            format!("\"{}\"", path)
        };
        (t.accent_blue, (display, Some(summary)))
    } else if name_lower.contains("bash") {
        let summary = tc.result_summary.clone();
        (t.accent_purple, (summary, None))
    } else {
        // Generic: tool_name + result_summary
        let display = format!("{} {}", tc.tool_name, tc.result_summary);
        (t.fg_dim, (display, None))
    };

    let tool_label = tool_display_name(&tc.tool_name);

    let mut spans: Vec<Span<'a>> = vec![
        Span::styled(
            format!("{}{}", indent, branch),
            Style::default().fg(t.fg_dim),
        ),
        Span::styled(
            format!("{:<6}", tool_label),
            Style::default().fg(tool_color),
        ),
    ];

    // File path / primary detail
    if !detail.0.is_empty() {
        spans.push(Span::styled(
            detail.0.to_string(),
            Style::default().fg(t.accent_warm),
        ));
    }

    // Line counts or secondary detail
    if let Some(secondary) = detail.1 {
        spans.push(Span::raw(" "));
        // Parse +N -M for colored rendering
        for part in secondary.split_whitespace() {
            if part.starts_with('+') {
                spans.push(Span::styled(
                    part.to_string(),
                    Style::default().fg(t.accent_green),
                ));
            } else if part.starts_with('-') {
                spans.push(Span::styled(
                    part.to_string(),
                    Style::default().fg(t.accent_red),
                ));
            } else {
                spans.push(Span::styled(
                    part.to_string(),
                    Style::default().fg(t.fg_muted),
                ));
            }
            spans.push(Span::raw(" "));
        }
    }

    Line::from(spans)
}

/// Format added/removed counts as "+N -M", omitting zeros.
fn format_edit_counts(added: u32, removed: u32) -> String {
    match (added, removed) {
        (0, 0) => String::new(),
        (a, 0) => format!("+{}", a),
        (0, r) => format!("-{}", r),
        (a, r) => format!("+{} -{}", a, r),
    }
}

/// Extract a match-count string from a grep result summary.
/// Looks for a number followed by "match" (e.g. "12 matches").
fn extract_match_count(summary: &str) -> String {
    // Try to find "N match" pattern.
    for word in summary.split_whitespace() {
        if let Ok(n) = word.parse::<u64>() {
            return format!("{} matches", n);
        }
    }
    summary.to_string()
}

/// Return a short display name for the tool.
fn tool_display_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("edit") {
        "Edit".to_string()
    } else if lower.contains("write") {
        "Write".to_string()
    } else if lower.contains("read") {
        "Read".to_string()
    } else if lower.contains("grep") {
        "Grep".to_string()
    } else if lower.contains("bash") {
        "Bash".to_string()
    } else {
        // Use the raw name, capped at 10 chars.
        truncate(name, 10).to_string()
    }
}

/// Format a unix-ms timestamp as HH:MM:SS.
fn format_hms(unix_ms: i64) -> String {
    let total_secs = unix_ms / 1000;
    let h = (total_secs % 86400) / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Truncate a string to at most `max` characters, appending "..." if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        s.chars().take(max).collect()
    } else {
        let mut out: String = s.chars().take(max - 3).collect();
        out.push_str("...");
        out
    }
}

/// Derive a dim background tint from the accent color for selection highlight.
fn selection_bg(accent: ratatui::style::Color) -> ratatui::style::Color {
    match accent {
        ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(
            ((r as u16) * 30 / 255) as u8,
            ((g as u16) * 30 / 255) as u8,
            ((b as u16) * 30 / 255) as u8,
        ),
        _ => ratatui::style::Color::Rgb(8, 6, 5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hms() {
        // 14:32:01 UTC
        let ts = (14 * 3600 + 32 * 60 + 1) * 1000_i64;
        assert_eq!(format_hms(ts), "14:32:01");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world!", 8), "hello...");
    }

    #[test]
    fn test_format_edit_counts() {
        assert_eq!(format_edit_counts(14, 8), "+14 -8");
        assert_eq!(format_edit_counts(45, 0), "+45");
        assert_eq!(format_edit_counts(0, 3), "-3");
        assert_eq!(format_edit_counts(0, 0), "");
    }

    #[test]
    fn test_conversation_state_scroll() {
        let mut state = ConversationState::new();
        state.scroll_down(5);
        assert_eq!(state.scroll, 5);
        state.scroll_up(3);
        assert_eq!(state.scroll, 2);
        state.scroll_up(100);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn test_conversation_state_selection() {
        let mut state = ConversationState::new();
        assert_eq!(state.selected_turn, None);

        state.select_next(5);
        assert_eq!(state.selected_turn, Some(0));

        state.select_next(5);
        assert_eq!(state.selected_turn, Some(1));

        state.select_prev();
        assert_eq!(state.selected_turn, Some(0));

        state.select_prev();
        assert_eq!(state.selected_turn, Some(0)); // clamps at 0
    }

    #[test]
    fn test_conversation_state_toggle_expand() {
        let mut state = ConversationState::new();
        assert!(!state.expanded_turns.contains(&0));

        state.toggle_expand(0);
        assert!(state.expanded_turns.contains(&0));

        state.toggle_expand(0);
        assert!(!state.expanded_turns.contains(&0));
    }

    #[test]
    fn test_select_next_empty() {
        let mut state = ConversationState::new();
        state.select_next(0);
        assert_eq!(state.selected_turn, None);
    }

    #[test]
    fn test_select_next_clamps_at_max() {
        let mut state = ConversationState::new();
        state.selected_turn = Some(4);
        state.select_next(5);
        assert_eq!(state.selected_turn, Some(4)); // max-1
    }
}
