use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::{App, Pane, PlaybackState, SidebarPanel};

/// All actions that a keypress can trigger.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    TogglePlay,
    ScrubLeft,
    ScrubRight,
    JumpPrevCheckpoint,
    JumpNextCheckpoint,
    SetSpeed(u8),
    Rewind,
    RewindFile,
    UndoRewind,
    CutRange,
    Checkpoint,
    SoloTrack,
    MuteTrack,
    GroupByIntent,
    ToggleBlastRadius,
    ToggleSentinels,
    ToggleWatchdog,
    CycleFocus,
    Search,
    Help,
    None,
}

/// Map a crossterm `KeyEvent` to an `Action`.
pub fn map_key(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Action::Quit,
        (KeyCode::Char(' '), _) => Action::TogglePlay,
        (KeyCode::Left, KeyModifiers::SHIFT) => Action::JumpPrevCheckpoint,
        (KeyCode::Right, KeyModifiers::SHIFT) => Action::JumpNextCheckpoint,
        (KeyCode::Left, _) => Action::ScrubLeft,
        (KeyCode::Right, _) => Action::ScrubRight,
        (KeyCode::Char('1'), _) => Action::SetSpeed(1),
        (KeyCode::Char('2'), _) => Action::SetSpeed(2),
        (KeyCode::Char('3'), _) => Action::SetSpeed(3),
        (KeyCode::Char('4'), _) => Action::SetSpeed(4),
        (KeyCode::Char('5'), _) => Action::SetSpeed(5),
        (KeyCode::Char('r'), KeyModifiers::NONE) => Action::Rewind,
        (KeyCode::Char('R'), _) => Action::RewindFile,
        (KeyCode::Char('u'), _) => Action::UndoRewind,
        (KeyCode::Char('x'), _) => Action::CutRange,
        (KeyCode::Char('c'), _) => Action::Checkpoint,
        (KeyCode::Char('s'), _) => Action::SoloTrack,
        (KeyCode::Char('m'), _) => Action::MuteTrack,
        (KeyCode::Char('g'), _) => Action::GroupByIntent,
        (KeyCode::Char('b'), _) => Action::ToggleBlastRadius,
        (KeyCode::Char('i'), _) => Action::ToggleSentinels,
        (KeyCode::Char('w'), _) => Action::ToggleWatchdog,
        (KeyCode::Tab, _) => Action::CycleFocus,
        (KeyCode::Char('/'), _) => Action::Search,
        (KeyCode::Char('?'), _) => Action::Help,
        _ => Action::None,
    }
}

/// Apply an `Action` to the `App` state.
///
/// Some actions (Checkpoint, Rewind, RewindFile, UndoRewind, CutRange, SoloTrack,
/// MuteTrack, GroupByIntent, Search, Help) require external coordination and are
/// intentional no-ops here; the caller handles them.
pub fn apply_action(app: &mut App, action: Action) {
    match action {
        Action::Quit => app.should_quit = true,
        Action::TogglePlay => app.toggle_play(),
        Action::ScrubLeft => app.scrub_left(),
        Action::ScrubRight => app.scrub_right(),

        Action::JumpPrevCheckpoint => {
            // Jump to the nearest checkpoint whose index is less than the current playhead.
            if let Some(&target) = app
                .checkpoint_ids
                .iter()
                .rev()
                .find(|&&id| (id as usize) < app.playhead)
            {
                app.playhead = target as usize;
                app.playback = PlaybackState::Paused;
            }
        }
        Action::JumpNextCheckpoint => {
            // Jump to the nearest checkpoint whose index is greater than the current playhead.
            if let Some(&target) = app
                .checkpoint_ids
                .iter()
                .find(|&&id| (id as usize) > app.playhead)
            {
                app.playhead = target as usize;
                app.playback = PlaybackState::Paused;
            }
        }

        Action::SetSpeed(s) => app.set_speed(s),

        // Sidebar panel toggles — pressing the same key again closes the sidebar.
        Action::ToggleBlastRadius => {
            toggle_sidebar(app, SidebarPanel::BlastRadius);
        }
        Action::ToggleSentinels => {
            toggle_sidebar(app, SidebarPanel::Sentinels);
        }
        Action::ToggleWatchdog => {
            toggle_sidebar(app, SidebarPanel::Watchdog);
        }

        Action::CycleFocus => {
            app.focused_pane = match &app.focused_pane {
                Pane::Preview => Pane::Timeline,
                Pane::Timeline => {
                    if app.sidebar_visible {
                        Pane::Sidebar
                    } else {
                        Pane::Preview
                    }
                }
                Pane::Sidebar => Pane::Preview,
            };
        }

        // These actions require external handling; nothing to do in the state machine.
        Action::Rewind
        | Action::RewindFile
        | Action::UndoRewind
        | Action::CutRange
        | Action::Checkpoint
        | Action::SoloTrack
        | Action::MuteTrack
        | Action::GroupByIntent
        | Action::Search
        | Action::Help
        | Action::None => {}
    }
}

/// Toggle a sidebar panel: if already showing this panel, close the sidebar.
/// Otherwise, open the sidebar and switch to this panel.
fn toggle_sidebar(app: &mut App, panel: SidebarPanel) {
    if app.sidebar_visible && app.sidebar_panel == panel {
        app.sidebar_visible = false;
    } else {
        app.sidebar_visible = true;
        app.sidebar_panel = panel;
    }
}
