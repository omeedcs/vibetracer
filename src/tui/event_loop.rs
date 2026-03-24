use crate::analysis::blast_radius::BlastRadiusTracker;
use crate::analysis::sentinels::SentinelEngine;
use crate::analysis::watchdog::Watchdog;
use crate::config::Config;
use crate::event::EditEvent;
use crate::recorder::Recorder;
use crate::checkpoint::CheckpointManager;
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

/// Run the main TUI event loop.
///
/// This function owns the render cycle, input handling, file-change processing,
/// and analysis engine invocation. The caller sets up the terminal, session,
/// recorder, watcher, and config, then hands everything in here.
pub fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    recorder: &mut Recorder,
    checkpoint_manager: &CheckpointManager,
    fs_rx: &mpsc::Receiver<PathBuf>,
    config: &Config,
    project_path: &Path,
) -> Result<()> {
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

            let lo = layout::compute_layout(area, app.sidebar_visible);

            // Status bar.
            widgets::status_bar::StatusBar::new(app).render(lo.status_bar, buf);

            // Sidebar panels (if visible).
            if let Some(sidebar_rect) = lo.sidebar {
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
            widgets::preview::PreviewPane::new(app).render(lo.preview, buf);

            // Timeline.
            widgets::timeline::TimelineWidget::new(app).render(lo.timeline, buf);

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
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("t", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" theme", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("g", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" commands", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("?", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" help", Style::default().fg(COLOR_MUTED)),
            ]);
            kb_line.render(lo.keybindings, buf);

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
                            input::Action::Quit | input::Action::QuitAndStopDaemon => break,

                            input::Action::Help => {
                                show_help = !show_help;
                            }

                            input::Action::Checkpoint => {
                                let id = checkpoint_manager
                                    .save(recorder.current_file_hashes().clone())?;
                                app.checkpoint_ids.push(id);
                                edits_since_checkpoint = 0;
                            }

                            other => {
                                input::apply_action(app, other);
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

    Ok(())
}
