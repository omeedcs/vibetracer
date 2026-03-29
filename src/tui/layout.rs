use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Holds the computed `Rect` regions for each major UI section.
#[derive(Clone)]
pub struct AppLayout {
    pub status_bar: Rect,
    pub sep_after_status: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
    pub dashboard: Option<Rect>,
    pub sep_after_main: Rect,
    pub timeline: Rect,
    pub sep_after_timeline: Rect,
    pub keybindings: Rect,
}

/// Compute the layout for the full terminal area.
///
/// Vertical split:
///   - 1 line  : status bar
///   - 1 line  : separator
///   - flexible: main area  (preview + optional sidebar/dashboard)
///   - 1 line  : separator
///   - 4-8 lines : timeline
///   - 1 line  : separator
///   - 1 line  : keybindings bar
///
/// The right panel can be either the old sidebar (blast radius, etc.)
/// or the new dashboard panel. Dashboard takes precedence when both are
/// requested.
pub fn compute_layout(
    area: Rect,
    sidebar_visible: bool,
    dashboard_visible: bool,
    conversation_visible: bool,
) -> AppLayout {
    let timeline_height = if area.height < 15 {
        4
    } else if area.height < 25 {
        6
    } else {
        8
    };

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),               // status bar
            Constraint::Length(1),               // separator
            Constraint::Min(3),                  // main area (minimum 3 rows)
            Constraint::Length(1),               // separator
            Constraint::Length(timeline_height), // timeline
            Constraint::Length(1),               // separator
            Constraint::Length(1),               // keybindings
        ])
        .split(area);

    let status_bar = vertical[0];
    let sep_after_status = vertical[1];
    let main_area = vertical[2];
    let sep_after_main = vertical[3];
    let timeline = vertical[4];
    let sep_after_timeline = vertical[5];
    let keybindings = vertical[6];

    let has_right_panel = dashboard_visible || sidebar_visible || conversation_visible;

    let (preview, sidebar, dashboard) = if has_right_panel {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main_area);

        if dashboard_visible || conversation_visible {
            // Dashboard or conversation takes the right panel
            (horizontal[0], None, Some(horizontal[1]))
        } else {
            // Old-style sidebar
            (horizontal[0], Some(horizontal[1]), None)
        }
    } else {
        (main_area, None, None)
    };

    AppLayout {
        status_bar,
        sep_after_status,
        main_area,
        preview,
        sidebar,
        dashboard,
        sep_after_main,
        timeline,
        sep_after_timeline,
        keybindings,
    }
}
