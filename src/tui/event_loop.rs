use crate::analysis::blast_radius::BlastRadiusTracker;
use crate::analysis::sentinels::SentinelEngine;
use crate::analysis::watchdog::Watchdog;
use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::event::EditEvent;
use crate::recorder::Recorder;
use crate::tui::{App, SidebarPanel, input, layout, widgets};
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

            let lo = layout::compute_layout(area, app.sidebar_visible);

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

            // Vertical separator between preview and sidebar.
            if let Some(sidebar_rect) = lo.sidebar {
                let sep_x = sidebar_rect.x.saturating_sub(1);
                let focused = app.focused_pane == crate::tui::Pane::Sidebar;
                let color = if focused { focus_color } else { sep_color };
                for y in lo.main_area.y..lo.main_area.y + lo.main_area.height {
                    if sep_x >= lo.main_area.x && sep_x < lo.main_area.x + lo.main_area.width {
                        buf.set_string(sep_x, y, "\u{2502}", Style::default().fg(color));
                    }
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

            // Keybindings bar.
            let kb_sep = Span::styled(" \u{2502} ", Style::default().fg(app.theme.separator));
            let kb_gap = Span::styled("  ", Style::default());
            let kb_key = |k: &str| Span::styled(k.to_string(), Style::default().fg(app.theme.fg));
            let kb_desc = |d: &str| Span::styled(d.to_string(), Style::default().fg(app.theme.fg_muted));

            let kb_line = Line::from(vec![
                Span::styled(" ", Style::default()),
                kb_key("b"), kb_desc(" blast"), kb_sep.clone(),
                kb_key("i"), kb_desc(" sentinel"), kb_sep.clone(),
                kb_key("w"), kb_desc(" watchdog"),
                kb_gap.clone(),
                kb_key("t"), kb_desc(" theme"), kb_sep.clone(),
                kb_key("g"), kb_desc(" commands"),
                kb_gap.clone(),
                kb_key("?"), kb_desc(" help"),
            ]);
            kb_line.render(lo.keybindings, buf);

            // Help overlay (on top of everything).
            if show_help {
                widgets::help_overlay::HelpOverlay::new(&app.theme).render(area, buf);
            }
        })?;

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
                        let action = input::map_key(key);

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

                            input::Action::Checkpoint => {
                                if let Some(ref mut rec) = recorder {
                                    let id = checkpoint_manager
                                        .save(rec.current_file_hashes().clone())?;
                                    app.checkpoint_ids.push(id);
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
                                                path: edit.file,
                                                from_hash: current_hash,
                                                to_hash: edit.after_hash.clone(),
                                            }],
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
                        MouseEventKind::ScrollUp => {
                            app.timeline_zoom = (app.timeline_zoom * 1.2).min(20.0);
                        }
                        MouseEventKind::ScrollDown => {
                            app.timeline_zoom = (app.timeline_zoom / 1.2).max(1.0);
                            if app.timeline_zoom <= 1.01 {
                                app.timeline_zoom = 1.0;
                                app.timeline_scroll = 0;
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
