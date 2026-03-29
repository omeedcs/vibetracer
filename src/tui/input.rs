use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::theme::Theme;
use crate::tui::app::{Mode, PreviewMode};
use crate::tui::filter::Filter;
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

    // ── mode transitions ─────────────────────────────────────────────────────
    EnterTimelineMode,
    EnterInspectMode,
    EnterSearchMode,
    ExitMode, // back to Normal
    OpenCommandPalette,
    ToggleDashboard,

    // ── search mode actions ──────────────────────────────────────────────────
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,

    // ── timeline mode actions ────────────────────────────────────────────────
    TimelinePanLeft,
    TimelinePanRight,
    TimelineSelectUp,
    TimelineSelectDown,
    TimelineJumpToEdit,

    // ── inspect mode actions ─────────────────────────────────────────────────
    InspectNext,
    InspectPrev,
    InspectToggleDiff,
    InspectShowFile,
    InspectShowConversation,
    InspectExpand,

    // ── blame / annotations ──────────────────────────────────────────────────
    ToggleBlame,
    ToggleAnnotations,

    // ── bookmarks ────────────────────────────────────────────────────────────
    CreateBookmark,
    JumpToBookmark,

    // ── conversation panel ──────────────────────────────────────────────────
    ToggleConversation,

    // ── panel maximize ───────────────────────────────────────────────────────
    MaximizePanel,

    Noop,
}

/// Map a crossterm `KeyEvent` to an `Action`, respecting the current mode.
pub fn map_key(key: KeyEvent, mode: &Mode) -> Action {
    match mode {
        Mode::Normal => map_key_normal(key),
        Mode::Timeline => map_key_timeline(key),
        Mode::Inspect => map_key_inspect(key),
        Mode::Search => map_key_search(key),
    }
}

/// Key mappings for Normal mode.
fn map_key_normal(key: KeyEvent) -> Action {
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

        // Mode transitions
        (KeyCode::Char('t'), KeyModifiers::NONE) => Action::EnterTimelineMode,
        (KeyCode::Char('i'), KeyModifiers::NONE) => Action::EnterInspectMode,
        (KeyCode::Char('/'), _) => Action::EnterSearchMode,
        (KeyCode::Char(':'), _) => Action::OpenCommandPalette,
        (KeyCode::Char('p'), m) if m.contains(KeyModifiers::CONTROL) => Action::OpenCommandPalette,

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
        (KeyCode::Char('w'), KeyModifiers::NONE) => Action::ToggleWatchdog,

        // Dashboard toggle
        (KeyCode::Char('D'), _) => Action::ToggleDashboard,

        // Preview mode toggle
        (KeyCode::Char('d'), KeyModifiers::NONE) => Action::TogglePreviewMode,

        // Blame and annotations
        (KeyCode::Char('B'), _) => Action::ToggleBlame,
        (KeyCode::Char('A'), _) => Action::ToggleAnnotations,

        // Bookmarks
        (KeyCode::Char('M'), _) => Action::CreateBookmark,
        (KeyCode::Char('\''), _) => Action::JumpToBookmark,

        // Conversation panel toggle
        (KeyCode::Char('C'), _) => Action::ToggleConversation,

        // Panel maximize
        (KeyCode::Char('z'), KeyModifiers::NONE) => Action::MaximizePanel,

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

        // Solo agent (1-9) — only in command view
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) => Action::SoloAgent(c as u8 - b'0'),

        _ => Action::Noop,
    }
}

/// Key mappings for Timeline mode.
fn map_key_timeline(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::ExitMode,
        (KeyCode::Left, _) => Action::TimelinePanLeft,
        (KeyCode::Right, _) => Action::TimelinePanRight,
        (KeyCode::Up, _) => Action::TimelineSelectUp,
        (KeyCode::Down, _) => Action::TimelineSelectDown,
        (KeyCode::Char('+'), _) => Action::ZoomTimelineIn,
        (KeyCode::Char('-'), _) => Action::ZoomTimelineOut,
        (KeyCode::Char('='), _) => Action::ZoomTimelineReset,
        (KeyCode::Char('s'), _) => Action::SoloTrack,
        (KeyCode::Char('m'), _) => Action::MuteTrack,
        (KeyCode::Enter, _) => Action::TimelineJumpToEdit,
        (KeyCode::Char('q'), _) => Action::Quit,
        _ => Action::Noop,
    }
}

/// Key mappings for Inspect mode.
fn map_key_inspect(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::ExitMode,
        (KeyCode::Char('n'), _) => Action::InspectNext,
        (KeyCode::Char('p'), KeyModifiers::NONE) => Action::InspectPrev,
        (KeyCode::Char('d'), _) => Action::InspectToggleDiff,
        (KeyCode::Char('f'), _) => Action::InspectShowFile,
        (KeyCode::Char('c'), _) => Action::InspectShowConversation,
        (KeyCode::Enter, _) => Action::InspectExpand,
        (KeyCode::Char('q'), _) => Action::Quit,
        _ => Action::Noop,
    }
}

/// Key mappings for Search mode.
fn map_key_search(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::ExitMode,
        (KeyCode::Enter, _) => Action::SearchConfirm,
        (KeyCode::Backspace, _) => Action::SearchBackspace,
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => Action::SearchInput(c),
        (KeyCode::Up, _) => Action::ScrollPreviewUp,
        (KeyCode::Down, _) => Action::ScrollPreviewDown,
        _ => Action::Noop,
    }
}

