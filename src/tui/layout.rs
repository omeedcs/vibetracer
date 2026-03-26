use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Holds the computed `Rect` regions for each major UI section.
#[derive(Clone)]
pub struct AppLayout {
    pub status_bar: Rect,
    pub sep_after_status: Rect,
    pub main_area: Rect,
    pub preview: Rect,
    pub sidebar: Option<Rect>,
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
///   - flexible: main area  (preview + optional sidebar)
///   - 1 line  : separator
///   - 8 lines : timeline
///   - 1 line  : separator
///   - 1 line  : keybindings bar
///
/// If `sidebar_visible`, the main area is split 65% / 35% horizontally.
pub fn compute_layout(area: Rect, sidebar_visible: bool) -> AppLayout {
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
        sep_after_status,
        main_area,
        preview,
        sidebar,
        sep_after_main,
        timeline,
        sep_after_timeline,
        keybindings,
    }
}
