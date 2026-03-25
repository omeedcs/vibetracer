use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc;

use anyhow::{Context, Result};

use super::correlation::HookPayload;

/// Messages sent from the socket listener thread to the daemon main loop.
#[derive(Debug)]
pub enum SocketMessage {
    /// A hook enrichment from an agent (payload + relative filename).
    Hook(HookPayload, String),
    /// Notification that a restore operation is starting.
    RestoreStart { restore_id: u64, files: Vec<String> },
    /// Notification that a restore operation has completed.
    RestoreEnd { restore_id: u64 },
    /// A status query -- the stream is passed so the daemon can write back.
    StatusQuery(UnixStream),
    /// A request to shut down the daemon.
    Stop,
}

/// Start the Unix socket listener.
///
/// Binds a `UnixListener` at `sock_path`, then spawns a thread for each
/// incoming connection. Messages are parsed from newline-delimited JSON and
/// dispatched to the main loop via `tx`.
///
/// This function blocks forever (intended to run in a dedicated thread). It
/// returns `Ok(())` only if the listener socket is closed externally or an
/// unrecoverable bind error occurs.
pub fn listen(sock_path: &Path, tx: mpsc::Sender<SocketMessage>) -> Result<()> {
    // Remove any leftover socket file from a previous run.
    if sock_path.exists() {
        std::fs::remove_file(sock_path)
            .with_context(|| format!("remove stale socket {:?}", sock_path))?;
    }

    let listener = UnixListener::bind(sock_path)
        .with_context(|| format!("bind Unix socket at {:?}", sock_path))?;

    // Accept connections in a loop. Each connection is handled in its own thread.
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &tx) {
                        tracing::warn!("socket connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                // If the socket has been removed (shutdown), break out.
                if !sock_path.exists() {
                    break;
                }
                tracing::warn!("accept error on daemon socket: {}", e);
            }
        }
    }

    Ok(())
}