/// Apply an `Action` to the `App` state.
///
/// Some actions (Checkpoint, Restore, UndoRestore, Help, OpenCommandPalette)
/// require external coordination and are intentional no-ops here; the caller
/// handles them.
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

        // ── mode transitions ─────────────────────────────────────────────────
        Action::EnterTimelineMode => {
            app.mode = Mode::Timeline;
            app.mode_cursor = 0;
            app.focused_pane = Pane::Timeline;
        }
        Action::EnterInspectMode => {
            app.mode = Mode::Inspect;
        }
        Action::EnterSearchMode => {
            app.mode = Mode::Search;
            app.search_input.clear();
        }
        Action::ExitMode => {
            // If exiting search mode, clear the filter
            if app.mode == Mode::Search {
                app.search_input.clear();
                app.active_filter = None;
                app.filter_matches.clear();
            }
            app.mode = Mode::Normal;
        }
        Action::ToggleDashboard => {
            app.dashboard_visible = !app.dashboard_visible;
            if app.dashboard_visible {
                app.conversation_visible = false;
            }
        }
        Action::ToggleConversation => {
            app.conversation_visible = !app.conversation_visible;
            if app.conversation_visible {
                app.dashboard_visible = false;
            }
        }

        // ── search mode actions ──────────────────────────────────────────────
        Action::SearchInput(c) => {
            app.search_input.push(c);
        }
        Action::SearchBackspace => {
            app.search_input.pop();
        }
        Action::SearchConfirm => {
            // Lock filter and return to Normal mode
            if !app.search_input.is_empty() {
                let session_start_ms = app.session_start * 1000;
                let filter = Filter::parse(&app.search_input, session_start_ms);
                let matches = crate::tui::filter::compute_matching_indices(&app.edits, &filter);
                app.filter_matches = matches;
                app.active_filter = Some(filter);
            }
            app.mode = Mode::Normal;
        }

        // ── timeline mode actions ────────────────────────────────────────────
        Action::TimelinePanLeft => {
            app.timeline_scroll = app.timeline_scroll.saturating_sub(5);
        }
        Action::TimelinePanRight => {
            app.timeline_scroll += 5;
        }
        Action::TimelineSelectUp => {
            app.mode_cursor = app.mode_cursor.saturating_sub(1);
        }
        Action::TimelineSelectDown => {
            if !app.tracks.is_empty() {
                app.mode_cursor = (app.mode_cursor + 1).min(app.tracks.len() - 1);
            }
        }
        Action::TimelineJumpToEdit => {
            // Jump the playhead to the first edit in the selected track
            if let Some(track) = app.tracks.get(app.mode_cursor) {
                if let Some(&first_idx) = track.edit_indices.first() {
                    app.playhead = first_idx;
                    app.playback = crate::tui::PlaybackState::Paused;
                }
            }
        }

        // ── inspect mode actions ─────────────────────────────────────────────
        Action::InspectNext => {
            if app.playhead + 1 < app.edits.len() {
                app.playhead += 1;
                app.playback = crate::tui::PlaybackState::Paused;
            }
        }
        Action::InspectPrev => {
            if app.playhead > 0 {
                app.playhead -= 1;
                app.playback = crate::tui::PlaybackState::Paused;
            }
        }
        Action::InspectToggleDiff => {
            app.preview_mode = match app.preview_mode {
                PreviewMode::File => PreviewMode::Diff,
                PreviewMode::Diff => PreviewMode::File,
            };
        }
        Action::InspectShowFile => {
            app.preview_mode = PreviewMode::File;
        }
        Action::InspectShowConversation | Action::InspectExpand => {
            // Placeholder for Wave 2 conversation integration
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
            if app.preview_scroll > 0 {
                app.preview_scroll -= 1;
                app.preview_scroll_target = app.preview_scroll;
            }
        }
        Action::ScrollPreviewDown => {
            app.preview_scroll += 1;
            app.preview_scroll_target = app.preview_scroll;
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
                    if app.sidebar_visible || app.dashboard_visible {
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

        // Bookmarks
        Action::CreateBookmark => {
            let edit_index = app.playhead;
            let label = format!("edit #{}", edit_index);
            app.bookmark_manager.add(label.clone(), edit_index);
            app.show_toast(
                format!("bookmark: {}", label),
                crate::tui::app::ToastStyle::Success,
            );
        }
        Action::JumpToBookmark => {
            app.bookmark_popup_visible = true;
            app.bookmark_popup_selected = 0;
        }

        Action::ToggleBlame => {
            app.blame_visible = !app.blame_visible;
            if app.blame_visible {
                app.annotations_visible = false; // mutually exclusive
            }
            let msg = if app.blame_visible { "blame: on" } else { "blame: off" };
            app.show_toast(msg.to_string(), crate::tui::app::ToastStyle::Info);
        }
        Action::ToggleAnnotations => {
            app.annotations_visible = !app.annotations_visible;
            if app.annotations_visible {
                app.blame_visible = false; // mutually exclusive
            }
            let msg = if app.annotations_visible { "annotations: on" } else { "annotations: off" };
            app.show_toast(msg.to_string(), crate::tui::app::ToastStyle::Info);
        }
        Action::MaximizePanel => {
            app.show_toast("coming soon".to_string(), crate::tui::app::ToastStyle::Info);
        }

        // These actions require external handling; nothing to do in the state machine.
        Action::Restore
        | Action::UndoRestore
        | Action::Checkpoint
        | Action::Help
        | Action::OpenCommandPalette
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
