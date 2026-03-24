pub mod app;
pub mod event_loop;
pub mod input;
pub mod layout;
pub mod operation;
pub mod playhead;
pub mod widgets;

pub use app::*;

use crate::config::Config;
use crate::recorder::Recorder;
use crate::session::SessionManager;
use crate::snapshot::checkpoint::CheckpointManager;
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

    // ── event loop ────────────────────────────────────────────────────────────
    event_loop::run_event_loop(
        &mut terminal,
        &mut app,
        &mut recorder,
        &checkpoint_manager,
        &fs_rx,
        &config,
        &project_path,
    )?;

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
