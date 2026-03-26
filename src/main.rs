use clap::Parser;
use std::path::PathBuf;

use vibetracer::config::Config;
use vibetracer::import::claude::{import_session, list_sessions};
use vibetracer::restore::RestoreEngine;
use vibetracer::session::SessionManager;
use vibetracer::snapshot::edit_log::EditLog;
use vibetracer::snapshot::store::SnapshotStore;
use vibetracer::tui::{App, PlaybackState, RunOptions};

#[derive(Parser)]
#[command(
    name = "vibetracer",
    about = "Trace, replay, and rewind AI coding edits",
    version
)]
struct Cli {
    /// Project directory to watch (defaults to current directory)
    path: Option<String>,

    /// Write debug log to .vibetracer/debug.log
    #[arg(long)]
    debug: bool,

    /// Internal: run as daemon child process (do not use directly)
    #[arg(long, hide = true)]
    daemon_child: bool,

    /// Disable auto-starting the background daemon (single-process mode)
    #[arg(long)]
    no_daemon: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Replay a past session
    Replay { session_id: String },
    /// List past sessions
    Sessions,
    /// Create default config
    Init,
    /// Import a past Claude Code session for replay
    Import {
        /// Session ID or path to JSONL file (lists available sessions if omitted)
        session: Option<String>,
    },
    /// Restore a file to a prior state
    Restore {
        /// File path to restore (relative to project root)
        file: String,
        /// Edit ID to restore to (from edits.jsonl)
        #[arg(long)]
        edit_id: u64,
    },
    /// Export a session to external formats (Agent Trace JSON, git notes)
    Export {
        /// Output format
        #[arg(long, value_enum)]
        format: vibetracer::export::ExportFormat,
        /// Session ID (from `vibetracer sessions`)
        session_id: String,
        /// Output file path (default: stdout for agent-trace, git note on HEAD for git-notes)
        #[arg(long)]
        output: Option<String>,
    },
    /// Start MCP server (stdio JSON-RPC for AI coding assistants)
    Mcp,
    /// Manage the background recorder daemon
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
}

#[derive(clap::Subcommand)]
enum DaemonCommands {
    /// Start the background recorder
    Start,
    /// Stop the background recorder
    Stop,
    /// Show daemon status
    Status,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // ── Daemon child mode ────────────────────────────────────────────────────
    // When spawned with --daemon-child, run the daemon main loop directly.
    if cli.daemon_child {
        let project_path = resolve_path(cli.path.as_deref())?;
        let config = load_config_or_default(&project_path);
        return vibetracer::daemon::run_daemon(project_path, config);
    }

