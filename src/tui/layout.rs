use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Holds the computed `Rect` regions for each major UI section.
pub struct AppLayout {
    pub status_bar: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub timeline: Rect,
    pub keybindings: Rect,
}

/// Compute the layout for the full terminal area.
///
/// Vertical split:
///   - 1 line  : status bar
///   - flexible: main area  (preview + optional sidebar)
///   - 8 lines : timeline
///   - 1 line  : keybindings bar
///
/// If `sidebar_visible`, the main area is split 65% / 35% horizontally.
pub fn compute_layout(area: Rect, sidebar_visible: bool) -> AppLayout {
    // ── top-level vertical split ─────────────────────────────────────────────
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(0),    // main area
            Constraint::Length(8), // timeline
            Constraint::Length(1), // keybindings
        ])
        .split(area);

    let status_bar = vertical[0];
    let main_area = vertical[1];
    let timeline = vertical[2];
    let keybindings = vertical[3];

    // ── horizontal split of main area ───────────────────────────────────────
    let (preview, sidebar) = if sidebar_visible {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main_area);
        (horizontal[0], Some(horizontal[1]))
    } else {
        (main_area, None)
    };

    AppLayout {
        status_bar,
        main_area,
        preview,
        sidebar,
        timeline,
        keybindings,
    }
}
