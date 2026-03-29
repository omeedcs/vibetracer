use crate::analysis::blast_radius::BlastRadiusTracker;
use crate::analysis::sentinels::SentinelEngine;
use crate::analysis::watchdog::Watchdog;
use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::event::EditEvent;
use crate::recorder::Recorder;
use crate::tui::{App, SidebarPanel, input, layout, widgets};
use crate::tui::app::Mode;
use anyhow::Result;
use crossterm::event::{self as ct_event, Event, KeyEventKind};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Render a solid background block over the entire terminal area using the given color.
struct BgFill(Color);
impl Widget for BgFill {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(self.0);
                }
            }
        }
    }
}

/// Render a horizontal separator line filling the given area with `─`.
struct HorizontalSep {
    color: Color,
    focused: bool,
    focus_color: Color,
}
impl Widget for HorizontalSep {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let color = if self.focused { self.focus_color } else { self.color };
        let line = "─".repeat(area.width as usize);
        buf.set_string(area.x, area.y, &line, Style::default().fg(color));
    }
}

/// Run the main TUI event loop.
///
/// This function owns the render cycle, input handling, file-change processing,
/// and analysis engine invocation. The caller sets up the terminal, session,
/// recorder, watcher, and config, then hands everything in here.
///
/// Two source modes are supported:
///
/// - **No-daemon mode** (`fs_rx` + `recorder`): The TUI watches for file
///   changes itself and uses a `Recorder` to produce edit events.
/// - **Daemon mode** (`edit_rx`): The TUI tails the daemon's edit log for
///   pre-built `EditEvent`s. No watcher or recorder is needed.
///
/// In replay mode both receivers are `None` and no new edits arrive.
#[allow(clippy::too_many_arguments)]
pub fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut recorder: Option<&mut Recorder>,
    checkpoint_manager: &CheckpointManager,
    fs_rx: Option<&mpsc::Receiver<PathBuf>>,
    edit_rx: Option<&mpsc::Receiver<EditEvent>>,
    claude_log_rx: Option<&mpsc::Receiver<crate::claude_log::ConversationTurn>>,
    config: &Config,
    project_path: &Path,
    session_dir: &Path,
    daemon_running: bool,
) -> Result<()> {
    // Syntax highlighter for file view mode.
    let highlighter = crate::tui::syntax::Highlighter::new();

    // Track playhead to detect changes and auto-scroll to changed lines.
    let mut last_playhead: usize = app.playhead;

    let mut last_play_advance = std::time::Instant::now();

    // Edit count since last checkpoint (for auto-checkpoint).
    let mut edits_since_checkpoint: u32 = 0;

    // Whether the help overlay is visible.
    let mut show_help = false;

    // Channel used by Recorder to emit EditEvents (not currently consumed
    // externally, but required by the Recorder API for daemon reuse).
    let (edit_event_tx, _edit_event_rx) = mpsc::channel::<EditEvent>();

    // Analysis engines.
    let watchdog = Watchdog::new(config.watchdog.constants.clone());
    let sentinel_engine = SentinelEngine::new(project_path.to_path_buf());
    let blast_tracker = BlastRadiusTracker::new(config.blast_radius.clone());
    // Track which files have been edited this session (for blast radius staleness).
    let mut edited_files: std::collections::HashSet<String> = std::collections::HashSet::new();

    // ── main event loop ───────────────────────────────────────────────────────
    loop {
        // ── update dashboard state ────────────────────────────────────────────
        app.update_dashboard();

        // ── evaluate alert conditions ────────────────────────────────────────
        {
            use crate::tui::alerts::{AlertAction, AlertState};

            let alert_state = AlertState {
                session_cost: app.dashboard_state.total_cost,
                sentinel_failures: app.dashboard_state.sentinel_fail,
                stale_count: app.dashboard_state.stale_count,
                edit_velocity: app.dashboard_state.edit_velocity,
                edit_count: app.edits.len() as u64,
            };
            let fired = app.alert_evaluator.evaluate(&alert_state);
            for alert in fired {
                match alert.action {
                    AlertAction::Toast => {
                        app.show_toast(alert.message, crate::tui::app::ToastStyle::Warning);
                    }
                    AlertAction::Flash => {
                        app.screen_flash = Some(std::time::Instant::now());
                    }
                    AlertAction::Bell => {
                        print!("\x07");
                    }
                }
            }
        }

        // ── render ────────────────────────────────────────────────────────────
        let file_content_data: Option<(String, String)> = app.current_file_content(session_dir);
        let changed_lines = app.changed_lines_from_patch();

        terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();

            // Background fill.
            BgFill(app.theme.bg).render(area, buf);

            // Skip rendering if terminal is too small.
            if area.width < 20 || area.height < 8 {
                let msg = "terminal too small";
                let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
                let y = area.y + area.height / 2;
                buf.set_string(x, y, msg, Style::default().fg(app.theme.accent_red));
                return;
            }

            let lo = layout::compute_layout(area, app.sidebar_visible, app.dashboard_visible, app.conversation_visible);

            // Store layout for mouse routing.
            app.last_layout = Some(lo.clone());

            // Determine which pane is focused for border highlighting.
            let focus_color = app.theme.accent_warm;
            let sep_color = app.theme.separator;

            // Render horizontal separators between zones.
            HorizontalSep {
                color: sep_color,
                focused: app.focused_pane == crate::tui::Pane::Preview,
                focus_color,
            }
            .render(lo.sep_after_status, buf);

            HorizontalSep {
                color: sep_color,
                focused: app.focused_pane == crate::tui::Pane::Timeline,
                focus_color,
            }
            .render(lo.sep_after_main, buf);

            HorizontalSep {
                color: sep_color,
                focused: false,
                focus_color,
            }
            .render(lo.sep_after_timeline, buf);

            // Status bar.
            widgets::status_bar::StatusBar::new(app).render(lo.status_bar, buf);

            // Sidebar panels (if visible).
            if let Some(sidebar_rect) = lo.sidebar {
                match app.sidebar_panel {
                    SidebarPanel::BlastRadius => {
                        if let Some((ref source, ref status)) = app.blast_radius_status {
                            widgets::blast_radius_panel::BlastRadiusPanel::new(source, status, &app.theme)
                                .render(sidebar_rect, buf);
                        } else {
                            let msg = "no blast radius data";
                            buf.set_string(
                                sidebar_rect.x + 1,
                                sidebar_rect.y + 1,
                                msg,
                                Style::default().fg(app.theme.fg_dim),
                            );
                        }
                    }
                    SidebarPanel::Sentinels => {
                        widgets::sentinel_panel::SentinelPanel::new(&app.sentinel_violations, &app.theme)
                            .render(sidebar_rect, buf);
                    }
                    SidebarPanel::Watchdog => {
                        widgets::watchdog_panel::WatchdogPanel::new(&app.watchdog_alerts, &app.theme)
                            .render(sidebar_rect, buf);
                    }
                }
            }

            // Vertical separator between preview and right panel (sidebar or dashboard).
            let right_panel_rect = lo.dashboard.or(lo.sidebar);
            if let Some(right_rect) = right_panel_rect {
                let sep_x = right_rect.x.saturating_sub(1);
                let focused = app.focused_pane == crate::tui::Pane::Sidebar;
                let color = if focused { focus_color } else { sep_color };
                for y in lo.main_area.y..lo.main_area.y + lo.main_area.height {
                    if sep_x >= lo.main_area.x && sep_x < lo.main_area.x + lo.main_area.width {
                        buf.set_string(sep_x, y, "\u{2502}", Style::default().fg(color));
                    }
                }
            }

            // Dashboard or conversation panel (right side, replaces sidebar when visible).
            if let Some(dash_rect) = lo.dashboard {
                if app.conversation_visible {
                    widgets::conversation::ConversationPanel::new(
                        &app.conversation_turns,
                        app.conversation_state.scroll,
                        &app.theme,
                        app.conversation_state.selected_turn,
                    )
                    .render(dash_rect, buf);
                } else {
                    widgets::dashboard::DashboardPanel::new(&app.dashboard_state, &app.theme)
                        .render(dash_rect, buf);
                }
            }

            // Preview pane.
            let content_ref = file_content_data
                .as_ref()
                .map(|(c, f)| (c.as_str(), f.as_str()));
            widgets::preview::PreviewPane::new(
                app,
                content_ref,
                Some(&highlighter),
                &changed_lines,
            )
            .render(lo.preview, buf);

            // Timeline.
            widgets::timeline::TimelineWidget::new(app).render(lo.timeline, buf);

            // Keybindings bar — mode-aware.
            let kb_sep = Span::styled(" \u{2502} ", Style::default().fg(app.theme.separator));
            let kb_key = |k: &str| Span::styled(k.to_string(), Style::default().fg(app.theme.fg));
            let kb_desc = |d: &str| Span::styled(d.to_string(), Style::default().fg(app.theme.fg_muted));

            let mode_color = match app.mode {
                Mode::Normal => app.theme.accent_green,
                Mode::Timeline => app.theme.accent_blue,
                Mode::Inspect => app.theme.accent_warm,
                Mode::Search => app.theme.accent_purple,
            };

            let mut kb_spans: Vec<Span> = vec![
                Span::styled(
                    format!(" {} ", app.mode.label()),
                    Style::default().fg(app.theme.bg).bg(mode_color),
                ),
                Span::styled(" ", Style::default()),
            ];

            match app.mode {
                Mode::Normal => {
                    kb_spans.extend_from_slice(&[
                        kb_key("Space"), kb_desc(":play"), kb_sep.clone(),
                        kb_key("\u{2190}\u{2192}"), kb_desc(":scrub"), kb_sep.clone(),
                        kb_key("t"), kb_desc(":timeline"), kb_sep.clone(),
                        kb_key("i"), kb_desc(":inspect"), kb_sep.clone(),
                        kb_key("/"), kb_desc(":search"), kb_sep.clone(),
                        kb_key(":"), kb_desc(":cmd"), kb_sep.clone(),
                        kb_key("d"), kb_desc(":diff"), kb_sep.clone(),
                        kb_key("?"), kb_desc(":help"),
                    ]);
                }
                Mode::Timeline => {
                    kb_spans.extend_from_slice(&[
                        kb_key("\u{2190}\u{2192}"), kb_desc(":pan"), kb_sep.clone(),
                        kb_key("\u{2191}\u{2193}"), kb_desc(":select"), kb_sep.clone(),
                        kb_key("+/-"), kb_desc(":zoom"), kb_sep.clone(),
                        kb_key("s"), kb_desc(":solo"), kb_sep.clone(),
                        kb_key("m"), kb_desc(":mute"), kb_sep.clone(),
                        kb_key("Enter"), kb_desc(":jump"), kb_sep.clone(),
                        kb_key("Esc"), kb_desc(":back"),
                    ]);
                }
                Mode::Inspect => {
                    kb_spans.extend_from_slice(&[
                        kb_key("n"), kb_desc(":next"), kb_sep.clone(),
                        kb_key("p"), kb_desc(":prev"), kb_sep.clone(),
                        kb_key("d"), kb_desc(":diff"), kb_sep.clone(),
                        kb_key("f"), kb_desc(":file"), kb_sep.clone(),
                        kb_key("Esc"), kb_desc(":back"),
                    ]);
                }
                Mode::Search => {
                    kb_spans.extend_from_slice(&[
                        kb_desc("type to filter"), kb_sep.clone(),
                        kb_key("Enter"), kb_desc(":lock"), kb_sep.clone(),
                        kb_key("Esc"), kb_desc(":clear"),
                    ]);
                }
            }

            let kb_line = Line::from(kb_spans);
            kb_line.render(lo.keybindings, buf);

            // Session diff overlay (on top of main content, below help/palette).
            if let Some(ref diff) = app.session_diff {
                widgets::session_diff_view::SessionDiffView::new(
                    diff,
                    &app.theme,
                    app.session_diff_selected,
                )
                .render(area, buf);
            }

            // Help overlay (on top of everything).
            if show_help {
                widgets::help_overlay::HelpOverlay::new(&app.theme).render(area, buf);
            }

            // Command palette overlay (on top of everything).
            if app.command_palette.visible {
                widgets::command_palette::CommandPaletteWidget {
                    palette: &app.command_palette,
                    theme_bg: app.theme.bg,
                    theme_fg: app.theme.fg,
                    theme_fg_dim: app.theme.fg_muted,
                    theme_accent: app.theme.accent_warm,
                    theme_separator: app.theme.separator,
                }.render(area, buf);
            }

            // Bookmark list popup overlay (on top of everything).
            if app.bookmark_popup_visible {
                let sorted = app.bookmark_manager.sorted();
                let sorted_owned: Vec<crate::tui::bookmarks::Bookmark> =
                    sorted.into_iter().cloned().collect();
                widgets::bookmark_list::BookmarkListWidget::new(
                    &sorted_owned,
                    app.bookmark_popup_selected,
                    &app.theme,
                )
                .render(area, buf);
            }

            // Screen flash overlay (brief red-tinted overlay that fades after 200ms).
            if let Some(flash_time) = app.screen_flash {
                let elapsed_ms = flash_time.elapsed().as_millis();
                if elapsed_ms < 200 {
                    // Fade from ~40% opacity to 0 over 200ms.
                    let alpha = 1.0 - (elapsed_ms as f64 / 200.0);
                    let tint_r = (40.0 * alpha) as u8;
                    for y in area.y..area.y + area.height {
                        for x in area.x..area.x + area.width {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                let bg_color = cell.bg;
                                let (br, bg_g, bb) = match bg_color {
                                    Color::Rgb(r, g, b) => (r, g, b),
                                    _ => (0, 0, 0),
                                };
                                let new_r = br.saturating_add(tint_r);
                                cell.set_bg(Color::Rgb(new_r, bg_g, bb));
                            }
                        }
                    }
                }
            }
        })?;

        // Clear screen flash after 200ms.
        if let Some(flash_time) = app.screen_flash {
            if flash_time.elapsed().as_millis() >= 200 {
                app.screen_flash = None;
            }
        }

        // ── poll for crossterm events (adaptive timeout) ──────────────────────
        let poll_duration = match &app.playback {
            crate::tui::PlaybackState::Playing { .. } => Duration::from_millis(16), // ~60fps
            _ => Duration::from_millis(100),                                        // idle
        };
        if ct_event::poll(poll_duration)? {
            match ct_event::read()? {
                Event::Resize(_cols, _rows) => {
                    continue;
                }
                Event::Key(key) => {
                    // Ignore key-release events on platforms that emit them.
                    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {

                        // ── Command palette key routing ──────────────────────
                        if app.command_palette.visible {
                            use crossterm::event::KeyCode;
                            match key.code {
                                KeyCode::Esc => { app.command_palette.close(); }
                                KeyCode::Enter => {
                                    // Check if the raw input is a :diff command before
                                    // confirming via the filtered entry list.
                                    let raw = app.command_palette.input.clone();
                                    if raw.starts_with("diff ") {
                                        app.command_palette.close();
                                        parse_and_open_diff(app, &raw);
                                    } else if let Some(action_id) = app.command_palette.confirm() {
                                        // Dispatch palette action by ID
                                        dispatch_palette_action(app, &action_id);
                                    }
                                }
                                KeyCode::Up => { app.command_palette.select_up(); }
                                KeyCode::Down => { app.command_palette.select_down(); }
                                KeyCode::Backspace => { app.command_palette.pop_char(); }
                                KeyCode::Char(c) => { app.command_palette.push_char(c); }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Session diff overlay key routing ───────────────
                        if app.session_diff.is_some() {
                            use crossterm::event::KeyCode;
                            match key.code {
                                KeyCode::Esc => {
                                    app.session_diff = None;
                                    app.session_diff_selected = 0;
                                }
                                KeyCode::Up => {
                                    if app.session_diff_selected > 0 {
                                        app.session_diff_selected -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(ref diff) = app.session_diff {
                                        if !diff.file_changes.is_empty()
                                            && app.session_diff_selected + 1
                                                < diff.file_changes.len()
                                        {
                                            app.session_diff_selected += 1;
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Bookmark popup key routing ──────────────────────
                        if app.bookmark_popup_visible {
                            use crossterm::event::KeyCode;
                            match key.code {
                                KeyCode::Esc => {
                                    app.bookmark_popup_visible = false;
                                }
                                KeyCode::Up => {
                                    if app.bookmark_popup_selected > 0 {
                                        app.bookmark_popup_selected -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let sorted = app.bookmark_manager.sorted();
                                    if !sorted.is_empty()
                                        && app.bookmark_popup_selected + 1 < sorted.len()
                                    {
                                        app.bookmark_popup_selected += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    let sorted = app.bookmark_manager.sorted();
                                    if let Some(bm) = sorted.get(app.bookmark_popup_selected) {
                                        let target = bm.edit_index;
                                        if target < app.edits.len() {
                                            app.playhead = target;
                                            app.playback = crate::tui::PlaybackState::Paused;
                                            app.cached_content = None;
                                            app.show_toast(
                                                format!("jumped to #{}", target),
                                                crate::tui::app::ToastStyle::Info,
                                            );
                                        }
                                    }
                                    app.bookmark_popup_visible = false;
                                }
                                KeyCode::Char('d') => {
                                    // Delete the selected bookmark. The popup shows
                                    // sorted (descending) bookmarks, so we need to
                                    // find the original index in the manager.
                                    let sorted = app.bookmark_manager.sorted();
                                    if let Some(bm) = sorted.get(app.bookmark_popup_selected) {
                                        // Find the matching bookmark in the original vec
                                        // by edit_index and timestamp.
                                        let target_idx = bm.edit_index;
                                        let target_ts = bm.timestamp;
                                        if let Some(pos) = app
                                            .bookmark_manager
                                            .bookmarks
                                            .iter()
                                            .position(|b| {
                                                b.edit_index == target_idx
                                                    && b.timestamp == target_ts
                                            })
                                        {
                                            app.bookmark_manager.remove(pos);
                                        }
                                    }
                                    // Clamp selection.
                                    let new_len = app.bookmark_manager.bookmarks.len();
                                    if new_len == 0 {
                                        app.bookmark_popup_selected = 0;
                                    } else if app.bookmark_popup_selected >= new_len {
                                        app.bookmark_popup_selected = new_len - 1;
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        let action = input::map_key(key, &app.mode);

                        match action {
                            input::Action::Quit => break,

                            input::Action::QuitAndStopDaemon => {
                                if daemon_running {
                                    let _ = crate::daemon::stop_daemon(project_path);
                                }
                                break;
                            }

                            input::Action::Help => {
                                show_help = !show_help;
                            }

                            input::Action::OpenCommandPalette => {
                                app.command_palette.open();
                            }

                            input::Action::Checkpoint => {
                                if let Some(ref mut rec) = recorder {
                                    let id = checkpoint_manager
                                        .save(rec.current_file_hashes().clone())?;
                                    app.checkpoint_ids.push(id);
                                    app.show_toast(
                                        format!("checkpoint #{}", id),
                                        crate::tui::app::ToastStyle::Success,
                                    );
                                    edits_since_checkpoint = 0;
                                }
                            }

                            input::Action::Restore => {
                                if let Some(edit) = app.current_edit().cloned() {
                                    let store = crate::snapshot::store::SnapshotStore::new(
                                        session_dir.join("snapshots"),
                                    );
                                    let engine = crate::restore::RestoreEngine::new(
                                        project_path.to_path_buf(),
                                        store,
                                    );
                                    let current_hash =
                                        engine.current_hash(&edit.file).unwrap_or_default();
                                    if let Err(e) =
                                        engine.restore_file(&edit.file, &edit.after_hash)
                                    {
                                        tracing::warn!("restore failed: {}", e);
                                    } else {
                                        let mut restore_log =
                                            crate::restore::restore_log::RestoreLog::new(
                                                session_dir.join("restores.jsonl"),
                                            );
                                        let _ = restore_log.append(
                                            crate::event::RestoreScope::File {
                                                path: edit.file.clone(),
                                                target_edit_id: edit.id,
                                            },
                                            vec![crate::event::RestoreFileEntry {
                                                path: edit.file.clone(),
                                                from_hash: current_hash,
                                                to_hash: edit.after_hash.clone(),
                                            }],
                                        );
                                        app.show_toast(
                                            format!("restored {}", edit.file),
                                            crate::tui::app::ToastStyle::Success,
                                        );
                                    }
                                }
                            }

                            input::Action::UndoRestore => {
                                let restore_log = crate::restore::restore_log::RestoreLog::new(
                                    session_dir.join("restores.jsonl"),
                                );
                                if let Ok(events) = restore_log.last_n(1) {
                                    if let Some(last) = events.first() {
                                        let store = crate::snapshot::store::SnapshotStore::new(
                                            session_dir.join("snapshots"),
                                        );
                                        let engine = crate::restore::RestoreEngine::new(
                                            project_path.to_path_buf(),
                                            store,
                                        );
                                        for entry in &last.files_restored {
                                            if entry.from_hash.is_empty() {
                                                let _ = engine.delete_file(&entry.path);
                                            } else {
                                                let _ = engine
                                                    .restore_file(&entry.path, &entry.from_hash);
                                            }
                                        }
                                    }
                                }
                                app.show_toast(
                                    "restore undone".to_string(),
                                    crate::tui::app::ToastStyle::Info,
                                );
                            }

                            other => {
                                input::apply_action(app, other);
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    use crossterm::event::MouseEventKind;
                    match mouse.kind {
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                            let is_up = matches!(mouse.kind, MouseEventKind::ScrollUp);

                            let in_preview = app.last_layout.as_ref()
                                .map(|lo| {
                                    mouse.row >= lo.preview.y
                                        && mouse.row < lo.preview.y + lo.preview.height
                                        && mouse.column >= lo.preview.x
                                        && mouse.column < lo.preview.x + lo.preview.width
                                })
                                .unwrap_or(false);

                            let in_timeline = app.last_layout.as_ref()
                                .map(|lo| {
                                    mouse.row >= lo.timeline.y
                                        && mouse.row < lo.timeline.y + lo.timeline.height
                                })
                                .unwrap_or(false);

                            if in_preview {
                                if is_up && app.preview_scroll > 0 {
                                    app.preview_scroll = app.preview_scroll.saturating_sub(3);
                                    app.preview_scroll_target = app.preview_scroll;
                                } else if !is_up {
                                    app.preview_scroll += 3;
                                    app.preview_scroll_target = app.preview_scroll;
                                }
                            } else if in_timeline {
                                if is_up {
                                    app.timeline_zoom = (app.timeline_zoom * 1.2).min(20.0);
                                } else {
                                    app.timeline_zoom = (app.timeline_zoom / 1.2).max(1.0);
                                    if app.timeline_zoom <= 1.01 {
                                        app.timeline_zoom = 1.0;
                                        app.timeline_scroll = 0;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {} // Ignore focus events, etc.
            }
        }

        // ── playhead-change detection: auto-scroll to first changed line ─────
        if app.playhead != last_playhead {
            let changed = app.changed_lines_from_patch();
            if let Some(&first_changed) = changed.iter().min() {
                let visible = 20;
                app.preview_scroll_target = first_changed.saturating_sub(visible / 2);
                app.preview_scroll = app.preview_scroll_target;
            }
            app.cached_content = None;
            last_playhead = app.playhead;
        }

        // ── drain edit sources (non-blocking) ──────────────────────────────────

        // Mode A: Daemon mode -- drain pre-built EditEvents from the tailer.
        if let Some(rx) = edit_rx {
            loop {
                match rx.try_recv() {
                    Ok(event) => {
                        edited_files.insert(event.file.clone());
                        app.push_edit(event);
                        edits_since_checkpoint += 1;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }

        // Mode B: No-daemon mode -- drain file-change channel and record.
        if let (Some(rx), Some(ref mut recorder)) = (fs_rx, recorder.as_deref_mut()) {
            loop {
                match rx.try_recv() {
                    Ok(abs_path) => {
                        if let Ok(Some(result)) =
                            recorder.process_file_change(&abs_path, &edit_event_tx, None)
                        {
                            let rel_path = result.event.file.clone();
                            let old_content = &result.old_content;
                            let new_content = &result.new_content;

                            edited_files.insert(rel_path.clone());
                            app.push_edit(result.event);

                            // -- Run analysis engines on this edit --

                            // Watchdog: check if a registered constant was modified.
                            let alerts = watchdog.check(&rel_path, old_content, new_content);
                            if !alerts.is_empty() {
                                app.watchdog_alerts = alerts;
                                app.sidebar_visible = true;
                                app.sidebar_panel = SidebarPanel::Watchdog;
                            }

                            // Sentinels: evaluate all rules that watch this file.
                            let mut violations = Vec::new();
                            for (name, rule) in &config.sentinels {
                                let watches = glob::Pattern::new(&rule.watch)
                                    .map(|p| p.matches(&rel_path))
                                    .unwrap_or(false);
                                if watches {
                                    violations.extend(sentinel_engine.evaluate(name, rule));
                                }
                            }
                            if !violations.is_empty() {
                                app.sentinel_violations = violations;
                                app.sidebar_visible = true;
                                app.sidebar_panel = SidebarPanel::Sentinels;
                            }

                            // Blast radius: check dependents of this file.
                            let dependents = blast_tracker.get_dependents(&rel_path);
                            if !dependents.is_empty() {
                                let status =
                                    blast_tracker.check_staleness(&rel_path, &edited_files);
                                app.blast_radius_status = Some((rel_path.clone(), status));
                                app.sidebar_visible = true;
                                app.sidebar_panel = SidebarPanel::BlastRadius;
                            }

                            edits_since_checkpoint += 1;

                            // Auto-checkpoint.
                            if config.watch.auto_checkpoint_every > 0
                                && edits_since_checkpoint >= config.watch.auto_checkpoint_every
                            {
                                let id = checkpoint_manager
                                    .save(recorder.current_file_hashes().clone())?;
                                app.checkpoint_ids.push(id);
                                edits_since_checkpoint = 0;
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }

        // ── drain Claude Code conversation log updates ────────────────────────
        if let Some(rx) = claude_log_rx {
            loop {
                match rx.try_recv() {
                    Ok(turn) => {
                        app.conversation_turns.push(turn);
                        app.token_stats = crate::claude_log::compute_stats(&app.conversation_turns);
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }

        // Frame-rate-aware playback
        if let crate::tui::PlaybackState::Playing { speed } = &app.playback {
            let interval = Duration::from_millis(500 / (*speed as u64).max(1));
            if last_play_advance.elapsed() >= interval {
                app.scrub_right();
                last_play_advance = std::time::Instant::now();
            }
        }

        // Smooth scroll interpolation
        if app.preview_scroll != app.preview_scroll_target {
            let diff = app.preview_scroll_target as f64 - app.preview_scroll as f64;
            let step = (diff * 0.15).round() as isize;
            if step.unsigned_abs() < 1 {
                app.preview_scroll = app.preview_scroll_target;
            } else {
                app.preview_scroll = (app.preview_scroll as isize + step).max(0) as usize;
            }
        }

        // Respect should_quit from apply_action (e.g. 'q' key).
        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Register all default palette entries.
pub fn register_palette_entries(palette: &mut widgets::command_palette::CommandPalette) {
    use widgets::command_palette::PaletteEntry;

    let entries = [
        // Navigation
        ("play_pause", "Play / Pause", Some("Space"), "Playback"),
        ("scrub_left", "Scrub Left", Some("\u{2190}"), "Playback"),
        ("scrub_right", "Scrub Right", Some("\u{2192}"), "Playback"),
        // Modes
        ("mode_timeline", "Enter Timeline Mode", Some("t"), "Mode"),
        ("mode_inspect", "Enter Inspect Mode", Some("i"), "Mode"),
        ("mode_search", "Enter Search Mode", Some("/"), "Mode"),
        // View
        ("toggle_diff", "Toggle Diff / File View", Some("d"), "View"),
        ("toggle_commands", "Toggle Command View", Some("g"), "View"),
        ("toggle_dashboard", "Toggle Dashboard", Some("D"), "View"),
        ("toggle_conversation", "Toggle Conversation", Some("C"), "View"),
        ("toggle_blame", "Toggle Blame View", Some("B"), "View"),
        ("toggle_annotations", "Toggle Annotations", Some("A"), "View"),
        ("zoom_in", "Zoom Timeline In", Some("+"), "View"),
        ("zoom_out", "Zoom Timeline Out", Some("-"), "View"),
        ("zoom_reset", "Zoom Timeline Reset", Some("0"), "View"),
        // Analysis
        ("toggle_blast", "Toggle Blast Radius", Some("b"), "Analysis"),
        ("toggle_watchdog", "Toggle Watchdog", Some("w"), "Analysis"),
        ("session_diff", "Session Diff...", None, "Analysis"),
        // Tracks
        ("solo_track", "Solo Track", Some("s"), "Track"),
        ("mute_track", "Mute Track", Some("m"), "Track"),
        // Actions
        ("restore", "Restore File at Playhead", Some("R"), "Action"),
        ("undo_restore", "Undo Restore", Some("u"), "Action"),
        ("checkpoint", "Create Checkpoint", Some("c"), "Action"),
        ("bookmark", "Create Bookmark", Some("M"), "Action"),
        ("jump_bookmark", "Jump to Bookmark", Some("'"), "Action"),
        // Theme
        ("theme_next", "Next Theme", None, "Theme"),
        ("theme_dark", "Theme: Dark", None, "Theme"),
        ("theme_catppuccin", "Theme: Catppuccin Mocha", None, "Theme"),
        ("theme_gruvbox", "Theme: Gruvbox Dark", None, "Theme"),
        ("theme_tokyo", "Theme: Tokyo Night", None, "Theme"),
        ("theme_dracula", "Theme: Dracula", None, "Theme"),
        ("theme_nord", "Theme: Nord", None, "Theme"),
        ("theme_rose_pine", "Theme: Rose Pine", None, "Theme"),
        // Session
        ("quit", "Quit", Some("q"), "Session"),
        ("quit_daemon", "Quit + Stop Daemon", Some("Q"), "Session"),
    ];

    for (id, label, shortcut, category) in entries {
        palette.register(PaletteEntry {
            id: id.to_string(),
            label: label.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            category: category.to_string(),
        });
    }
}

/// Dispatch an action chosen from the command palette.
fn dispatch_palette_action(app: &mut App, action_id: &str) {
    use crate::theme::Theme;

    match action_id {
        "play_pause" => { app.toggle_play(); app.playback_flash = Some(std::time::Instant::now()); }
        "scrub_left" => app.scrub_left(),
        "scrub_right" => app.scrub_right(),
        "mode_timeline" => { app.mode = Mode::Timeline; app.mode_cursor = 0; app.focused_pane = crate::tui::Pane::Timeline; }
        "mode_inspect" => { app.mode = Mode::Inspect; }
        "mode_search" => { app.mode = Mode::Search; app.search_input.clear(); }
        "toggle_diff" => input::apply_action(app, input::Action::TogglePreviewMode),
        "toggle_commands" => input::apply_action(app, input::Action::ToggleCommandView),
        "toggle_dashboard" => input::apply_action(app, input::Action::ToggleDashboard),
        "toggle_conversation" => input::apply_action(app, input::Action::ToggleConversation),
        "toggle_blame" => input::apply_action(app, input::Action::ToggleBlame),
        "toggle_annotations" => input::apply_action(app, input::Action::ToggleAnnotations),
        "toggle_blast" => input::apply_action(app, input::Action::ToggleBlastRadius),
        "toggle_watchdog" => input::apply_action(app, input::Action::ToggleWatchdog),
        "zoom_in" => input::apply_action(app, input::Action::ZoomTimelineIn),
        "zoom_out" => input::apply_action(app, input::Action::ZoomTimelineOut),
        "zoom_reset" => input::apply_action(app, input::Action::ZoomTimelineReset),
        "solo_track" => input::apply_action(app, input::Action::SoloTrack),
        "mute_track" => input::apply_action(app, input::Action::MuteTrack),
        "restore" => {} // Handled externally in event loop
        "undo_restore" => {} // Handled externally in event loop
        "checkpoint" => {} // Handled externally in event loop
        "session_diff" => {
            app.show_toast(
                "use :diff <from> <to>".to_string(),
                crate::tui::app::ToastStyle::Info,
            );
        }
        "bookmark" => {
            input::apply_action(app, input::Action::CreateBookmark);
        }
        "jump_bookmark" => {
            input::apply_action(app, input::Action::JumpToBookmark);
        }
        "theme_next" => input::apply_action(app, input::Action::CycleTheme),
        "quit" => app.should_quit = true,
        "quit_daemon" => app.should_quit = true,
        id if id.starts_with("theme_") => {
            let name = match id {
                "theme_dark" => "dark",
                "theme_catppuccin" => "catppuccin-mocha",
                "theme_gruvbox" => "gruvbox-dark",
                "theme_tokyo" => "tokyo-night",
                "theme_dracula" => "dracula",
                "theme_nord" => "nord",
                "theme_rose_pine" => "rose-pine",
                _ => return,
            };
            app.theme = Theme::from_preset(name);
            app.theme_name = name.to_string();
            app.theme_flash = Some(std::time::Instant::now());
        }
        _ => {}
    }
}

/// Parse a raw `:diff <from> <to>` command and open the session diff overlay.
fn parse_and_open_diff(app: &mut App, raw: &str) {
    use crate::tui::session_diff::SessionDiff;

    // Expected format: "diff 20 80" or "diff 20 80" (leading "diff " stripped).
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() >= 3 {
        if let (Ok(from), Ok(to)) = (parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
            if app.edits.is_empty() {
                app.show_toast(
                    "no edits to diff".to_string(),
                    crate::tui::app::ToastStyle::Warning,
                );
                return;
            }
            let diff = SessionDiff::compute(&app.edits, from, to);
            app.session_diff_selected = 0;
            app.session_diff = Some(diff);
            return;
        }
    }
    app.show_toast(
        "usage: diff <from> <to>".to_string(),
        crate::tui::app::ToastStyle::Warning,
    );
}
