pub mod input;
pub mod layout;
pub mod widgets;

use crate::analysis::blast_radius::{BlastRadiusTracker, DependencyStatus};
use crate::analysis::sentinels::{SentinelEngine, SentinelViolation};
use crate::analysis::watchdog::{Watchdog, WatchdogAlert};
use crate::config::Config;
use crate::equation::detect::{self as eq_detect, DetectedEquation};
use crate::event::{EditEvent, EditKind};
use crate::pty::EmbeddedTerminal;
use crate::session::SessionManager;
use crate::snapshot::{checkpoint::CheckpointManager, edit_log::EditLog, store::SnapshotStore};
use crate::theme::Theme;
use crate::watcher::{differ::compute_diff, fs_watcher::FsWatcher};
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
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// Which primary pane currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum Pane {
    Preview,
    Timeline,
    Sidebar,
    TerminalPane,
}

/// Which panel is active inside the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarPanel {
    BlastRadius,
    Sentinels,
    Watchdog,
    Refactor,
    Equations,
}

/// Current playback mode.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    /// Following live edits as they arrive.
    Live,
    /// Paused at a fixed position in the timeline.
    Paused,
    /// Playing back at the given speed multiplier.
    Playing { speed: u8 },
}

/// Per-file track metadata shown in the timeline.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub filename: String,
    pub edit_indices: Vec<usize>,
    pub stale: bool,
}

/// Top-level application state for the TUI.
pub struct App {
    pub edits: Vec<EditEvent>,
    pub playhead: usize,
    pub playback: PlaybackState,

    pub focused_pane: Pane,
    pub sidebar_visible: bool,
    pub sidebar_panel: SidebarPanel,

    pub equation_lens: bool,
    pub schema_diff_mode: bool,

    pub solo_track: Option<String>,
    pub muted_tracks: Vec<String>,

    pub checkpoint_ids: Vec<u32>,
    pub session_start: i64,

    pub connected: bool,
    pub should_quit: bool,
    pub tracks: Vec<TrackInfo>,

    // Embedded terminal state
    pub terminal: Option<EmbeddedTerminal>,
    pub terminal_output: Vec<String>,
    pub terminal_visible: bool,
    pub last_vibetracer_pane: Option<Pane>,

    // Analysis state (populated by the event loop)
    pub watchdog_alerts: Vec<WatchdogAlert>,
    pub sentinel_violations: Vec<SentinelViolation>,
    pub blast_radius_status: Option<(String, DependencyStatus)>,
    pub equations: Vec<DetectedEquation>,

    // Color theme
    pub theme: Theme,
}

impl App {
    /// Create a new `App` with sensible defaults.
    pub fn new() -> Self {
        App {
            edits: Vec::new(),
            playhead: 0,
            playback: PlaybackState::Live,

            focused_pane: Pane::Timeline,
            sidebar_visible: false,
            sidebar_panel: SidebarPanel::BlastRadius,

            equation_lens: false,
            schema_diff_mode: false,

            solo_track: None,
            muted_tracks: Vec::new(),

            checkpoint_ids: Vec::new(),
            session_start: Utc::now().timestamp(),

            connected: false,
            should_quit: false,
            tracks: Vec::new(),

            terminal: None,
            terminal_output: Vec::new(),
            terminal_visible: false,
            last_vibetracer_pane: None,

            watchdog_alerts: Vec::new(),
            sentinel_violations: Vec::new(),
            blast_radius_status: None,
            equations: Vec::new(),

            theme: Theme::dark(),
        }
    }

    /// Push a new edit into the log, update or create its track entry,
    /// and advance the playhead if in Live mode.
    pub fn push_edit(&mut self, edit: EditEvent) {
        let idx = self.edits.len();
        let file = edit.file.clone();
        self.edits.push(edit);

        // Update the track for this file.
        if let Some(track) = self.tracks.iter_mut().find(|t| t.filename == file) {
            track.edit_indices.push(idx);
            track.stale = false;
        } else {
            self.tracks.push(TrackInfo {
                filename: file,
                edit_indices: vec![idx],
                stale: false,
            });
        }

        // In Live mode, keep the playhead at the latest edit.
        if self.playback == PlaybackState::Live {
            self.playhead = self.edits.len().saturating_sub(1);
        }
    }

    /// Return a reference to the edit currently at the playhead position, if any.
    pub fn current_edit(&self) -> Option<&EditEvent> {
        if self.edits.is_empty() {
            None
        } else {
            self.edits.get(self.playhead)
        }
    }

    /// Move the playhead one step to the left (backward). Sets state to Paused.
    pub fn scrub_left(&mut self) {
        self.playback = PlaybackState::Paused;
        if self.playhead > 0 {
            self.playhead -= 1;
        }
    }

