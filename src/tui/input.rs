use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::theme::Theme;
use crate::tui::app::PreviewMode;
use crate::tui::{App, Pane, SidebarPanel};

/// All actions that a keypress can trigger.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    QuitAndStopDaemon,
    Help,
    TogglePlay,
    ScrubLeft,
    ScrubRight,
    FileScrubLeft,
    FileScrubRight,
    Reattach,
    ToggleCommandView,
    Restore,
    UndoRestore,
    Checkpoint,
    ToggleRestoreEdits,
    SoloTrack,
    MuteTrack,
    ToggleBlastRadius,
    ToggleSentinels,
    ToggleWatchdog,
    CycleTheme,
    CycleFocus,
    TogglePreviewMode,
    ScrollPreviewUp,
    ScrollPreviewDown,
    ZoomTimelineIn,
    ZoomTimelineOut,
    ZoomTimelineReset,
    SoloAgent(u8),
    Noop,
}

/// Map a crossterm `KeyEvent` to an `Action`.
pub fn map_key(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('q'), KeyModifiers::NONE) => Action::Quit,
        (KeyCode::Char('Q'), _) => Action::QuitAndStopDaemon,

        // Help
        (KeyCode::Char('?'), _) => Action::Help,

        // Playback
        (KeyCode::Char(' '), _) => Action::TogglePlay,

        // Global scrub
        (KeyCode::Left, m) if !m.contains(KeyModifiers::SHIFT) => Action::ScrubLeft,
        (KeyCode::Right, m) if !m.contains(KeyModifiers::SHIFT) => Action::ScrubRight,

        // Per-file scrub
        (KeyCode::Left, m) if m.contains(KeyModifiers::SHIFT) => Action::FileScrubLeft,
        (KeyCode::Right, m) if m.contains(KeyModifiers::SHIFT) => Action::FileScrubRight,

        // Reattach detached file to global playhead
        (KeyCode::Char('a'), KeyModifiers::NONE) => Action::Reattach,

        // Toggle command/operation view
        (KeyCode::Char('g'), KeyModifiers::NONE) => Action::ToggleCommandView,

        // Restore (Shift+R)
        (KeyCode::Char('R'), _) => Action::Restore,

        // Undo restore
        (KeyCode::Char('u'), KeyModifiers::NONE) => Action::UndoRestore,

        // Checkpoint
        (KeyCode::Char('c'), KeyModifiers::NONE) => Action::Checkpoint,

        // Toggle showing restore-generated edits
        (KeyCode::Char('x'), KeyModifiers::NONE) => Action::ToggleRestoreEdits,

        // Track solo/mute
        (KeyCode::Char('s'), KeyModifiers::NONE) => Action::SoloTrack,
        (KeyCode::Char('m'), KeyModifiers::NONE) => Action::MuteTrack,

        // Sidebar panel toggles
        (KeyCode::Char('b'), KeyModifiers::NONE) => Action::ToggleBlastRadius,
        (KeyCode::Char('i'), KeyModifiers::NONE) => Action::ToggleSentinels,
        (KeyCode::Char('w'), KeyModifiers::NONE) => Action::ToggleWatchdog,

        // Theme cycling
        (KeyCode::Char('t'), KeyModifiers::NONE) => Action::CycleTheme,

        // Preview mode toggle
        (KeyCode::Char('d'), KeyModifiers::NONE) => Action::TogglePreviewMode,

        // Preview scroll (when preview is focused)
        (KeyCode::Char('j'), KeyModifiers::NONE) => Action::ScrollPreviewDown,
        (KeyCode::Char('k'), KeyModifiers::NONE) => Action::ScrollPreviewUp,

        // Timeline zoom
        (KeyCode::Char('+'), _) => Action::ZoomTimelineIn,
        (KeyCode::Char('='), KeyModifiers::NONE) => Action::ZoomTimelineIn,
        (KeyCode::Char('-'), KeyModifiers::NONE) => Action::ZoomTimelineOut,
        (KeyCode::Char('0'), KeyModifiers::NONE) => Action::ZoomTimelineReset,

        // Focus cycling
        (KeyCode::Tab, _) => Action::CycleFocus,

        // Solo agent (1-9)
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) => Action::SoloAgent(c as u8 - b'0'),

        _ => Action::Noop,
    }
}

