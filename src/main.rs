use clap::Parser;
use std::path::PathBuf;

use vibetracer::config::Config;
use vibetracer::import::claude::{import_session, list_sessions};
use vibetracer::session::SessionManager;
use vibetracer::snapshot::edit_log::EditLog;
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

    /// Run a command embedded in a pane (default: claude)
    #[arg(long, short = 'e')]
    embed: bool,

    /// Command to embed (used with --embed, defaults to "claude")
    #[arg(long, default_value = "claude")]
    cmd: String,

    /// Skip the startup animation
    #[arg(long)]
    no_splash: bool,

    /// Write debug log to .vibetracer/debug.log
    #[arg(long)]
    debug: bool,

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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // ── Init: write default config ─────────────────────────────────────────
        Some(Commands::Init) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            let vt_dir = project_path.join(".vibetracer");
            std::fs::create_dir_all(&vt_dir)?;
            let config_path = vt_dir.join("config.toml");

            if config_path.exists() {
                println!("config already exists at {}", config_path.display());
            } else {
                std::fs::write(&config_path, default_config_with_examples())?;
                println!("wrote config to {}", config_path.display());
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
                println!("{:<30}  {:<20}  mode", "id", "started_at");
                println!("{}", "-".repeat(60));
                for meta in sessions {
                    let dt = chrono::DateTime::from_timestamp(meta.started_at, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| meta.started_at.to_string());
                    println!("{:<30}  {:<20}  {:?}", meta.id, dt, meta.mode);
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

            // Run TUI in replay mode (no live watcher — load config or use default).
            let config = load_config_or_default(&project_path);
            vibetracer::tui::run_tui(project_path, config)?;
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
                        // Treat as UUID — look it up under ~/.claude/projects/
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

                    let config = load_config_or_default(&project_path);
                    vibetracer::tui::run_tui(project_path, config)?;
                }
            }
        }

        // ── Default: run live TUI ──────────────────────────────────────────────
        None => {
            if !cli.no_splash {
                vibetracer::splash::play_splash()?;
            }

            let project_path = resolve_path(cli.path.as_deref())?;
            let config = load_config_or_default(&project_path);

            let options = if cli.embed {
                RunOptions {
                    embed_command: Some(cli.cmd.clone()),
                }
            } else {
                RunOptions::default()
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
                    // Normal error — terminal already restored by run_tui
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
                Err(panic_info) => {
                    // Panic — force restore terminal
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

/// Generate a config file with commented examples showing how to use each feature.
fn default_config_with_examples() -> String {
    r#"# vibetracer configuration
# https://github.com/omeedcs/vibetracer

[watch]
debounce_ms = 100
ignore = [".git", "node_modules", "target", "__pycache__", ".vibetracer", ".venv"]
auto_checkpoint_every = 25

# ── Watchdog ─────────────────────────────────────────────────────────────────
# Register constants that should almost never change.
# vibetracer alerts you instantly if an AI edit modifies one.
#
# [[watchdog.constants]]
# file = "**/*.py"
# pattern = 'EARTH_RADIUS_KM\s*=\s*([\d.]+)'
# expected = "6371.0"
# severity = "critical"    # "critical" = full alert, "warning" = sidebar note
#
# [[watchdog.constants]]
# file = "**/*.rs"
# pattern = 'const\s+MAX_RETRIES\s*:\s*\w+\s*=\s*(\d+)'
# expected = "3"
# severity = "warning"

[watchdog]
constants = []

# ── Sentinels ────────────────────────────────────────────────────────────────
# Cross-file invariant rules. vibetracer evaluates these on every edit
# and alerts you when values fall out of sync.
#
# [sentinels.tensor_dims]
# description = "feature count must match model input size"
# watch = "**/*.py"
# rule = "grep_match"
# pattern_a = { file = "feature_config.py", regex = 'N_FEATURES\s*=\s*(\d+)' }
# pattern_b = { file = "model.py", regex = 'input_size\s*=\s*(\d+)' }
# assert = "a == b"

[sentinels]

# ── Blast Radius ─────────────────────────────────────────────────────────────
# Declare file dependencies so vibetracer can warn you when a source file
# is edited but its dependents haven't been updated yet.
#
# [[blast_radius.manual]]
# source = "**/feature_config*.py"
# dependents = ["**/predictor*.py", "**/serving*.py", "tests/test_model*.py"]

[blast_radius]
auto_detect = true
manual = []
"#
    .to_string()
}