    match cli.command {
        // ── Daemon subcommands ───────────────────────────────────────────────
        Some(Commands::Daemon { command }) => {
            let project_path = resolve_path(cli.path.as_deref())?;

            match command {
                DaemonCommands::Start => match vibetracer::daemon::start_daemon(&project_path) {
                    Ok((pid, session_id)) => {
                        println!("daemon started (PID {}, session {})", pid, session_id);
                    }
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                },

                DaemonCommands::Stop => match vibetracer::daemon::stop_daemon(&project_path) {
                    Ok(()) => {
                        println!("daemon stopped");
                    }
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                },

                DaemonCommands::Status => {
                    match vibetracer::daemon::daemon_status(&project_path) {
                        Ok(status_json) => {
                            // Pretty-print the status.
                            if let Ok(value) =
                                serde_json::from_str::<serde_json::Value>(&status_json)
                            {
                                let pid = value.get("pid").and_then(|v| v.as_i64()).unwrap_or(0);
                                let session = value
                                    .get("session_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let uptime = value
                                    .get("uptime_secs")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                let edits = value
                                    .get("edit_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let agents = value
                                    .get("agents")
                                    .and_then(|v| v.as_array())
                                    .map(|a| a.len())
                                    .unwrap_or(0);

                                println!("pid:       {}", pid);
                                println!("session:   {}", session);
                                println!("uptime:    {}s", uptime);
                                println!("edits:     {}", edits);
                                println!("agents:    {}", agents);
                            } else {
                                println!("{}", status_json);
                            }
                        }
                        Err(e) => {
                            eprintln!("error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        // ── Init: write auto-detected config ──────────────────────────────────
        Some(Commands::Init) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let vt_dir = project_path.join(".vibetracer");
            std::fs::create_dir_all(&vt_dir)?;
            let config_path = vt_dir.join("config.toml");

            if config_path.exists() {
                println!("config already exists at {}", config_path.display());
            } else {
                let config = vibetracer::auto_detect::auto_detect_config(&project_path);
                let toml_str = toml::to_string_pretty(&config)?;
                let header = "# vibetracer configuration (auto-generated)\n# https://github.com/omeedcs/vibetracer\n# Generated by: vibetracer init\n\n";
                std::fs::write(&config_path, format!("{header}{toml_str}"))?;

                // Print what was detected
                let const_count = config.watchdog.constants.len();
                let dep_count = config.blast_radius.manual.len();
                println!("wrote config to {}", config_path.display());
                if const_count > 0 {
                    println!("  detected {} watchdog constants", const_count);
                }
                if dep_count > 0 {
                    println!("  detected {} dependency mappings", dep_count);
                }
                if const_count == 0 && dep_count == 0 {
                    println!("  no auto-detectable patterns found (edit config manually)");
                }
            }

            // Suggest adding .vibetracer/ to .gitignore
            let gitignore = project_path.join(".gitignore");
            if gitignore.exists() {
                let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
                if !content.contains(".vibetracer") {
                    println!("hint: add .vibetracer/ to your .gitignore");
                }
            } else {
                println!("hint: add .vibetracer/ to your .gitignore");
            }

            // Detect agents
            let agents = vibetracer::import::detect::detect_agents(&project_path);
            if agents.is_empty() {
                println!("  no AI agents detected (start an agent and run init again)");
            } else {
                println!("  detected agents:");
                for agent in &agents {
                    println!("    - {} ({})", agent.name, agent.log_path.display());
                }
            }

            // Configure git notes.rewriteRef for git-notes export compatibility
            let notes_configured = std::process::Command::new("git")
                .args(["config", "--get", "notes.rewriteRef"])
                .current_dir(&project_path)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !notes_configured {
                let _ = std::process::Command::new("git")
                    .args(["config", "notes.rewriteRef", "refs/notes/commits"])
                    .current_dir(&project_path)
                    .output();
                println!("  configured git notes.rewriteRef for export compatibility");
            }
        }

        // ── Sessions: list past sessions ───────────────────────────────────────
        Some(Commands::Sessions) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let sessions_dir = project_path.join(".vibetracer").join("sessions");
            let manager = SessionManager::new(sessions_dir);
            let sessions = manager.list()?;

            if sessions.is_empty() {
                println!("no sessions found");
            } else {
                println!("{:<30}  {:<20}  {:<8}  mode", "id", "started_at", "agents");
                println!("{}", "-".repeat(72));
                for meta in sessions {
                    let dt = chrono::DateTime::from_timestamp(meta.started_at, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| meta.started_at.to_string());
                    let agent_count = meta.agents.len();
                    println!(
                        "{:<30}  {:<20}  {:<8}  {:?}",
                        meta.id, dt, agent_count, meta.mode
                    );
                }
            }
        }

        // ── Replay: load session and replay in TUI ─────────────────────────────
        Some(Commands::Replay { session_id }) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let sessions_dir = project_path.join(".vibetracer").join("sessions");
            let manager = SessionManager::new(sessions_dir);

            // Load edit log for the session.
            let session_dir = manager.sessions_dir.join(&session_id);
            let edit_log_path = session_dir.join("edits.jsonl");

            if !edit_log_path.exists() {
                anyhow::bail!("no edit log found for session {}", session_id);
            }

            let edits = EditLog::read_all(&edit_log_path)?;

            // Build app in Paused mode with preloaded edits.
            let mut app = App::new();
            app.playback = PlaybackState::Paused;
            for edit in edits {
                app.push_edit(edit);
            }
            // Set playhead to beginning for replay.
            if !app.edits.is_empty() {
                app.playhead = 0;
                app.playback = PlaybackState::Paused;
            }

            println!(
                "replaying session {} ({} edits)",
                session_id,
                app.edits.len()
            );

            // Run TUI in replay mode with preloaded edits.
            let config = load_config_or_default(&project_path);
            let options = RunOptions {
                initial_app: Some(app),
                ..Default::default()
            };
            vibetracer::tui::run_tui_with_options(project_path, config, options)?;
        }

        // ── Import: import a past Claude Code session ─────────────────────────
        Some(Commands::Import { session }) => {
            let project_path = resolve_path(cli.path.as_deref())?;

            match session {
                None => {
                    // List available sessions
                    let sessions = list_sessions(&project_path)?;
                    if sessions.is_empty() {
                        println!("no Claude Code sessions found for this project");
                    } else {
                        println!("{:<40}  {:<22}  edits", "id", "started_at");
                        println!("{}", "-".repeat(70));
                        for s in sessions {
                            let dt = chrono::DateTime::from_timestamp_millis(s.started_at)
                                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| s.started_at.to_string());
                            println!("{:<40}  {:<22}  {}", s.id, dt, s.edit_count);
                        }
                    }
                }
                Some(session_arg) => {
                    // Resolve the JSONL path
                    let jsonl_path = if session_arg.ends_with(".jsonl") {
                        PathBuf::from(&session_arg)
                    } else {
                        // Treat as UUID -- look it up under ~/.claude/projects/
                        let home = dirs::home_dir()
                            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
                        let converted = project_path.to_string_lossy().replace('/', "-");
                        home.join(".claude")
                            .join("projects")
                            .join(&converted)
                            .join(format!("{}.jsonl", session_arg))
                    };

                    if !jsonl_path.exists() {
                        anyhow::bail!("session file not found: {}", jsonl_path.display());
                    }

                    let edits = import_session(&jsonl_path, &project_path)?;

                    // Build app in Paused mode with imported edits
                    let mut app = App::new();
                    app.playback = PlaybackState::Paused;
                    for edit in &edits {
                        app.push_edit(edit.clone());
                    }
                    if !app.edits.is_empty() {
                        app.playhead = 0;
                        app.playback = PlaybackState::Paused;
                    }

                    println!(
                        "imported {} edits from {}",
                        app.edits.len(),
                        jsonl_path.display()
                    );

                    // Run TUI with preloaded edits.
                    let config = load_config_or_default(&project_path);
                    let options = RunOptions {
                        initial_app: Some(app),
                        ..Default::default()
                    };
                    vibetracer::tui::run_tui_with_options(project_path, config, options)?;
                }
            }
        }

        // ── Restore: headless file restore to a prior edit ────────────────────
        Some(Commands::Restore { file, edit_id }) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let vt_dir = project_path.join(".vibetracer");

            // Find the active (or most recent) session directory.
            let session_dir = {
                let pid_path = vt_dir.join("daemon.pid");
                if pid_path.exists() {
                    // Daemon is (or was) running -- read session ID from PID file.
                    match vibetracer::daemon::pid::read_pid_file(&pid_path) {
                        Ok((_pid, session_id)) => {
                            let dir = vt_dir.join("sessions").join(&session_id);
                            if dir.exists() {
                                dir
                            } else {
                                find_most_recent_session(&vt_dir)?
                            }
                        }
                        Err(_) => find_most_recent_session(&vt_dir)?,
                    }
                } else {
                    find_most_recent_session(&vt_dir)?
                }
            };

            let edit_log_path = session_dir.join("edits.jsonl");
            if !edit_log_path.exists() {
                anyhow::bail!(
                    "no edit log found in session dir: {}",
                    session_dir.display()
                );
            }

            let edits = EditLog::read_all(&edit_log_path)?;
            let target = edits
                .iter()
                .find(|e| e.id == edit_id)
                .ok_or_else(|| anyhow::anyhow!("edit id {} not found in session", edit_id))?;

            // We restore to the before_hash of the target edit (state before that edit).
            let hash = target.before_hash.as_deref().ok_or_else(|| {
                anyhow::anyhow!("edit {} has no before_hash -- cannot restore", edit_id)
            })?;

            let store_dir = session_dir.join("snapshots");
            let store = SnapshotStore::new(store_dir);
            let engine = RestoreEngine::new(project_path.clone(), store);

            engine.restore_file(&file, hash)?;
            println!("restored {} to state before edit {}", file, edit_id);
        }

        // ── Export: export a session to external formats ─────────────────────
        Some(Commands::Export {
            format,
            session_id,
            output,
        }) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let sessions_dir = project_path.join(".vibetracer").join("sessions");
            let edit_log_path = sessions_dir.join(&session_id).join("edits.jsonl");
            if !edit_log_path.exists() {
                anyhow::bail!("no edit log found for session {}", session_id);
            }
            let edits = EditLog::read_all(&edit_log_path)?;
            match format {
                vibetracer::export::ExportFormat::AgentTrace => {
                    let output_path = output.as_deref().map(std::path::Path::new);
                    vibetracer::export::agent_trace::export_agent_trace_to_path(
                        &edits,
                        &session_id,
                        output_path,
                    )?;
                }
                vibetracer::export::ExportFormat::GitNotes => {
                    vibetracer::export::git_notes::export_git_notes(
                        &edits,
                        &project_path,
                        output.as_deref(),
                    )?;
                }
            }
        }

        // ── MCP: start stdio JSON-RPC server ─────────────────────────────────
        Some(Commands::Mcp) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            vibetracer::mcp::run_mcp_server(project_path)?;
        }

        // ── Default: run live TUI ──────────────────────────────────────────────
        None => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let config = load_config_or_default(&project_path);

            let options = RunOptions {
                no_daemon: cli.no_daemon,
                ..Default::default()
            };

            if cli.debug {
                let log_path = project_path.join(".vibetracer").join("debug.log");
                std::fs::create_dir_all(log_path.parent().unwrap())?;
                let file = std::fs::File::create(&log_path)?;
                tracing_subscriber::fmt()
                    .with_writer(file)
                    .with_ansi(false)
                    .init();
                eprintln!("debug log: {}", log_path.display());
            }

            // Ensure terminal is restored even on panic.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                vibetracer::tui::run_tui_with_options(project_path, config, options)
            }));

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // Normal error -- terminal already restored by run_tui
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
                Err(panic_info) => {
                    // Panic -- force restore terminal
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(
                        std::io::stdout(),
                        crossterm::terminal::LeaveAlternateScreen,
                        crossterm::cursor::Show
                    );
                    eprintln!("vibetracer crashed. Your terminal has been restored.");
                    if let Some(msg) = panic_info.downcast_ref::<&str>() {
                        eprintln!("panic: {msg}");
                    } else if let Some(msg) = panic_info.downcast_ref::<String>() {
                        eprintln!("panic: {msg}");
                    }
                    eprintln!("Please report this at https://github.com/omeedcs/vibetracer/issues");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

/// Resolve the project path from an optional CLI argument (defaults to cwd).
fn resolve_path(arg: Option<&str>) -> anyhow::Result<PathBuf> {
    match arg {
        Some(p) => Ok(PathBuf::from(p)),
        None => Ok(std::env::current_dir()?),
    }
}

/// Load config from `.vibetracer/config.toml`, falling back to defaults.
fn load_config_or_default(project_path: &std::path::Path) -> Config {
    let config_path = project_path.join(".vibetracer").join("config.toml");
    Config::load(&config_path).unwrap_or_default()
}

/// Find the most recently modified session directory under `vt_dir/sessions/`.
fn find_most_recent_session(vt_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let sessions_dir = vt_dir.join("sessions");
    if !sessions_dir.exists() {
        anyhow::bail!("no sessions directory found at {}", sessions_dir.display());
    }

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let modified = entry
            .metadata()?
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
            best = Some((modified, path));
        }
    }

    best.map(|(_, p)| p)
        .ok_or_else(|| anyhow::anyhow!("no sessions found under {}", sessions_dir.display()))
}
