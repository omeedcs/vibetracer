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
    ToggleEquationLens,
    ToggleBlastRadius,
    ToggleSentinels,
    ToggleSchemaMode,
    ToggleRefactorTracker,
    ToggleWatchdog,
    CycleFocus,
    ToggleTerminalFocus,
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
        (KeyCode::Char('e'), _) => Action::ToggleEquationLens,
        (KeyCode::Char('b'), _) => Action::ToggleBlastRadius,
        (KeyCode::Char('i'), _) => Action::ToggleSentinels,
        (KeyCode::Char('d'), _) => Action::ToggleSchemaMode,
        (KeyCode::Char('f'), _) => Action::ToggleRefactorTracker,
        (KeyCode::Char('w'), _) => Action::ToggleWatchdog,
        (KeyCode::Tab, _) => Action::CycleFocus,
        // Ctrl+\ toggles between terminal focus and vibetracer panes
        (KeyCode::Char('\\'), KeyModifiers::CONTROL) => Action::ToggleTerminalFocus,
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

        // Sidebar panel toggles — each opens the sidebar and switches to the relevant panel.
        Action::ToggleEquationLens => {
            app.equation_lens = !app.equation_lens;
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Equations;
        }
        Action::ToggleBlastRadius => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::BlastRadius;
        }
        Action::ToggleSentinels => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Sentinels;
        }
        Action::ToggleSchemaMode => {
            app.schema_diff_mode = !app.schema_diff_mode;
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Refactor;
        }
        Action::ToggleRefactorTracker => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Refactor;
        }
        Action::ToggleWatchdog => {
            app.sidebar_visible = true;
            app.sidebar_panel = SidebarPanel::Watchdog;
        }

        Action::CycleFocus => {
            app.focused_pane = match &app.focused_pane {
                Pane::Preview => Pane::Timeline,
                Pane::Timeline => {
                    if app.terminal_visible {
                        Pane::TerminalPane
                    } else if app.sidebar_visible {
                        Pane::Sidebar
                    } else {
                        Pane::Preview
                    }
                }
                Pane::TerminalPane => {
                    if app.sidebar_visible {
                        Pane::Sidebar
                    } else {
                        Pane::Preview
                    }
                }
                Pane::Sidebar => Pane::Preview,
            };
        }

        Action::ToggleTerminalFocus => {
            if app.focused_pane == Pane::TerminalPane {
                // Return to the last vibetracer pane
                app.focused_pane = app.last_vibetracer_pane.clone().unwrap_or(Pane::Preview);
            } else if app.terminal_visible {
                app.last_vibetracer_pane = Some(app.focused_pane.clone());
                app.focused_pane = Pane::TerminalPane;
            }
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
