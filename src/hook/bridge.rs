use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use crate::event::BusEvent;

/// Bridges incoming Claude Code hook payloads to the internal `BusEvent` channel.
pub struct HookBridge {
    socket_path: PathBuf,
    tx: Sender<BusEvent>,
}

impl HookBridge {
    /// Create a new `HookBridge`.
    pub fn new(socket_path: PathBuf, tx: Sender<BusEvent>) -> Self {
        Self { socket_path, tx }
    }

    /// Return the path to the Unix socket.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Bind a Unix domain socket, spawn a listener thread, and return its handle.
    ///
    /// Consumes `self`. Socket cleanup must be handled by the caller (or via
    /// `cleanup_socket`).
    pub fn start(self) -> Result<JoinHandle<()>> {
        // Remove a stale socket file if present.
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)
                .with_context(|| format!("remove stale socket {:?}", self.socket_path))?;
        }

        let listener = UnixListener::bind(&self.socket_path)
            .with_context(|| format!("bind Unix socket {:?}", self.socket_path))?;

        let tx = self.tx;
        // Clone the path for use inside the thread (the original is consumed by self).
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let reader = BufReader::new(stream);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => {
                                    if let Some(event) = Self::parse_payload(&l) {
                                        let _ = tx.send(event);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(handle)
    }

    /// Parse a raw hook payload line into a `BusEvent::HookEnrichment`.
    ///
    /// Expected format: `<TOOL_NAME> <JSON_INPUT>`
    /// Extracts `file_path` from the JSON input if present.
    pub fn parse_payload(line: &str) -> Option<BusEvent> {
        let mut iter = line.splitn(2, ' ');
        let tool_name = iter.next()?.to_string();
        let json_str = iter.next().unwrap_or("{}");

        let json: serde_json::Value = serde_json::from_str(json_str).unwrap_or(serde_json::json!({}));

        let file_path = json
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(BusEvent::HookEnrichment {
            file: file_path.unwrap_or_default(),
            tool: tool_name,
            intent: None,
        })
    }
}

/// Remove the socket file at `path` if it exists.
pub fn cleanup_socket(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_parse_payload_with_file_path() {
        let line = r#"Write {"file_path": "src/main.rs", "content": "hello"}"#;
        let event = HookBridge::parse_payload(line).unwrap();
        match event {
            BusEvent::HookEnrichment { file, tool, .. } => {
                assert_eq!(tool, "Write");
                assert_eq!(file, "src/main.rs");
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn test_parse_payload_without_file_path() {
        let line = r#"Bash {"command": "ls"}"#;
        let event = HookBridge::parse_payload(line).unwrap();
        match event {
            BusEvent::HookEnrichment { file, tool, .. } => {
                assert_eq!(tool, "Bash");
                assert_eq!(file, "");
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
