use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Holds the computed `Rect` regions for each major UI section.
pub struct AppLayout {
    pub status_bar: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub terminal: Option<Rect>,
    pub timeline: Rect,
    pub keybindings: Rect,
}

/// Compute the layout for the full terminal area.
///
/// Vertical split:
///   - 1 line  : status bar
///   - flexible: main area  (terminal + preview + optional sidebar)
///   - 8 lines : timeline
///   - 1 line  : keybindings bar
///
/// If `terminal_visible`, the main area is split 50% / 50% horizontally
/// between terminal (left) and preview (right). If sidebar is also visible,
/// the preview side is further split with the sidebar.
///
/// If only `sidebar_visible`, the main area is split 65% / 35% horizontally.
pub fn compute_layout(area: Rect, sidebar_visible: bool, terminal_visible: bool) -> AppLayout {
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
    let (terminal, preview, sidebar) = if terminal_visible && sidebar_visible {
        // terminal | preview | sidebar  — 40% / 35% / 25%
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(35),
                Constraint::Percentage(25),
            ])
            .split(main_area);
        (Some(horizontal[0]), horizontal[1], Some(horizontal[2]))
    } else if terminal_visible {
        // terminal | preview  — 50% / 50%
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_area);
        (Some(horizontal[0]), horizontal[1], None)
    } else if sidebar_visible {
        // preview | sidebar  — 65% / 35%
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main_area);
        (None, horizontal[0], Some(horizontal[1]))
    } else {
        (None, main_area, None)
    };

    AppLayout {
        status_bar,
        main_area,
        preview,
        sidebar,
        terminal,
        timeline,
        keybindings,
    }
}
