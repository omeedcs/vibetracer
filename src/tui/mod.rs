pub mod app;
pub mod event_loop;
pub mod input;
pub mod layout;
pub mod operation;
pub mod playhead;
pub mod syntax;
pub mod tailer;
pub mod widgets;

pub use app::*;

use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::daemon;
use crate::recorder::Recorder;
use crate::session::SessionManager;
use crate::theme::Theme;
use crate::watcher::fs_watcher::FsWatcher;
use anyhow::Result;
use chrono::Utc;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;

/// Options for running the TUI.
#[derive(Default)]
pub struct RunOptions {
    /// If Some, start with a pre-built App (e.g. for replay/import with preloaded edits).
    pub initial_app: Option<App>,
    /// If true, skip daemon auto-start and run file watching in-process.
    pub no_daemon: bool,
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
    let is_replay = options.initial_app.is_some();

    // ── daemon / session setup ────────────────────────────────────────────────
    let vt_dir = project_path.join(".vibetracer");
    let pid_path = vt_dir.join("daemon.pid");
    let sock_path = vt_dir.join("daemon.sock");

    // Determine whether we connect to a daemon or run in single-process mode.
    // Replay mode always skips the daemon (it uses preloaded edits).
    let (session_dir, daemon_running) = if is_replay || options.no_daemon {
        // Single-process mode: create a session ourselves.
        let sessions_dir = vt_dir.join("sessions");
        let session_manager = SessionManager::new(sessions_dir);
        let session = session_manager.create()?;
        (session.dir, false)
    } else {
        // Check for a running daemon.
        let alive = if pid_path.exists() {
            match daemon::pid::read_pid_file(&pid_path) {
                Ok((pid, _)) => daemon::pid::is_process_alive(pid),
                Err(_) => false,
            }
        } else {
            false
        };

        if !alive {
            // Clean up stale artifacts if present, then start the daemon.
            if pid_path.exists() {
                daemon::pid::cleanup_stale(&pid_path, &sock_path)?;
            }
            daemon::start_daemon(&project_path)?;
        }

        // Read session ID from the daemon's PID file.
        let (_pid, session_id) = daemon::pid::read_pid_file(&pid_path)?;
        let session_dir = vt_dir.join("sessions").join(&session_id);
        (session_dir, true)
    };

    let checkpoint_manager = CheckpointManager::new(session_dir.join("checkpoints"));

    // Derive a session-like struct for summary writing.
    let session_id = session_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let session = crate::session::Session {
        id: session_id,
        dir: session_dir.clone(),
    };

    // ── source mode: daemon tail vs. in-process watcher ───────────────────────
    //
    // In daemon mode we tail edits.jsonl for pre-built EditEvents.
    // In no-daemon mode we run FsWatcher + Recorder ourselves.
    let edit_log_path = session_dir.join("edits.jsonl");

    // These are only used in no-daemon mode but must be declared here so
    // they live long enough (watcher must not be dropped while the event
    // loop runs).
    let mut recorder_opt: Option<Recorder> = None;
    let mut watcher_opt: Option<FsWatcher> = None;
    let fs_rx_opt: Option<mpsc::Receiver<PathBuf>>;
    let edit_rx_opt: Option<mpsc::Receiver<crate::event::EditEvent>>;
    let mut preloaded_edits: Vec<crate::event::EditEvent> = Vec::new();

    if daemon_running {
        // Tail the daemon's edit log.
        let (existing, rx) = tailer::tail_edit_log(edit_log_path)?;
        preloaded_edits = existing;
        edit_rx_opt = Some(rx);
        fs_rx_opt = None;
    } else if !is_replay {
        // No-daemon mode: run watcher + recorder in-process.
        let recorder = Recorder::new(project_path.clone(), session_dir.clone());
        let (fs_tx, fs_rx) = mpsc::channel::<PathBuf>();
        let mut watcher = FsWatcher::with_ignore(
            project_path.clone(),
            fs_tx,
            config.watch.debounce_ms,
            config.watch.ignore.clone(),
        )?;
        watcher.start()?;

        recorder_opt = Some(recorder);
        watcher_opt = Some(watcher);
        fs_rx_opt = Some(fs_rx);
        edit_rx_opt = None;
    } else {
        // Replay mode: no watcher, no tailer.
        fs_rx_opt = None;
        edit_rx_opt = None;
    }

    // ── terminal setup ────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── app state ─────────────────────────────────────────────────────────────
    let mut app = options.initial_app.unwrap_or_default();

    // Load existing edits from daemon's edit log (if any).
    for edit in preloaded_edits {
        app.push_edit(edit);
    }

    app.connected = true;
    app.theme = Theme::from_preset(&config.theme.preset);
    app.theme_name = config.theme.preset.clone();

    // ── event loop ────────────────────────────────────────────────────────────
    event_loop::run_event_loop(
        &mut terminal,
        &mut app,
        recorder_opt.as_mut(),
        &checkpoint_manager,
        fs_rx_opt.as_ref(),
        edit_rx_opt.as_ref(),
        &config,
        &project_path,
        &session_dir,
        daemon_running,
    )?;

    // ── generate session summary before cleaning up ───────────────────────────
    write_session_summary(&session, &app)?;

    // ── restore terminal ──────────────────────────────────────────────────────
    if let Some(ref mut watcher) = watcher_opt {
        watcher.stop();
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture
    )?;
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