/// Apply an `Action` to the `App` state.
///
/// Some actions (Checkpoint, Restore, UndoRestore, Help) require external
/// coordination and are intentional no-ops here; the caller handles them.
pub fn apply_action(app: &mut App, action: Action) {
    match action {
        Action::Quit | Action::QuitAndStopDaemon => app.should_quit = true,
        Action::TogglePlay => {
            app.toggle_play();
            app.playback_flash = Some(std::time::Instant::now());
        }
        Action::ScrubLeft => {
            let prev_file = app.current_edit().map(|e| e.file.clone());
            app.scrub_left();
            let new_file = app.current_edit().map(|e| e.file.clone());
            if prev_file != new_file {
                if let Some(f) = new_file {
                    app.track_flash = Some((f, std::time::Instant::now()));
                }
            }
        }
        Action::ScrubRight => {
            let prev_file = app.current_edit().map(|e| e.file.clone());
            app.scrub_right();
            let new_file = app.current_edit().map(|e| e.file.clone());
            if prev_file != new_file {
                if let Some(f) = new_file {
                    app.track_flash = Some((f, std::time::Instant::now()));
                }
            }
        }

        // Per-file scrub: detach the current file's track and scrub it.
        Action::FileScrubLeft => {
            if let Some(edit) = app.current_edit() {
                let file = edit.file.clone();
                if !app.detached_files.contains(&file) {
                    // Auto-detach at the current per-file position
                    let pos = app.file_playheads.get(&file).copied().unwrap_or(0);
                    app.detached_files.insert(file.clone());
                    app.file_playheads.insert(file.clone(), pos);
                }
                let pos = app.file_playheads.get(&file).copied().unwrap_or(0);
                if pos > 0 {
                    app.file_playheads.insert(file, pos - 1);
                }
            }
        }
        Action::FileScrubRight => {
            if let Some(edit) = app.current_edit() {
                let file = edit.file.clone();
                if !app.detached_files.contains(&file) {
                    let pos = app.file_playheads.get(&file).copied().unwrap_or(0);
                    app.detached_files.insert(file.clone());
                    app.file_playheads.insert(file.clone(), pos);
                }
                let pos = app.file_playheads.get(&file).copied().unwrap_or(0);
                app.file_playheads.insert(file, pos + 1);
            }
        }

        // Reattach: snap the current file back to the global playhead.
        Action::Reattach => {
            if let Some(edit) = app.current_edit() {
                let file = edit.file.clone();
                app.detached_files.remove(&file);
                app.file_playheads.remove(&file);
            }
        }

        // Toggle command/operation view
        Action::ToggleCommandView => {
            app.command_view = !app.command_view;
        }

        // Toggle preview mode (file view vs diff view)
        Action::TogglePreviewMode => {
            app.preview_mode = match app.preview_mode {
                PreviewMode::File => PreviewMode::Diff,
                PreviewMode::Diff => PreviewMode::File,
            };
        }

        // Preview scroll
        Action::ScrollPreviewUp => {
            if app.focused_pane == Pane::Preview && app.preview_scroll > 0 {
                app.preview_scroll -= 1;
                app.preview_scroll_target = app.preview_scroll;
            }
        }
        Action::ScrollPreviewDown => {
            if app.focused_pane == Pane::Preview {
                app.preview_scroll += 1;
                app.preview_scroll_target = app.preview_scroll;
            }
        }

        // Timeline zoom
        Action::ZoomTimelineIn => {
            app.timeline_zoom = (app.timeline_zoom * 1.5).min(20.0);
        }
        Action::ZoomTimelineOut => {
            app.timeline_zoom = (app.timeline_zoom / 1.5).max(1.0);
        }
        Action::ZoomTimelineReset => {
            app.timeline_zoom = 1.0;
            app.timeline_scroll = 0;
        }

        // Toggle showing restore-generated edits
        Action::ToggleRestoreEdits => {
            app.show_restore_edits = !app.show_restore_edits;
            let msg = if app.show_restore_edits {
                "restore edits: visible"
            } else {
                "restore edits: hidden"
            };
            app.show_toast(msg.to_string(), crate::tui::app::ToastStyle::Info);
        }

        // Cycle through theme presets
        Action::CycleTheme => {
            let presets = Theme::preset_names();
            let current_idx = presets
                .iter()
                .position(|&n| n == app.theme_name)
                .unwrap_or(0);
            let next_idx = (current_idx + 1) % presets.len();
            let next_name = presets[next_idx];
            app.theme = Theme::from_preset(next_name);
            app.theme_name = next_name.to_string();
            app.theme_flash = Some(std::time::Instant::now());
        }

        // Sidebar panel toggles -- pressing the same key again closes the sidebar.
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

        // Solo track: toggle solo on the file of the current edit.
        Action::SoloTrack => {
            if let Some(edit) = app.current_edit() {
                let file = edit.file.clone();
                if app.solo_track.as_ref() == Some(&file) {
                    app.solo_track = None;
                } else {
                    app.solo_track = Some(file);
                }
            }
        }

        // Mute track: toggle mute on the file of the current edit.
        Action::MuteTrack => {
            if let Some(edit) = app.current_edit() {
                let file = edit.file.clone();
                if let Some(pos) = app.muted_tracks.iter().position(|f| f == &file) {
                    app.muted_tracks.remove(pos);
                } else {
                    app.muted_tracks.push(file);
                }
            }
        }

        // Solo agent: filter timeline to only show edits from agent N.
        // (Stored as solo_track with a special prefix so the timeline can distinguish)
        Action::SoloAgent(n) => {
            let mut seen = std::collections::HashSet::new();
            let mut agents: Vec<String> = Vec::new();
            for edit in &app.edits {
                if let Some(ref agent_id) = edit.agent_id {
                    if seen.insert(agent_id.clone()) {
                        agents.push(agent_id.clone());
                    }
                }
            }

            let idx = (n as usize).saturating_sub(1);
            if let Some(agent_id) = agents.get(idx) {
                if app.solo_agent.as_ref() == Some(agent_id) {
                    app.solo_agent = None;
                } else {
                    app.solo_agent = Some(agent_id.clone());
                }
            }
        }

        // These actions require external handling; nothing to do in the state machine.
        Action::Restore
        | Action::UndoRestore
        | Action::Checkpoint
        | Action::Help
        | Action::Noop => {}
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
