pub mod app;
pub mod input;
pub mod layout;
pub mod playhead;
pub mod widgets;

pub use app::*;

use crate::analysis::blast_radius::BlastRadiusTracker;
use crate::analysis::sentinels::SentinelEngine;
use crate::analysis::watchdog::Watchdog;
use crate::config::Config;
use crate::event::EditEvent;
use crate::recorder::Recorder;
use crate::session::SessionManager;
use crate::snapshot::checkpoint::CheckpointManager;
use crate::theme::Theme;
use crate::watcher::fs_watcher::FsWatcher;
use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::{self as ct_event, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// ─── keybindings bar muted color (fallback for non-themed spans) ──────────────
const COLOR_MUTED: Color = Color::Rgb(70, 75, 85);

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

/// Options for running the TUI.
#[derive(Default)]
pub struct RunOptions {
    /// If Some, start with a pre-built App (e.g. for replay/import with preloaded edits).
    pub initial_app: Option<App>,
}

/// Run the interactive TUI, watching `project_path` for changes.
pub fn run_tui(project_path: PathBuf, config: Config) -> Result<()> {
    run_tui_with_options(project_path, config, RunOptions::default())
}

/// Run the interactive TUI with extra options (e.g. preloaded edits for replay).
pub fn run_tui_with_options(
    project_path: PathBuf,
    config: Config,
    options: RunOptions,
) -> Result<()> {
    // ── session setup ──────────────────────────────────────────────────────────
    let sessions_dir = project_path.join(".vibetracer").join("sessions");
    let session_manager = SessionManager::new(sessions_dir);
    let session = session_manager.create()?;

    let mut recorder = Recorder::new(project_path.clone(), session.dir.clone());
    let checkpoint_manager = CheckpointManager::new(session.dir.join("checkpoints"));

    // ── file-change channel & watcher ─────────────────────────────────────────
    let (fs_tx, fs_rx) = mpsc::channel::<PathBuf>();
    let mut watcher = FsWatcher::with_ignore(
        project_path.clone(),
        fs_tx,
        config.watch.debounce_ms,
        config.watch.ignore.clone(),
    )?;
    watcher.start()?;

    // Channel used by Recorder to emit EditEvents (not currently consumed
    // externally, but required by the Recorder API for daemon reuse).
    let (edit_event_tx, _edit_event_rx) = mpsc::channel::<EditEvent>();

    // ── terminal setup ────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── app state ─────────────────────────────────────────────────────────────
    let mut app = options.initial_app.unwrap_or_default();
    app.connected = true;
    app.theme = Theme::from_preset(&config.theme.preset);
    app.theme_name = config.theme.preset.clone();

    // Edit count since last checkpoint (for auto-checkpoint).
    let mut edits_since_checkpoint: u32 = 0;

    // Whether the help overlay is visible.
    let mut show_help = false;

    // Analysis engines.
    let watchdog = Watchdog::new(config.watchdog.constants.clone());
    let sentinel_engine = SentinelEngine::new(project_path.clone());
    let blast_tracker = BlastRadiusTracker::new(config.blast_radius.clone());
    // Track which files have been edited this session (for blast radius staleness).
    let mut edited_files: std::collections::HashSet<String> = std::collections::HashSet::new();

    // ── main event loop ───────────────────────────────────────────────────────
    loop {
        // ── render ────────────────────────────────────────────────────────────
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
                buf.set_string(x, y, msg, Style::default().fg(Color::Rgb(158, 90, 90)));
                return;
            }

            let layout = layout::compute_layout(area, app.sidebar_visible);

            // Status bar.
            widgets::status_bar::StatusBar::new(&app).render(layout.status_bar, buf);

            // Sidebar panels (if visible).
            if let Some(sidebar_rect) = layout.sidebar {
                match app.sidebar_panel {
                    SidebarPanel::BlastRadius => {
                        if let Some((ref source, ref status)) = app.blast_radius_status {
                            widgets::blast_radius_panel::BlastRadiusPanel::new(source, status)
                                .render(sidebar_rect, buf);
                        } else {
                            let msg = "no blast radius data";
                            buf.set_string(
                                sidebar_rect.x + 1,
                                sidebar_rect.y + 1,
                                msg,
                                Style::default().fg(Color::Rgb(58, 62, 71)),
                            );
                        }
                    }
                    SidebarPanel::Sentinels => {
                        widgets::sentinel_panel::SentinelPanel::new(&app.sentinel_violations)
                            .render(sidebar_rect, buf);
                    }
                    SidebarPanel::Watchdog => {
                        widgets::watchdog_panel::WatchdogPanel::new(&app.watchdog_alerts)
                            .render(sidebar_rect, buf);
                    }
                }
            }

            // Preview pane.
            widgets::preview::PreviewPane::new(&app).render(layout.preview, buf);

            // Timeline.
            widgets::timeline::TimelineWidget::new(&app).render(layout.timeline, buf);

            // Keybindings bar.
            let kb_line = Line::from(vec![
                Span::styled("b", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" blast radius", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("i", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" sentinels", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("w", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" watchdog", Style::default().fg(COLOR_MUTED)),
            ]);
            kb_line.render(layout.keybindings, buf);

            // Help overlay (on top of everything).
            if show_help {
                widgets::help_overlay::HelpOverlay.render(area, buf);
            }
        })?;

        // ── poll for crossterm events (100 ms timeout) ────────────────────────
        if ct_event::poll(Duration::from_millis(100))? {
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

                            input::Action::Help => {
                                show_help = !show_help;
                            }

                            input::Action::Checkpoint => {
                                let id = checkpoint_manager.save(recorder.current_file_hashes().clone())?;
                                app.checkpoint_ids.push(id);
                                edits_since_checkpoint = 0;
                            }

                            other => {
                                input::apply_action(&mut app, other);
                            }
                        }
                    }
                }
                _ => {} // Ignore mouse events, focus events, etc.
            }
        }

        // ── drain file-change channel (non-blocking) ──────────────────────────
        loop {
            match fs_rx.try_recv() {
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

        // Respect should_quit from apply_action (e.g. 'q' key).
        if app.should_quit {
            break;
        }
    }

    // ── generate session summary before cleaning up ───────────────────────────
    write_session_summary(&session, &app)?;

    // ── restore terminal ──────────────────────────────────────────────────────
    watcher.stop();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // ── session summary ───────────────────────────────────────────────────────
    let duration_secs = (Utc::now().timestamp() - app.session_start).max(0) as u64;
    let summary_path = session.dir.join("summary.md");
    println!(
        "session {} ended — {} edits, {} checkpoints, {}m{}s  (summary: {})",
        session.id,
        app.edits.len(),
        app.checkpoint_ids.len(),
        duration_secs / 60,
        duration_secs % 60,
        summary_path.display(),
    );

    Ok(())
}

