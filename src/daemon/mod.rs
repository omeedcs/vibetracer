pub mod agent_registry;
pub mod correlation;
pub mod hook_listener;
pub mod pid;

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::Config;
use crate::recorder::{Enrichment, Recorder};
use crate::session::SessionManager;
use crate::watcher::fs_watcher::FsWatcher;

use agent_registry::AgentRegistry;
use correlation::Correlator;
use hook_listener::SocketMessage;

/// Standard filesystem locations for daemon artifacts, relative to
/// `<project>/.vibetracer/`.
fn pid_path(project_path: &std::path::Path) -> PathBuf {
    project_path.join(".vibetracer").join("daemon.pid")
}

fn sock_path(project_path: &std::path::Path) -> PathBuf {
    project_path.join(".vibetracer").join("daemon.sock")
}

/// Run the daemon process.
///
/// This is the entry point called by `--daemon-child`. It:
/// 1. Creates a new session.
/// 2. Writes the PID file.
/// 3. Starts the file watcher and socket listener.
/// 4. Enters the main loop processing file changes and socket messages.
/// 5. Cleans up on shutdown.
pub fn run_daemon(project_path: PathBuf, config: Config) -> Result<()> {
    let vt_dir = project_path.join(".vibetracer");
    std::fs::create_dir_all(&vt_dir)?;

    // 1. Create a new session.
    let sessions_dir = vt_dir.join("sessions");
    let session_mgr = SessionManager::new(sessions_dir);
    let session = session_mgr.create()?;

    let pid_file = pid_path(&project_path);
    let sock_file = sock_path(&project_path);
    let my_pid = std::process::id() as i32;

    // 2. Write PID file so the parent process (and future CLI commands) can
    //    discover this daemon.
    pid::write_pid_file(&pid_file, my_pid, &session.id)?;

    // 3. Create the recorder.
    let recorder = Recorder::new(project_path.clone(), session.dir.clone());

    // 4. Start file watcher.
    let (fs_tx, fs_rx) = mpsc::channel::<PathBuf>();
    let mut watcher = FsWatcher::with_ignore(
        project_path.clone(),
        fs_tx,
        config.watch.debounce_ms,
        config.watch.ignore.clone(),
    )?;
    watcher.start()?;

    // 5. Start socket listener thread.
    let (sock_tx, sock_rx) = mpsc::channel::<SocketMessage>();
    let sock_file_clone = sock_file.clone();
    let _listener_thread = std::thread::spawn(move || {
        if let Err(e) = hook_listener::listen(&sock_file_clone, sock_tx) {
            tracing::error!("socket listener error: {}", e);
        }
    });

    // 6. Create correlator and agent registry.
    let mut correlator = Correlator::new();
    let mut agent_registry = AgentRegistry::new();

    // Channel for sending EditEvents (Recorder requires one, but the daemon
    // doesn't consume them through the channel -- it uses them inline).
    let (event_tx, _event_rx) = mpsc::channel();

    // We need mutable access to recorder in the loop.
    let mut recorder = recorder;
    let mut edit_count: u64 = 0;

    // 7. Main loop.
    loop {
        let mut should_stop = false;

        // 7a. Drain socket messages.
        while let Ok(msg) = sock_rx.try_recv() {
            match msg {
                SocketMessage::Hook(payload, file) => {
                    // Register or update the agent.
                    let ts = Utc::now().timestamp_millis();
                    agent_registry.register_or_update(
                        &payload.agent_id,
                        "claude-code",
                        ts,
                    );
                    // Push enrichment for correlation.
                    correlator.push_enrichment(&file, payload);
                }

                SocketMessage::RestoreStart { restore_id, files } => {
                    correlator.register_restore(restore_id, &files);
                }

                SocketMessage::RestoreEnd { restore_id } => {
                    correlator.clear_restore(restore_id);
                }

                SocketMessage::StatusQuery(mut stream) => {
                    let agents = agent_registry.to_vec();
                    let uptime_secs = {
                        // Calculate from session start time.
                        let meta_path = session.dir.join("meta.json");
                        if let Ok(content) = std::fs::read_to_string(&meta_path) {
                            if let Ok(meta) =
                                serde_json::from_str::<crate::session::SessionMeta>(&content)
                            {
                                (Utc::now().timestamp() - meta.started_at).max(0)
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    };

                    let status = serde_json::json!({
                        "type": "status",
                        "pid": my_pid,
                        "session_id": session.id,
                        "uptime_secs": uptime_secs,
                        "edit_count": edit_count,
                        "agents": agents,
                    });

                    let _ = writeln!(stream, "{}", status);
                }

                SocketMessage::Stop => {
                    should_stop = true;
                }
            }
        }

        if should_stop {
            break;
        }

        // 7b. Drain file changes.
        while let Ok(abs_path) = fs_rx.try_recv() {
            // Compute relative path for correlation lookup.
            let rel_path = abs_path
                .strip_prefix(&project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| abs_path.to_string_lossy().to_string());

            // Build enrichment: restore takes precedence over hook.
            let enrichment = if let Some(restore_id) = correlator.pop_restore(&rel_path) {
                // Restore wins -- discard any hook enrichment for this file.
                let _ = correlator.pop_enrichment(&rel_path);
                Some(Enrichment {
                    restore_id: Some(restore_id),
                    ..Default::default()
                })
            } else if let Some(hook) = correlator.pop_enrichment(&rel_path) {
                let label = agent_registry
                    .get(&hook.agent_id)
                    .map(|info| info.agent_label.clone());
                Some(Enrichment {
                    agent_id: Some(hook.agent_id.clone()),
                    agent_label: label,
                    operation_id: Some(hook.operation_id),
                    operation_intent: hook.intent,
                    tool_name: Some(hook.tool_name),
                    restore_id: None,
                })
            } else {
                None
            };

            match recorder.process_file_change(
                &abs_path,
                &event_tx,
                enrichment.as_ref(),
            ) {
                Ok(Some(_result)) => {
                    edit_count += 1;
                    // Increment agent edit count if enrichment came from a hook.
                    if let Some(ref enrich) = enrichment {
                        if let Some(ref agent_id) = enrich.agent_id {
                            let ts = Utc::now().timestamp_millis();
                            agent_registry.increment_edit_count(agent_id, ts);
                        }
                    }
                }
                Ok(None) => {
                    // No actual change detected -- skip.
                }
                Err(e) => {
                    tracing::warn!("error processing file change {:?}: {}", abs_path, e);
                }
            }
        }

        // 7c. Cleanup stale enrichments (5 second threshold).
        correlator.cleanup_stale(5_000);

        // 7d. Sleep 50ms before next iteration.
        std::thread::sleep(Duration::from_millis(50));
    }

    // Shutdown: update session metadata with final agent list.
    let meta_path = session.dir.join("meta.json");
    if let Ok(content) = std::fs::read_to_string(&meta_path) {
        if let Ok(mut meta) =
            serde_json::from_str::<crate::session::SessionMeta>(&content)
        {
            meta.agents = agent_registry.to_vec();
            if let Ok(json) = serde_json::to_string_pretty(&meta) {
                let _ = std::fs::write(&meta_path, json);
            }
        }
    }

    // Cleanup PID file and socket.
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&sock_file);

    Ok(())
}

/// Start the daemon as a detached child process.
///
/// Returns `Ok(pid, session_id)` on success after confirming the daemon wrote
/// its PID file.
pub fn start_daemon(project_path: &std::path::Path) -> Result<(i32, String)> {
    let pid_file = pid_path(project_path);
    let sock_file = sock_path(project_path);

    // Check for an already-running daemon.
    if pid_file.exists() {
        let (existing_pid, existing_session) = pid::read_pid_file(&pid_file)?;
        if pid::is_process_alive(existing_pid) {
            anyhow::bail!(
                "daemon already running (PID {}, session {})",
                existing_pid,
                existing_session
            );
        }
        // Stale PID file from a crashed daemon.
        pid::cleanup_stale(&pid_file, &sock_file)?;
    }

    // Spawn the daemon as a child process.
    let exe = std::env::current_exe().context("resolve current executable path")?;
    let project_str = project_path
        .to_str()
        .context("project path is not valid UTF-8")?;

    let child = std::process::Command::new(exe)
        .arg("--daemon-child")
        .arg(project_str)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawn daemon child process")?;

    // We don't wait on the child -- it runs independently.
    drop(child);

    // Poll for the PID file to appear (up to 3 seconds, 50ms intervals).
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if pid_file.exists() {
            match pid::read_pid_file(&pid_file) {
                Ok((pid, session_id)) => return Ok((pid, session_id)),
                Err(_) => {
                    // File exists but not fully written yet.
                }
            }
        }

        if std::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for daemon to start (3 seconds)");
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Stop a running daemon by sending a stop command over the Unix socket.
///
/// Falls back to SIGTERM if the daemon doesn't exit within 5 seconds.
pub fn stop_daemon(project_path: &std::path::Path) -> Result<()> {
    let pid_file = pid_path(project_path);
    let sock_file = sock_path(project_path);

    if !pid_file.exists() {
        anyhow::bail!("no daemon running (PID file not found)");
    }

    let (daemon_pid, _session_id) = pid::read_pid_file(&pid_file)?;

    if !pid::is_process_alive(daemon_pid) {
        // Daemon already dead -- clean up.
        pid::cleanup_stale(&pid_file, &sock_file)?;
        println!("daemon was not running (cleaned up stale PID file)");
        return Ok(());
    }

    // Send stop command over the socket.
    if sock_file.exists() {
        if let Ok(mut stream) =
            std::os::unix::net::UnixStream::connect(&sock_file)
        {
            let _ = writeln!(stream, r#"{{"type":"control","command":"stop"}}"#);
        }
    }

    // Poll for the PID file to disappear (up to 5 seconds).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if !pid_file.exists() || !pid::is_process_alive(daemon_pid) {
            // Clean up any leftover files.
            let _ = std::fs::remove_file(&pid_file);
            let _ = std::fs::remove_file(&sock_file);
            return Ok(());
        }

        if std::time::Instant::now() >= deadline {
            break;
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    // Fallback: send SIGTERM.
    tracing::warn!(
        "daemon (PID {}) did not stop gracefully, sending SIGTERM",
        daemon_pid
    );
    unsafe {
        libc::kill(daemon_pid, libc::SIGTERM);
    }

    // Wait a bit more for the process to die.
    std::thread::sleep(Duration::from_millis(500));
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&sock_file);

    Ok(())
}

/// Query the status of a running daemon.
pub fn daemon_status(project_path: &std::path::Path) -> Result<String> {
    let pid_file = pid_path(project_path);
    let sock_file = sock_path(project_path);

    if !pid_file.exists() {
        anyhow::bail!("no daemon running (PID file not found)");
    }

    let (daemon_pid, session_id) = pid::read_pid_file(&pid_file)?;

    if !pid::is_process_alive(daemon_pid) {
        pid::cleanup_stale(&pid_file, &sock_file)?;
        anyhow::bail!("daemon was not running (cleaned up stale PID file)");
    }

    // Try to get detailed status from the socket.
    if sock_file.exists() {
        if let Ok(mut stream) =
            std::os::unix::net::UnixStream::connect(&sock_file)
        {
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .ok();
            let _ = writeln!(stream, r#"{{"type":"control","command":"status"}}"#);

            let mut response = String::new();
            if std::io::BufRead::read_line(
                &mut std::io::BufReader::new(&stream),
                &mut response,
            )
            .is_ok()
                && !response.trim().is_empty()
            {
                return Ok(response.trim().to_string());
            }
        }
    }

    // Fallback: return basic info from PID file.
    Ok(format!(
        r#"{{"type":"status","pid":{},"session_id":"{}"}}"#,
        daemon_pid, session_id
    ))
}
