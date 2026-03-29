use crate::theme::Theme;
use crate::tui::widgets::sparkline::{format_sparkline, SparklineBuffer};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

// ── Data model ─────────────────────────────────────────────────────────────

/// All data required by the dashboard panel, updated each tick.
pub struct DashboardState {
    // Token tracking
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_hit_pct: f64,
    pub token_rate: SparklineBuffer,

    // Cost tracking
    pub total_cost: f64,
    pub cost_rate: SparklineBuffer,

    // Edit velocity
    pub edit_velocity: f64,
    pub velocity_sparkline: SparklineBuffer,

    // File heatmap: (filename, edit_count) sorted by count desc
    pub file_heat: Vec<(String, u32)>,

    // Agent status: (agent_label, edit_count, is_active)
    pub agent_status: Vec<(String, u32, bool)>,

    // Operations: (name, edit_count, is_active)
    pub operations: Vec<(String, u32, bool)>,

    // Analysis summaries
    pub stale_count: u32,
    pub updated_count: u32,
    pub untouched_count: u32,
    pub stale_files: Vec<String>,

    pub sentinel_pass: u32,
    pub sentinel_fail: u32,
    pub sentinel_failures: Vec<String>,

    pub watchdog_ok: bool,
    pub watchdog_alerts: Vec<String>,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            tokens_in: 0,
            tokens_out: 0,
            cache_hit_pct: 0.0,
            token_rate: SparklineBuffer::new(20),
            total_cost: 0.0,
            cost_rate: SparklineBuffer::new(20),
            edit_velocity: 0.0,
            velocity_sparkline: SparklineBuffer::new(20),
            file_heat: Vec::new(),
            agent_status: Vec::new(),
            operations: Vec::new(),
            stale_count: 0,
            updated_count: 0,
            untouched_count: 0,
            stale_files: Vec::new(),
            sentinel_pass: 0,
            sentinel_fail: 0,
            sentinel_failures: Vec::new(),
            watchdog_ok: true,
            watchdog_alerts: Vec::new(),
        }
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Format a count as a compact human-readable string: "1.2k", "45.2k", "1.2M".
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        if m >= 100.0 {
            format!("{:.0}M", m)
        } else if m >= 10.0 {
            format!("{:.1}M", m)
        } else {
            format!("{:.1}M", m)
        }
    } else if n >= 1_000 {
        let k = n as f64 / 1_000.0;
        if k >= 100.0 {
            format!("{:.0}k", k)
        } else if k >= 10.0 {
            format!("{:.1}k", k)
        } else {
            format!("{:.1}k", k)
        }
    } else {
        format!("{}", n)
    }
}

/// Build a horizontal bar string of `width` characters, filled proportionally.
fn bar(filled: usize, total: usize, width: usize) -> (String, String) {
    let ratio = if total == 0 {
        0.0
    } else {
        filled as f64 / total as f64
    };
    let fill = (ratio * width as f64).round() as usize;
    let fill = fill.min(width);
    let empty = width - fill;
    (
        "\u{2588}".repeat(fill),
        "\u{2591}".repeat(empty),
    )
}