    /// Move the playhead one step to the right (forward). If we reach the end,
    /// return to Live mode.
    pub fn scrub_right(&mut self) {
        if self.edits.is_empty() {
            return;
        }
        let last = self.edits.len() - 1;
        if self.playhead < last {
            self.playhead += 1;
        }
        if self.playhead >= last {
            self.playback = PlaybackState::Live;
        }
    }

    /// Cycle playback state: Live -> Paused, Paused -> Playing{1}, Playing -> Paused.
    pub fn toggle_play(&mut self) {
        self.playback = match &self.playback {
            PlaybackState::Live => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing { speed: 1 },
            PlaybackState::Playing { .. } => PlaybackState::Paused,
        };
    }

    /// Update playback speed; only has effect if currently Playing.
    pub fn set_speed(&mut self, speed: u8) {
        if let PlaybackState::Playing { .. } = self.playback {
            self.playback = PlaybackState::Playing { speed };
        }
    }

    /// Refresh terminal output from the embedded terminal into terminal_output.
    pub fn sync_terminal_output(&mut self) {
        if let Some(ref term) = self.terminal {
            self.terminal_output = term.get_output(1000);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// If Some, spawn this command in the embedded terminal on startup.
    pub embed_command: Option<String>,
}

/// Run the interactive TUI, watching `project_path` for changes.
pub fn run_tui(project_path: PathBuf, config: Config) -> Result<()> {
    run_tui_with_options(project_path, config, RunOptions::default())
}

/// Run the interactive TUI with extra options (e.g. embedded terminal).
pub fn run_tui_with_options(
    project_path: PathBuf,
    config: Config,
    options: RunOptions,
) -> Result<()> {
    // ── session setup ──────────────────────────────────────────────────────────
    let sessions_dir = project_path.join(".vibetracer").join("sessions");
    let session_manager = SessionManager::new(sessions_dir);
    let session = session_manager.create()?;

    let snapshot_store = SnapshotStore::new(session.dir.join("snapshots"));
    let edit_log = EditLog::new(session.dir.join("edits.jsonl"));
    let checkpoint_manager = CheckpointManager::new(session.dir.join("checkpoints"));

    // ── file-change channel & watcher ─────────────────────────────────────────
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher = FsWatcher::with_ignore(
        project_path.clone(),
        tx,
        config.watch.debounce_ms,
        config.watch.ignore.clone(),
    )?;
    watcher.start()?;

    // ── terminal setup ────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── app state ─────────────────────────────────────────────────────────────
    let mut app = App::new();
    app.connected = true;
    app.theme = Theme::from_preset(&config.theme.preset);

    // ── embedded terminal (if requested) ─────────────────────────────────────
    if let Some(ref cmd) = options.embed_command {
        match EmbeddedTerminal::new(80, 24, Some(cmd.as_str())) {
            Ok(embed) => {
                embed.start_reader();
                app.terminal = Some(embed);
                app.terminal_visible = true;
                app.focused_pane = Pane::TerminalPane;
            }
            Err(e) => {
                // Non-fatal: log but continue without embedded terminal.
                eprintln!("warning: could not start embedded terminal: {e}");
            }
        }
    }

    // Track last-known snapshot hashes per file path.
    let mut file_hashes: HashMap<String, String> = HashMap::new();
    // Auto-incrementing edit ID counter.
    let mut edit_id_counter: u64 = 1;
    // Edit count since last checkpoint (for auto-checkpoint).
    let mut edits_since_checkpoint: u32 = 0;

    // Current snapshot hashes for checkpoint saving.
    let mut current_file_hashes: HashMap<String, String> = HashMap::new();

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
        // Sync terminal output into app state before rendering.
        app.sync_terminal_output();

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

            let layout = layout::compute_layout(area, app.sidebar_visible, app.terminal_visible);

            // Status bar.
            widgets::status_bar::StatusBar::new(&app).render(layout.status_bar, buf);

            // Embedded terminal pane (if visible).
            if let Some(term_rect) = layout.terminal {
                let focused = app.focused_pane == Pane::TerminalPane;
                widgets::terminal_pane::TerminalPane::new(&app.terminal_output, focused)
                    .render(term_rect, buf);
            }

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
                    SidebarPanel::Refactor => {
                        widgets::refactor_panel::RefactorPanel::new(None).render(sidebar_rect, buf);
                    }
                    SidebarPanel::Equations => {
                        widgets::equation_panel::EquationPanel::new(&app.equations, None)
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
                Span::styled("d", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" schema diff", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("f", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" refactor", Style::default().fg(COLOR_MUTED)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled("e", Style::default().fg(Color::Rgb(138, 143, 152))),
                Span::styled(" equations", Style::default().fg(COLOR_MUTED)),
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
                    // Resize the embedded PTY to match the actual terminal pane rect.
                    if let Some(ref term) = app.terminal {
                        if app.terminal_visible {
                            let size = terminal.size()?;
                            let area = Rect::new(0, 0, size.width, size.height);
                            let lo = layout::compute_layout(area, app.sidebar_visible, true);
                            if let Some(term_rect) = lo.terminal {
                                let _ = term.resize(
                                    term_rect.width.max(10),
                                    term_rect.height.saturating_sub(1).max(3),
                                );
                            }
                        }
                    }
                    continue;
                }
                Event::Key(key) => {
                    // Ignore key-release events on platforms that emit them.
                    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                        // Check for Ctrl+\ first — global toggle regardless of focus.
                        use crossterm::event::{KeyCode, KeyModifiers};
                        if key.code == KeyCode::Char('\\')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            input::apply_action(&mut app, input::Action::ToggleTerminalFocus);
                            continue;
                        }

                        // When terminal pane is focused, forward all keys to the PTY.
                        if app.focused_pane == Pane::TerminalPane {
                            if let Some(ref term) = app.terminal {
                                let _ = term.send_key(key);
                            }
                            continue;
                        }

                        let action = input::map_key(key);

                        match action {
                            input::Action::Quit => break,

                            input::Action::Help => {
                                show_help = !show_help;
                            }

                            input::Action::Checkpoint => {
                                let id = checkpoint_manager.save(current_file_hashes.clone())?;
                                app.checkpoint_ids.push(id);
                                edits_since_checkpoint = 0;
                            }

                            input::Action::ToggleEquationLens => {
                                input::apply_action(&mut app, input::Action::ToggleEquationLens);
                                // Scan the current file for equations immediately.
                                if app.equation_lens {
                                    if let Some(edit) = app.current_edit() {
                                        let file_path = project_path.join(&edit.file);
                                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                                            app.equations = eq_detect::extract_equations(&content);
                                        }
                                    }
                                }
                            }

                            input::Action::ScrubLeft | input::Action::ScrubRight => {
                                input::apply_action(&mut app, action);
                                // Rescan equations if lens is on after scrubbing.
                                if app.equation_lens {
                                    if let Some(edit) = app.current_edit() {
                                        let file_path = project_path.join(&edit.file);
                                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                                            app.equations = eq_detect::extract_equations(&content);
                                        }
                                    }
                                }
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
            match rx.try_recv() {
                Ok(abs_path) => {
                    // Compute relative path from project root.
                    let rel_path = abs_path
                        .strip_prefix(&project_path)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| abs_path.to_string_lossy().to_string());

                    // Read new content from disk (or treat as delete if missing).
                    let new_content = std::fs::read_to_string(&abs_path).unwrap_or_default();

                    // Look up old content from snapshot store (empty if first edit).
                    let old_content = if let Some(hash) = file_hashes.get(&rel_path) {
                        snapshot_store
                            .retrieve(hash)
                            .ok()
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    // Skip if content hasn't changed.
                    if old_content == new_content {
                        continue;
                    }

                    // Compute diff.
                    let diff = compute_diff(&old_content, &new_content, &rel_path);

                    // Determine edit kind.
                    let kind = if !abs_path.exists() {
                        EditKind::Delete
                    } else if file_hashes.contains_key(&rel_path) {
                        EditKind::Modify
                    } else {
                        EditKind::Create
                    };

                    // Store new snapshot.
                    let after_hash = snapshot_store.store(new_content.as_bytes())?;

                    let before_hash = file_hashes.get(&rel_path).cloned();

                    // Build edit event.
                    let edit = EditEvent {
                        id: edit_id_counter,
                        ts: Utc::now().timestamp_millis(),
                        file: rel_path.clone(),
                        kind,
                        patch: diff.patch,
                        before_hash,
                        after_hash: after_hash.clone(),
                        intent: None,
                        tool: None,
                        lines_added: diff.lines_added,
                        lines_removed: diff.lines_removed,
                    };

                    edit_id_counter += 1;

                    // Persist.
                    edit_log.append(&edit)?;

                    // Update state.
                    file_hashes.insert(rel_path.clone(), after_hash.clone());
                    current_file_hashes.insert(rel_path.clone(), after_hash);
                    edited_files.insert(rel_path.clone());
                    app.push_edit(edit);

                    // -- Run analysis engines on this edit --

                    // Watchdog: check if a registered constant was modified.
                    let alerts = watchdog.check(&rel_path, &old_content, &new_content);
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
                        let status = blast_tracker.check_staleness(&rel_path, &edited_files);
                        app.blast_radius_status = Some((rel_path.clone(), status));
                        app.sidebar_visible = true;
                        app.sidebar_panel = SidebarPanel::BlastRadius;
                    }

                    // Equations: if equation lens is on, detect equations.
                    if app.equation_lens {
                        app.equations = eq_detect::extract_equations(&new_content);
                    }

                    edits_since_checkpoint += 1;

                    // Auto-checkpoint.
                    if config.watch.auto_checkpoint_every > 0
                        && edits_since_checkpoint >= config.watch.auto_checkpoint_every
                    {
                        let id = checkpoint_manager.save(current_file_hashes.clone())?;
                        app.checkpoint_ids.push(id);
                        edits_since_checkpoint = 0;
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
