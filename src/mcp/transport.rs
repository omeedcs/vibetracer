use std::io::{BufRead, BufReader, Read, Write};

use anyhow::{Context, Result};

use super::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

// ─── StdioReader ─────────────────────────────────────────────────────────────

/// Reads newline-delimited JSON-RPC messages from a byte stream.
pub struct StdioReader {
    reader: BufReader<Box<dyn Read + Send>>,
}

impl StdioReader {
    pub fn new(input: Box<dyn Read + Send>) -> Self {
        Self {
            reader: BufReader::new(input),
        }
    }

    /// Reads one newline-delimited JSON-RPC request.
    ///
    /// Returns `Ok(None)` on EOF.  Blank lines are silently skipped.
    pub fn read_message(&mut self) -> Result<Option<JsonRpcRequest>> {
        loop {
            let mut line = String::new();
            let bytes_read = self
                .reader
                .read_line(&mut line)
                .context("failed to read from stdin")?;
            if bytes_read == 0 {
                return Ok(None); // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue; // skip blank lines
            }
            let request: JsonRpcRequest =
                serde_json::from_str(trimmed).context("failed to parse JSON-RPC request")?;
            return Ok(Some(request));
        }
    }
}

// ─── StdioWriter ─────────────────────────────────────────────────────────────

/// Writes newline-delimited JSON-RPC messages to a byte stream.
pub struct StdioWriter {
    writer: Box<dyn Write + Send>,
}

impl StdioWriter {
    pub fn new(output: Box<dyn Write + Send>) -> Self {
        Self { writer: output }
    }

    /// Serializes a JSON-RPC response as one JSON line, then flushes.
    pub fn write_message(&mut self, response: &JsonRpcResponse) -> Result<()> {
        let json = serde_json::to_string(response).context("failed to serialize response")?;
        writeln!(self.writer, "{}", json).context("failed to write response")?;
        self.writer
            .flush()
            .context("failed to flush after response")
    }

    /// Serializes a JSON-RPC notification as one JSON line, then flushes.
    pub fn write_notification(&mut self, notification: &JsonRpcNotification) -> Result<()> {
        let json =
            serde_json::to_string(notification).context("failed to serialize notification")?;
        writeln!(self.writer, "{}", json).context("failed to write notification")?;
        self.writer
            .flush()
            .context("failed to flush after notification")
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    /// A shared buffer that implements `Write + Send` so we can inspect
    /// the output after the `StdioWriter` has written to it.
    #[derive(Clone)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl SharedBuf {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Vec::new())))
        }
        fn to_string(&self) -> String {
            let bytes = self.0.lock().unwrap();
            String::from_utf8(bytes.clone()).unwrap()
        }
    }

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // ── StdioReader tests ───────────────────────────────────────────────────

    #[test]
    fn test_read_single_message() {
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let cursor = Cursor::new(format!("{}\n", input));
        let mut reader = StdioReader::new(Box::new(cursor));

        let msg = reader
            .read_message()
            .unwrap()
            .expect("should read a message");
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id, Some(json!(1)));
        assert_eq!(msg.method, "initialize");
        assert_eq!(msg.params, Some(json!({})));
    }

    #[test]
    fn test_read_eof_returns_none() {
        let cursor = Cursor::new(Vec::<u8>::new());
        let mut reader = StdioReader::new(Box::new(cursor));

        let msg = reader.read_message().unwrap();
        assert!(msg.is_none(), "EOF should return None");
    }

    #[test]
    fn test_read_multiple_messages() {
        let data = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_timeline"}}"#,
            "\n",
        );
        let cursor = Cursor::new(data);
        let mut reader = StdioReader::new(Box::new(cursor));

        let msg1 = reader.read_message().unwrap().expect("first message");
        assert_eq!(msg1.method, "tools/list");
        assert_eq!(msg1.id, Some(json!(1)));

        let msg2 = reader.read_message().unwrap().expect("second message");
        assert_eq!(msg2.method, "tools/call");
        assert_eq!(msg2.id, Some(json!(2)));
        assert_eq!(msg2.params, Some(json!({"name": "get_timeline"})));

        // After both messages, we should get EOF
        let msg3 = reader.read_message().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn test_read_skips_empty_lines() {
        let data = concat!(
            "\n",
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#,
            "\n",
            "\n",
        );
        let cursor = Cursor::new(data);
        let mut reader = StdioReader::new(Box::new(cursor));

        let msg = reader.read_message().unwrap().expect("should skip blanks");
        assert_eq!(msg.method, "ping");
        assert_eq!(msg.id, Some(json!(5)));
    }

    // ── StdioWriter tests ───────────────────────────────────────────────────

    #[test]
    fn test_write_response() {
        let buf = SharedBuf::new();
        let mut writer = StdioWriter::new(Box::new(buf.clone()));

        let response = JsonRpcResponse::success(json!(1), json!({"status": "ok"}));
        writer.write_message(&response).unwrap();

        let output = buf.to_string();
        // Output should be a single JSON line followed by newline
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["status"], "ok");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_write_notification() {
        let buf = SharedBuf::new();
        let mut writer = StdioWriter::new(Box::new(buf.clone()));

        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        writer.write_notification(&notif).unwrap();

        let output = buf.to_string();
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "notifications/initialized");
        assert!(parsed.get("params").is_none());
    }
}