/// Handle a single client connection, reading newline-delimited JSON messages.
fn handle_connection(stream: UnixStream, tx: &mpsc::Sender<SocketMessage>) -> Result<()> {
    let reader = BufReader::new(stream.try_clone().context("clone stream for reading")?);

    for line in reader.lines() {
        let line = line.context("read line from socket")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_message(trimmed, stream.try_clone().ok()) {
            Ok(msg) => {
                if tx.send(msg).is_err() {
                    // Main loop has shut down; stop reading.
                    break;
                }
            }
            Err(e) => {
                tracing::warn!("invalid socket message: {}: {:?}", e, trimmed);
                // Send an error response if possible.
                if let Ok(mut s) = stream.try_clone() {
                    let _ = writeln!(s, r#"{{"error":"{}"}}"#, e);
                }
            }
        }
    }

    Ok(())
}

/// Parse a single JSON message string into a `SocketMessage`.
fn parse_message(json_str: &str, stream: Option<UnixStream>) -> Result<SocketMessage> {
    let value: serde_json::Value = serde_json::from_str(json_str).context("invalid JSON")?;

    let msg_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .context("missing 'type' field")?;

    match msg_type {
        "hook" => {
            let agent_id = value
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let operation_id = value
                .get("operation_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_name = value
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let file = value
                .get("file")
                .and_then(|v| v.as_str())
                .context("hook message missing 'file' field")?
                .to_string();
            let intent = value
                .get("intent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            Ok(SocketMessage::Hook(
                HookPayload {
                    agent_id,
                    operation_id,
                    tool_name,
                    intent,
                },
                file,
            ))
        }

        "control" => {
            let command = value
                .get("command")
                .and_then(|v| v.as_str())
                .context("control message missing 'command' field")?;
            match command {
                "stop" => Ok(SocketMessage::Stop),
                "status" => {
                    let stream = stream.context("no stream available for status response")?;
                    Ok(SocketMessage::StatusQuery(stream))
                }
                other => anyhow::bail!("unknown control command: {}", other),
            }
        }

        "restore_start" => {
            let restore_id = value
                .get("restore_id")
                .and_then(|v| v.as_u64())
                .context("restore_start missing 'restore_id'")?;
            let files: Vec<String> = value
                .get("files")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Ok(SocketMessage::RestoreStart { restore_id, files })
        }

        "restore_end" => {
            let restore_id = value
                .get("restore_id")
                .and_then(|v| v.as_u64())
                .context("restore_end missing 'restore_id'")?;
            Ok(SocketMessage::RestoreEnd { restore_id })
        }

        other => anyhow::bail!("unknown message type: {}", other),
    }
}

// ---- unit tests ---------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hook_message() {
        let json = r#"{"type":"hook","agent_id":"abc","operation_id":"op-1","tool_name":"Edit","file":"src/main.rs","intent":"fix bug"}"#;
        let msg = parse_message(json, None).unwrap();
        match msg {
            SocketMessage::Hook(payload, file) => {
                assert_eq!(payload.agent_id, "abc");
                assert_eq!(payload.operation_id, "op-1");
                assert_eq!(payload.tool_name, "Edit");
                assert_eq!(file, "src/main.rs");
                assert_eq!(payload.intent, Some("fix bug".to_string()));
            }
            _ => panic!("expected Hook message"),
        }
    }

    #[test]
    fn parse_hook_message_missing_optional_fields() {
        let json = r#"{"type":"hook","file":"a.rs"}"#;
        let msg = parse_message(json, None).unwrap();
        match msg {
            SocketMessage::Hook(payload, file) => {
                assert_eq!(payload.agent_id, "");
                assert_eq!(file, "a.rs");
                assert!(payload.intent.is_none());
            }
            _ => panic!("expected Hook message"),
        }
    }

    #[test]
    fn parse_control_stop() {
        let json = r#"{"type":"control","command":"stop"}"#;
        let msg = parse_message(json, None).unwrap();
        assert!(matches!(msg, SocketMessage::Stop));
    }

    #[test]
    fn parse_restore_start() {
        let json = r#"{"type":"restore_start","restore_id":42,"files":["src/a.rs","src/b.rs"]}"#;
        let msg = parse_message(json, None).unwrap();
        match msg {
            SocketMessage::RestoreStart { restore_id, files } => {
                assert_eq!(restore_id, 42);
                assert_eq!(files, vec!["src/a.rs", "src/b.rs"]);
            }
            _ => panic!("expected RestoreStart"),
        }
    }

    #[test]
    fn parse_restore_end() {
        let json = r#"{"type":"restore_end","restore_id":42}"#;
        let msg = parse_message(json, None).unwrap();
        match msg {
            SocketMessage::RestoreEnd { restore_id } => {
                assert_eq!(restore_id, 42);
            }
            _ => panic!("expected RestoreEnd"),
        }
    }

    #[test]
    fn parse_unknown_type_errors() {
        let json = r#"{"type":"bogus"}"#;
        assert!(parse_message(json, None).is_err());
    }

    #[test]
    fn parse_invalid_json_errors() {
        assert!(parse_message("{not json", None).is_err());
    }

    #[test]
    fn parse_hook_missing_file_errors() {
        let json = r#"{"type":"hook","agent_id":"abc"}"#;
        assert!(parse_message(json, None).is_err());
    }

    #[test]
    fn listen_and_send_messages() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let sock_path_clone = sock_path.clone();

        let (tx, rx) = mpsc::channel();

        // Start the listener in a background thread.
        let _listener_handle = std::thread::spawn(move || {
            let _ = listen(&sock_path_clone, tx);
        });

        // Give the listener a moment to bind.
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Connect and send a hook message.
        let mut stream = UnixStream::connect(&sock_path).unwrap();
        writeln!(
            stream,
            r#"{{"type":"hook","agent_id":"test","operation_id":"op-1","tool_name":"Edit","file":"main.rs"}}"#
        )
        .unwrap();

        // Send a stop message.
        writeln!(stream, r#"{{"type":"control","command":"stop"}}"#).unwrap();
        drop(stream);

        // Read messages from the channel.
        let msg1 = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert!(matches!(msg1, SocketMessage::Hook(_, _)));

        let msg2 = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert!(matches!(msg2, SocketMessage::Stop));

        // Clean up: remove the socket and connect once more to unblock
        // the listener's blocking accept() call so the thread can exit.
        let _ = std::fs::remove_file(&sock_path);
        // The listener thread will terminate once the tempdir is dropped
        // and the socket is gone. We don't join it to avoid hanging.
    }
}