fn write_session_summary(session: &crate::session::Session, app: &App) -> Result<()> {
    use std::collections::HashMap;

    let duration_secs = (chrono::Utc::now().timestamp() - app.session_start).max(0) as u64;
    let minutes = duration_secs / 60;
    let seconds = duration_secs % 60;

    let mut file_stats: HashMap<String, (u32, u32, u32)> = HashMap::new(); // (edits, added, removed)
    for edit in &app.edits {
        let entry = file_stats.entry(edit.file.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += edit.lines_added;
        entry.2 += edit.lines_removed;
    }

    let mut summary = String::new();
    summary.push_str("# vibetracer session summary\n\n");
    summary.push_str(&format!("**Session:** {}\n", session.id));
    summary.push_str(&format!("**Duration:** {}m {:02}s\n", minutes, seconds));
    summary.push_str(&format!("**Edits:** {}\n", app.edits.len()));
    summary.push_str(&format!(
        "**Checkpoints:** {}\n\n",
        app.checkpoint_ids.len()
    ));

    // Files changed table (sorted by edit count descending)
    summary.push_str("## Files Changed\n\n");
    summary.push_str("| File | Edits | Lines Added | Lines Removed |\n");
    summary.push_str("|------|-------|-------------|---------------|\n");

    let mut files: Vec<_> = file_stats.iter().collect();
    files.sort_by(|a, b| b.1.0.cmp(&a.1.0));

    for (file, (edits, added, removed)) in &files {
        summary.push_str(&format!(
            "| {} | {} | +{} | -{} |\n",
            file, edits, added, removed
        ));
    }

    // Timeline
    summary.push_str("\n## Timeline\n\n");
    summary.push_str("| # | Offset | File | Lines |\n");
    summary.push_str("|---|--------|------|-------|\n");

    let session_start_ms = app.session_start * 1000;
    for edit in &app.edits {
        let offset_secs = ((edit.ts - session_start_ms).max(0) / 1000) as u64;
        let m = offset_secs / 60;
        let s = offset_secs % 60;
        summary.push_str(&format!(
            "| {} | {}m{:02}s | {} | +{} -{} |\n",
            edit.id, m, s, edit.file, edit.lines_added, edit.lines_removed
        ));
    }

    let summary_path = session.dir.join("summary.md");
    std::fs::write(&summary_path, &summary)?;

    Ok(())
}