/// Truncate a string to at most `max_len` characters, appending nothing.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Render a single line at (area.x, y) if y < area.y + area.height.
fn render_at(line: Line, area: Rect, y: u16, buf: &mut Buffer) {
    if y >= area.y + area.height {
        return;
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

// ── Widget ─────────────────────────────────────────────────────────────────

/// Dense htop-style dashboard panel for real-time session stats.
pub struct DashboardPanel<'a> {
    pub state: &'a DashboardState,
    pub theme: &'a Theme,
}

impl<'a> DashboardPanel<'a> {
    pub fn new(state: &'a DashboardState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for DashboardPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };

        let w = area.width as usize;
        let muted = Style::default().fg(self.theme.fg_muted);
        let fg = Style::default().fg(self.theme.fg);
        let dim = Style::default().fg(self.theme.fg_dim);
        let red = Style::default().fg(self.theme.accent_red);
        let green = Style::default().fg(self.theme.accent_green);
        let warm = Style::default().fg(self.theme.accent_warm);
        let bar_fill_style = Style::default().fg(self.theme.bar_filled);
        let bar_empty_style = Style::default().fg(self.theme.bar_empty);

        let mut row = area.y;
        let max_y = area.y + area.height;

        // Macro-ish helper: bail if out of vertical space.
        macro_rules! need {
            ($n:expr) => {
                if row + $n > max_y {
                    return;
                }
            };
        }

        // ── TOKENS ─────────────────────────────────────────────────────

        need!(2);

        let in_str = format_count(self.state.tokens_in);
        let out_str = format_count(self.state.tokens_out);
        render_at(
            Line::from(vec![
                Span::styled("TOKENS", muted),
                Span::styled("          in:", dim),
                Span::styled(in_str, fg),
                Span::styled("  out:", dim),
                Span::styled(out_str, fg),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        need!(1);
        let spark_w = w.saturating_sub(12);
        let spark = format_sparkline(&self.state.token_rate.data(), spark_w);
        let cache_str = format!("  cache:{}%", self.state.cache_hit_pct as u64);
        render_at(
            Line::from(vec![
                Span::styled(spark, Style::default().fg(self.theme.accent_blue)),
                Span::styled(cache_str, dim),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        // blank separator
        if row < max_y {
            row += 1;
        }

        // ── COST ───────────────────────────────────────────────────────

        need!(2);

        let cost_str = format!("${:.2}", self.state.total_cost);
        let rate_data = self.state.cost_rate.data();
        let per_min = if rate_data.is_empty() {
            0.0
        } else {
            *rate_data.last().unwrap()
        };
        let rate_str = format!("  (${:.2}/min)", per_min);
        render_at(
            Line::from(vec![
                Span::styled("COST", muted),
                Span::styled("            ", dim),
                Span::styled(cost_str, fg),
                Span::styled(rate_str, dim),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        need!(1);
        let spark = format_sparkline(&self.state.cost_rate.data(), w);
        render_at(
            Line::from(vec![Span::styled(
                spark,
                Style::default().fg(self.theme.accent_warm),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        // blank separator
        if row < max_y {
            row += 1;
        }

        // ── VELOCITY ───────────────────────────────────────────────────

        need!(2);

        let vel_str = format!("{:.1} edits/min", self.state.edit_velocity);
        render_at(
            Line::from(vec![
                Span::styled("VELOCITY", muted),
                Span::styled("        ", dim),
                Span::styled(vel_str, fg),
            ]),
            area,
            row,
            buf,
        );
        row += 1;

        need!(1);
        let spark = format_sparkline(&self.state.velocity_sparkline.data(), w);
        render_at(
            Line::from(vec![Span::styled(
                spark,
                Style::default().fg(self.theme.accent_green),
            )]),
            area,
            row,
            buf,
        );
        row += 1;

        // blank separator
        if row < max_y {
            row += 1;
        }

        // ── FILES ──────────────────────────────────────────────────────

        if !self.state.file_heat.is_empty() {
            need!(2);

            let total_files = self.state.file_heat.len();
            let hot = self.state.file_heat.iter().filter(|(_, c)| *c >= 5).count();
            let header = format!(
                "FILES   {} touched   {} hot",
                total_files, hot,
            );
            render_at(
                Line::from(vec![Span::styled(header, muted)]),
                area,
                row,
                buf,
            );
            row += 1;

            let max_edit = self.state.file_heat.first().map(|(_, c)| *c).unwrap_or(1).max(1);
            let bar_width = 10;
            let show = self.state.file_heat.len().min(5);

            for (name, count) in self.state.file_heat.iter().take(show) {
                if row >= max_y {
                    break;
                }
                let truncated = truncate(name, 18);
                let padded = format!("{:<18}", truncated);
                let (filled, empty) = bar(*count as usize, max_edit as usize, bar_width);
                let count_str = format!("{:>3}", count);
                render_at(
                    Line::from(vec![
                        Span::styled(padded, fg),
                        Span::styled(" ", dim),
                        Span::styled(filled, bar_fill_style),
                        Span::styled(empty, bar_empty_style),
                        Span::styled(format!(" {}", count_str), fg),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }

            if self.state.file_heat.len() > show && row < max_y {
                let remaining = self.state.file_heat.len() - show;
                render_at(
                    Line::from(vec![Span::styled(
                        format!("(+{} more)", remaining),
                        dim,
                    )]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }

            // blank separator
            if row < max_y {
                row += 1;
            }
        }

        // ── AGENTS ─────────────────────────────────────────────────────

        if !self.state.agent_status.is_empty() {
            need!(2);

            render_at(
                Line::from(vec![Span::styled("AGENTS", muted)]),
                area,
                row,
                buf,
            );
            row += 1;

            let max_edit = self
                .state
                .agent_status
                .iter()
                .map(|(_, c, _)| *c)
                .max()
                .unwrap_or(1)
                .max(1);
            let bar_width = 12;

            for (i, (label, count, active)) in self.state.agent_status.iter().enumerate() {
                if row >= max_y {
                    break;
                }
                let padded = format!("{:<10}", truncate(label, 10));
                let (filled, empty) = bar(*count as usize, max_edit as usize, bar_width);
                let status = if *active { "aktv" } else { "idle" };
                let status_style = if *active { green } else { dim };
                let agent_color = Style::default().fg(
                    self.theme.agent_colors[i % self.theme.agent_colors.len()],
                );
                render_at(
                    Line::from(vec![
                        Span::styled(padded, agent_color),
                        Span::styled(filled, bar_fill_style),
                        Span::styled(empty, bar_empty_style),
                        Span::styled(format!(" {:>3}", count), fg),
                        Span::styled(format!("  {}", status), status_style),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            }

            // blank separator
            if row < max_y {
                row += 1;
            }
        }

        // ── OPS ────────────────────────────────────────────────────────

        if !self.state.operations.is_empty() {
            need!(2);

            let total_ops = self.state.operations.len();
            let active_ops = self.state.operations.iter().filter(|(_, _, a)| *a).count();
            render_at(
                Line::from(vec![Span::styled(
                    format!("OPS         {} total   {} active", total_ops, active_ops),
                    muted,
                )]),
                area,
                row,
                buf,
            );
            row += 1;

            let max_edit = self
                .state
                .operations
                .iter()
                .map(|(_, c, _)| *c)
                .max()
                .unwrap_or(1)
                .max(1);
            let bar_width = 7;

            for (name, count, active) in &self.state.operations {
                if row >= max_y {
                    break;
                }
                let prefix = if *active { "\u{25b8} " } else { "  " };
                let truncated = truncate(name, 18);
                let padded = format!("{:<18}", truncated);

                // Determine trailing label
                let done = !*active && *count > 0;
                let pending = !*active && *count == 0;

                let (filled, empty) = bar(*count as usize, max_edit as usize, bar_width);

                let mut spans = vec![
                    Span::styled(prefix, if *active { warm } else { dim }),
                    Span::styled(padded, fg),
                    Span::styled(" ", dim),
                ];

                if pending {
                    spans.push(Span::styled("pending", dim));
                } else {
                    spans.push(Span::styled(filled, bar_fill_style));
                    spans.push(Span::styled(empty, bar_empty_style));
                    if done {
                        spans.push(Span::styled(" done", green));
                    } else {
                        spans.push(Span::styled(format!(" {}", count), fg));
                    }
                }

                render_at(Line::from(spans), area, row, buf);
                row += 1;
            }

            // blank separator
            if row < max_y {
                row += 1;
            }
        }

        // ── BLAST ──────────────────────────────────────────────────────

        {
            let has_data = self.state.stale_count > 0
                || self.state.updated_count > 0
                || self.state.untouched_count > 0;

            if has_data {
                need!(1);

                render_at(
                    Line::from(vec![
                        Span::styled("BLAST", muted),
                        Span::styled("  stale:", dim),
                        Span::styled(
                            format!("{}", self.state.stale_count),
                            if self.state.stale_count > 0 { red } else { fg },
                        ),
                        Span::styled(" updated:", dim),
                        Span::styled(format!("{}", self.state.updated_count), fg),
                        Span::styled(" untch:", dim),
                        Span::styled(format!("{}", self.state.untouched_count), fg),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;

                for file in &self.state.stale_files {
                    if row >= max_y {
                        break;
                    }
                    render_at(
                        Line::from(vec![
                            Span::styled("\u{26a0} ", red),
                            Span::styled(truncate(file, w.saturating_sub(3)), red),
                        ]),
                        area,
                        row,
                        buf,
                    );
                    row += 1;
                }

                // blank separator
                if row < max_y {
                    row += 1;
                }
            }
        }

        // ── SENTINELS ──────────────────────────────────────────────────

        {
            let has_data = self.state.sentinel_pass > 0 || self.state.sentinel_fail > 0;

            if has_data {
                need!(1);

                let fail_style = if self.state.sentinel_fail > 0 { red } else { fg };
                render_at(
                    Line::from(vec![
                        Span::styled("SENTINELS", muted),
                        Span::styled(
                            format!("     {} pass", self.state.sentinel_pass),
                            green,
                        ),
                        Span::styled("  ", dim),
                        Span::styled(
                            if self.state.sentinel_fail > 0 {
                                format!("{} FAIL", self.state.sentinel_fail)
                            } else {
                                format!("{} fail", self.state.sentinel_fail)
                            },
                            fail_style,
                        ),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;

                for failure in &self.state.sentinel_failures {
                    if row >= max_y {
                        break;
                    }
                    render_at(
                        Line::from(vec![
                            Span::styled("\u{2717} ", red),
                            Span::styled(truncate(failure, w.saturating_sub(3)), red),
                        ]),
                        area,
                        row,
                        buf,
                    );
                    row += 1;
                }

                // blank separator
                if row < max_y {
                    row += 1;
                }
            }
        }

        // ── WATCHDOG ───────────────────────────────────────────────────

        {
            need!(1);

            if self.state.watchdog_ok && self.state.watchdog_alerts.is_empty() {
                render_at(
                    Line::from(vec![
                        Span::styled("WATCHDOG", muted),
                        Span::styled("      all clear", green),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;
            } else {
                let alert_count = self.state.watchdog_alerts.len();
                render_at(
                    Line::from(vec![
                        Span::styled("WATCHDOG", muted),
                        Span::styled(format!("      {} alert(s)", alert_count), red),
                    ]),
                    area,
                    row,
                    buf,
                );
                row += 1;

                for alert in &self.state.watchdog_alerts {
                    if row >= max_y {
                        break;
                    }
                    render_at(
                        Line::from(vec![
                            Span::styled("\u{26a0} ", red),
                            Span::styled(truncate(alert, w.saturating_sub(3)), red),
                        ]),
                        area,
                        row,
                        buf,
                    );
                    row += 1;
                }
            }
        }

        // `row` may have advanced past max_y in the final section; that is
        // handled by `render_at` which no-ops when y >= max_y.
        let _ = row;
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_count_small() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(1_000), "1.0k");
        assert_eq!(format_count(1_200), "1.2k");
        assert_eq!(format_count(32_100), "32.1k");
        assert_eq!(format_count(999_999), "1000k");
    }

    #[test]
    fn format_count_millions() {
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(1_200_000), "1.2M");
        assert_eq!(format_count(45_200_000), "45.2M");
    }

    #[test]
    fn bar_zero_total() {
        let (filled, empty) = bar(0, 0, 10);
        assert_eq!(filled.chars().count(), 0);
        assert_eq!(empty.chars().count(), 10);
    }

    #[test]
    fn bar_full() {
        let (filled, empty) = bar(10, 10, 10);
        assert_eq!(filled.chars().count(), 10);
        assert_eq!(empty.chars().count(), 0);
    }

    #[test]
    fn bar_half() {
        let (filled, empty) = bar(5, 10, 10);
        assert_eq!(filled.chars().count() + empty.chars().count(), 10);
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long() {
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn dashboard_state_default() {
        let state = DashboardState::new();
        assert_eq!(state.tokens_in, 0);
        assert_eq!(state.total_cost, 0.0);
        assert!(state.file_heat.is_empty());
        assert!(state.watchdog_ok);
    }

    #[test]
    fn render_zero_area() {
        let state = DashboardState::new();
        let theme = Theme::default();
        let panel = DashboardPanel::new(&state, &theme);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
        // Should not panic.
    }

    #[test]
    fn render_minimal_area() {
        let state = DashboardState::new();
        let theme = Theme::default();
        let panel = DashboardPanel::new(&state, &theme);
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
        // Should not panic; only first section header + sparkline rendered.
    }
}
