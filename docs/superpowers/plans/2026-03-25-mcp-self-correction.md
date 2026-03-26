# MCP Server & Self-Correction Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose vibetracer trace data via an MCP server (`vibetracer mcp`) so AI coding assistants can scrub through edit history to identify and fix their own mistakes.

**Architecture:** A new `src/mcp/` module implements a JSON-RPC 2.0 stdio server conforming to the MCP spec. It reads session directories directly (same as TUI), with a shared pagination module for streaming JSONL. The daemon gets a new `Subscribe` socket message type for live edit streaming. A Claude skill file at `skills/vibetracer-review.md` orchestrates the self-correction workflow.

**Tech Stack:** Rust (existing crate), serde_json (JSON-RPC), regex (search), similar (diffing), Unix sockets (streaming)

---

## File Structure

```
src/mcp/
├── mod.rs              # MCP server main loop, tool dispatch
├── transport.rs        # stdio JSON-RPC reader/writer
├── tools.rs            # Tool definitions (list, schemas)
├── handlers.rs         # Tool call handlers (business logic)
├── pagination.rs       # Shared JSONL streaming with offset/limit
├── streaming.rs        # Live edit subscription via daemon socket
├── types.rs            # MCP protocol types (requests, responses, errors)

src/daemon/
├── hook_listener.rs    # MODIFY: add Subscribe message type
├── mod.rs              # MODIFY: handle Subscribe, broadcast edits to subscribers

src/main.rs             # MODIFY: add Mcp subcommand
src/lib.rs              # MODIFY: add pub mod mcp

skills/
└── vibetracer-review.md  # Claude skill for self-correction workflow
```

---

### Task 1: MCP Protocol Types (`src/mcp/types.rs`)

**Files:**
- Create: `src/mcp/types.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test for JSON-RPC request deserialization**

```rust
// src/mcp/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_jsonrpc_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_sessions","arguments":{}}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Some(serde_json::Value::Number(1.into())));
        assert_eq!(req.method, "tools/call");
    }

    #[test]
    fn test_serialize_jsonrpc_response() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(1.into())),
            result: Some(serde_json::json!({"sessions": []})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"sessions\":[]"));
    }

    #[test]
    fn test_serialize_jsonrpc_error() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(1.into())),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "session not found".to_string(),
                data: None,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("-32602"));
        assert!(json.contains("session not found"));
    }

    #[test]
    fn test_deserialize_notification_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert!(req.id.is_none());
        assert_eq!(req.method, "notifications/initialized");
    }

    #[test]
    fn test_serialize_notification() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/tools/edit_event".to_string(),
            params: Some(serde_json::json!({"edit": {"id": 1}})),
        };
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("notifications/tools/edit_event"));
    }

    #[test]
    fn test_mcp_tool_definition_serialization() {
        let tool = McpToolDef {
            name: "list_sessions".to_string(),
            description: "List recorded trace sessions".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"}
                }
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("list_sessions"));
        assert!(json.contains("input_schema"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::types::tests -- --nocapture 2>&1 | head -20`
Expected: FAIL — module `mcp` does not exist yet

- [ ] **Step 3: Write the MCP protocol types**

```rust
// src/mcp/types.rs
use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request (also used for notifications when id is None).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 notification (no id field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// An MCP tool definition returned by tools/list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

// Standard JSON-RPC error codes.
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}
```

Also add to `src/mcp/mod.rs` (create it):
```rust
pub mod types;
```

And add to `src/lib.rs`:
```rust
pub mod mcp;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mcp::types::tests -- --nocapture`
Expected: all 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/types.rs src/mcp/mod.rs src/lib.rs
git commit -m "feat(mcp): add JSON-RPC 2.0 protocol types"
```

---

### Task 2: stdio Transport Layer (`src/mcp/transport.rs`)

**Files:**
- Create: `src/mcp/transport.rs`
- Modify: `src/mcp/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test for reading/writing JSON-RPC over byte streams**

```rust
// src/mcp/transport.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_single_message() {
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n";
        let mut reader = StdioReader::new(Box::new(Cursor::new(input.as_bytes().to_vec())));
        let msg = reader.read_message().unwrap();
        assert!(msg.is_some());
        let req = msg.unwrap();
        assert_eq!(req.method, "initialize");
    }

    #[test]
    fn test_read_eof_returns_none() {
        let mut reader = StdioReader::new(Box::new(Cursor::new(Vec::new())));
        let msg = reader.read_message().unwrap();
        assert!(msg.is_none());
    }

    #[test]
    fn test_write_response() {
        let mut output = Vec::new();
        {
            let mut writer = StdioWriter::new(Box::new(&mut output as &mut dyn std::io::Write));
            let resp = JsonRpcResponse::success(
                Some(serde_json::Value::Number(1.into())),
                serde_json::json!({"ok": true}),
            );
            writer.write_message(&resp).unwrap();
        }
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("\"jsonrpc\":\"2.0\""));
        assert!(written.ends_with('\n'));
    }

    #[test]
    fn test_read_multiple_messages() {
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"a\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"b\"}\n";
        let mut reader = StdioReader::new(Box::new(Cursor::new(input.as_bytes().to_vec())));
        let m1 = reader.read_message().unwrap().unwrap();
        let m2 = reader.read_message().unwrap().unwrap();
        assert_eq!(m1.method, "a");
        assert_eq!(m2.method, "b");
    }

    #[test]
    fn test_read_skips_empty_lines() {
        let input = "\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\"}\n\n";
        let mut reader = StdioReader::new(Box::new(Cursor::new(input.as_bytes().to_vec())));
        let msg = reader.read_message().unwrap().unwrap();
        assert_eq!(msg.method, "test");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::transport::tests -- --nocapture 2>&1 | head -20`
Expected: FAIL — `StdioReader` and `StdioWriter` don't exist

- [ ] **Step 3: Write the transport implementation**

```rust
// src/mcp/transport.rs
use std::io::{BufRead, BufReader, Read, Write};

use anyhow::{Context, Result};

use super::types::{JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};

/// Reads newline-delimited JSON-RPC messages from an input stream.
pub struct StdioReader {
    reader: BufReader<Box<dyn Read + Send>>,
}

impl StdioReader {
    pub fn new(input: Box<dyn Read + Send>) -> Self {
        Self {
            reader: BufReader::new(input),
        }
    }

    /// Read the next JSON-RPC request. Returns None on EOF.
    pub fn read_message(&mut self) -> Result<Option<JsonRpcRequest>> {
        loop {
            let mut line = String::new();
            let bytes_read = self.reader.read_line(&mut line).context("read from stdin")?;
            if bytes_read == 0 {
                return Ok(None); // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue; // skip blank lines
            }
            let req: JsonRpcRequest =
                serde_json::from_str(trimmed).context("parse JSON-RPC request")?;
            return Ok(Some(req));
        }
    }
}

/// Writes newline-delimited JSON-RPC messages to an output stream.
pub struct StdioWriter {
    writer: Box<dyn Write + Send>,
}

impl StdioWriter {
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self { writer }
    }

    /// Write a JSON-RPC response as a single line.
    pub fn write_message(&mut self, response: &JsonRpcResponse) -> Result<()> {
        let json = serde_json::to_string(response).context("serialize response")?;
        writeln!(self.writer, "{}", json).context("write to stdout")?;
        self.writer.flush().context("flush stdout")?;
        Ok(())
    }

    /// Write a JSON-RPC notification as a single line.
    pub fn write_notification(&mut self, notification: &JsonRpcNotification) -> Result<()> {
        let json = serde_json::to_string(notification).context("serialize notification")?;
        writeln!(self.writer, "{}", json).context("write to stdout")?;
        self.writer.flush().context("flush stdout")?;
        Ok(())
    }
}
```

Update `src/mcp/mod.rs`:
```rust
pub mod transport;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mcp::transport::tests -- --nocapture`
Expected: all 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/transport.rs src/mcp/mod.rs
git commit -m "feat(mcp): add stdio JSON-RPC transport layer"
```

---

### Task 3: Pagination Module (`src/mcp/pagination.rs`)

**Files:**
- Create: `src/mcp/pagination.rs`
- Modify: `src/mcp/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing tests**

```rust
// src/mcp/pagination.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EditEvent, EditKind};
    use tempfile::tempdir;

    fn write_test_edits(path: &std::path::Path, count: u32) {
        let mut file = std::fs::File::create(path).unwrap();
        for i in 1..=count {
            let event = EditEvent {
                id: i as u64,
                ts: 1_700_000_000_000 + (i as i64 * 1000),
                file: format!("src/file_{}.rs", i % 3),
                kind: EditKind::Modify,
                patch: format!("@@ -1 +1 @@\n-old{}\n+new{}", i, i),
                before_hash: Some(format!("before_{}", i)),
                after_hash: format!("after_{}", i),
                intent: Some(format!("edit {}", i)),
                tool: None,
                lines_added: 1,
                lines_removed: 1,
                agent_id: None,
                agent_label: None,
                operation_id: None,
                operation_intent: None,
                tool_name: None,
                restore_id: None,
            };
            let json = serde_json::to_string(&event).unwrap();
            use std::io::Write;
            writeln!(file, "{}", json).unwrap();
        }
    }

    #[test]
    fn test_read_all_edits_default_pagination() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        write_test_edits(&path, 5);

        let params = PageParams::default();
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 5);
        assert_eq!(result.total_count, 5);
    }

    #[test]
    fn test_pagination_with_offset_and_limit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        write_test_edits(&path, 10);

        let params = PageParams { offset: 3, limit: 2 };
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].id, 4); // 0-indexed offset: skip first 3
        assert_eq!(result.events[1].id, 5);
        assert_eq!(result.total_count, 10);
    }

    #[test]
    fn test_pagination_offset_beyond_total() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        write_test_edits(&path, 3);

        let params = PageParams { offset: 10, limit: 100 };
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 0);
        assert_eq!(result.total_count, 3);
    }

    #[test]
    fn test_pagination_with_filter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        write_test_edits(&path, 9); // files: file_1, file_2, file_0, file_1, file_2, file_0, ...

        let params = PageParams { offset: 0, limit: 100 };
        let filter = |e: &EditEvent| e.file == "src/file_1.rs";
        let result = read_edits_paged(&path, &params, Some(&filter)).unwrap();
        assert_eq!(result.total_count, 3); // ids 1, 4, 7
        assert_eq!(result.events.len(), 3);
        assert!(result.events.iter().all(|e| e.file == "src/file_1.rs"));
    }

    #[test]
    fn test_limit_clamped_to_max() {
        let params = PageParams { offset: 0, limit: 5000 };
        assert_eq!(params.effective_limit(), 1000);
    }

    #[test]
    fn test_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        std::fs::write(&path, "").unwrap();

        let params = PageParams::default();
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 0);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_skips_malformed_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        write_test_edits(&path, 2);
        // Append a malformed line
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "{{broken json").unwrap();

        let params = PageParams::default();
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.total_count, 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::pagination::tests -- --nocapture 2>&1 | head -20`
Expected: FAIL — `PageParams`, `read_edits_paged` don't exist

- [ ] **Step 3: Write the pagination implementation**

```rust
// src/mcp/pagination.rs
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};

use crate::event::EditEvent;

/// Pagination parameters for JSONL streaming.
pub struct PageParams {
    /// Number of matching entries to skip.
    pub offset: u32,
    /// Maximum number of entries to return.
    pub limit: u32,
}

const MAX_LIMIT: u32 = 1000;
const DEFAULT_LIMIT: u32 = 100;

impl Default for PageParams {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

impl PageParams {
    /// Return the effective limit, clamped to MAX_LIMIT.
    pub fn effective_limit(&self) -> u32 {
        self.limit.min(MAX_LIMIT)
    }
}

/// Result of a paginated read.
pub struct PageResult {
    /// The matching events in this page.
    pub events: Vec<EditEvent>,
    /// Total number of matching events (across all pages).
    pub total_count: u32,
}

/// Read edits from a JSONL file with pagination and optional filtering.
///
/// Streams line-by-line — never loads the full file into memory.
/// Malformed lines are silently skipped.
pub fn read_edits_paged(
    jsonl_path: &Path,
    params: &PageParams,
    filter: Option<&dyn Fn(&EditEvent) -> bool>,
) -> Result<PageResult> {
    let file =
        std::fs::File::open(jsonl_path).with_context(|| format!("open {:?}", jsonl_path))?;
    let reader = BufReader::new(file);

    let effective_limit = params.effective_limit();
    let mut events = Vec::new();
    let mut total_count: u32 = 0;
    let mut matched_so_far: u32 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event: EditEvent = match serde_json::from_str(trimmed) {
            Ok(e) => e,
            Err(_) => continue, // skip malformed lines
        };

        // Apply filter if provided.
        if let Some(f) = filter {
            if !f(&event) {
                continue;
            }
        }

        total_count += 1;

        // Pagination: skip offset, collect up to limit.
        if matched_so_far >= params.offset && events.len() < effective_limit as usize {
            events.push(event);
        }
        matched_so_far += 1;
    }

    Ok(PageResult {
        events,
        total_count,
    })
}
```

Update `src/mcp/mod.rs`:
```rust
pub mod pagination;
pub mod transport;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mcp::pagination::tests -- --nocapture`
Expected: all 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/pagination.rs src/mcp/mod.rs
git commit -m "feat(mcp): add paginated JSONL reader"
```

---

### Task 4: Tool Definitions (`src/mcp/tools.rs`)

**Files:**
- Create: `src/mcp/tools.rs`
- Modify: `src/mcp/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

```rust
// src/mcp/tools.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_valid_schemas() {
        let tools = all_tool_definitions();
        assert_eq!(tools.len(), 7);

        // Every tool must have a name, description, and valid input_schema
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn test_tool_names() {
        let tools = all_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"list_sessions"));
        assert!(names.contains(&"get_timeline"));
        assert!(names.contains(&"get_frame"));
        assert!(names.contains(&"diff_frames"));
        assert!(names.contains(&"search_edits"));
        assert!(names.contains(&"get_regression_window"));
        assert!(names.contains(&"subscribe_edits"));
    }

    #[test]
    fn test_list_sessions_schema_has_pagination() {
        let tools = all_tool_definitions();
        let tool = tools.iter().find(|t| t.name == "list_sessions").unwrap();
        let props = &tool.input_schema["properties"];
        assert!(props.get("limit").is_some());
        assert!(props.get("offset").is_some());
    }

    #[test]
    fn test_get_frame_schema_requires_session_id_and_frame_id() {
        let tools = all_tool_definitions();
        let tool = tools.iter().find(|t| t.name == "get_frame").unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"session_id"));
        assert!(required_strs.contains(&"frame_id"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::tools::tests -- --nocapture 2>&1 | head -20`
Expected: FAIL — `all_tool_definitions` doesn't exist

- [ ] **Step 3: Write the tool definitions**

```rust
// src/mcp/tools.rs
use super::types::McpToolDef;

/// Return all MCP tool definitions.
pub fn all_tool_definitions() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "list_sessions".to_string(),
            description: "List recorded vibetracer trace sessions with metadata".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of sessions to return (default 100, max 1000)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of sessions to skip"
                    }
                }
            }),
        },
        McpToolDef {
            name: "get_timeline".to_string(),
            description: "Get the edit timeline for a session — files changed, timestamps, diffs".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from list_sessions"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of edits to return (default 100, max 1000)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of edits to skip"
                    },
                    "file_filter": {
                        "type": "string",
                        "description": "Glob pattern to filter by file path (e.g. 'src/*.rs')"
                    }
                },
                "required": ["session_id"]
            }),
        },
        McpToolDef {
            name: "get_frame".to_string(),
            description: "Get the exact state of files at a specific point in the edit timeline".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from list_sessions"
                    },
                    "frame_id": {
                        "type": "integer",
                        "description": "Edit ID (frame number) to get state at"
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional: return only this file's content"
                    }
                },
                "required": ["session_id", "frame_id"]
            }),
        },
        McpToolDef {
            name: "diff_frames".to_string(),
            description: "Get unified diff between any two points in the edit timeline".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from list_sessions"
                    },
                    "frame_a": {
                        "type": "integer",
                        "description": "First frame (edit ID) to diff from"
                    },
                    "frame_b": {
                        "type": "integer",
                        "description": "Second frame (edit ID) to diff to"
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional: diff only this file"
                    }
                },
                "required": ["session_id", "frame_a", "frame_b"]
            }),
        },
        McpToolDef {
            name: "search_edits".to_string(),
            description: "Search for frames where a specific function, line, or pattern was modified".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from list_sessions"
                    },
                    "query": {
                        "type": "string",
                        "description": "Regex pattern to search for (falls back to literal if invalid regex). Matched against patch text, file path, and intent."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of matching edits to return (default 100, max 1000)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of matching edits to skip"
                    }
                },
                "required": ["session_id", "query"]
            }),
        },
        McpToolDef {
            name: "get_regression_window".to_string(),
            description: "Get candidate frames for bisecting a regression — the minimal set of edits to inspect".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from list_sessions"
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional: filter to edits touching this file"
                    },
                    "start_frame": {
                        "type": "integer",
                        "description": "Optional: only include edits at or after this frame"
                    },
                    "end_frame": {
                        "type": "integer",
                        "description": "Optional: only include edits at or before this frame"
                    }
                },
                "required": ["session_id"]
            }),
        },
        McpToolDef {
            name: "subscribe_edits".to_string(),
            description: "Subscribe to live edit notifications from the active recording session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID of the active recording session"
                    }
                },
                "required": ["session_id"]
            }),
        },
    ]
}
```

Update `src/mcp/mod.rs`:
```rust
pub mod pagination;
pub mod tools;
pub mod transport;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mcp::tools::tests -- --nocapture`
Expected: all 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/tools.rs src/mcp/mod.rs
git commit -m "feat(mcp): add MCP tool definitions for all 7 tools"
```

---

### Task 5: Tool Handlers (`src/mcp/handlers.rs`)

**Files:**
- Create: `src/mcp/handlers.rs`
- Modify: `src/mcp/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

This is the largest task. Each handler reads from the session directory.

- [ ] **Step 1: Write the failing tests for list_sessions and get_timeline**

```rust
// src/mcp/handlers.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EditKind;
    use tempfile::tempdir;

    /// Create a minimal session directory with meta.json and edits.jsonl.
    fn create_test_session(
        sessions_dir: &std::path::Path,
        session_id: &str,
        edit_count: u32,
    ) {
        let session_dir = sessions_dir.join(session_id);
        std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
        std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();

        // Write meta.json
        let meta = serde_json::json!({
            "id": session_id,
            "project_path": "/tmp/project",
            "started_at": 1700000000,
            "mode": "enriched",
            "agents": []
        });
        std::fs::write(
            session_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();

        // Write edits.jsonl
        let edits_path = session_dir.join("edits.jsonl");
        let mut file = std::fs::File::create(&edits_path).unwrap();
        for i in 1..=edit_count {
            let event = crate::event::EditEvent {
                id: i as u64,
                ts: 1_700_000_000_000 + (i as i64 * 1000),
                file: format!("src/file_{}.rs", i % 3),
                kind: EditKind::Modify,
                patch: format!("@@ -1 +1 @@\n-old{i}\n+new{i}"),
                before_hash: Some(format!("before_{i}")),
                after_hash: format!("after_{i}"),
                intent: Some(format!("edit {i}")),
                tool: None,
                lines_added: 1,
                lines_removed: 1,
                agent_id: None,
                agent_label: None,
                operation_id: None,
                operation_intent: None,
                tool_name: None,
                restore_id: None,
            };
            use std::io::Write;
            writeln!(file, "{}", serde_json::to_string(&event).unwrap()).unwrap();
        }
    }

    #[test]
    fn test_handle_list_sessions() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "20260325-120000-abcd", 5);
        create_test_session(&sessions_dir, "20260325-130000-efgh", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({});
        let result = ctx.handle_list_sessions(&args).unwrap();

        let sessions = result["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(result["total_count"], 2);
    }

    #[test]
    fn test_handle_get_timeline() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "test-session", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "test-session", "limit": 3, "offset": 2});
        let result = ctx.handle_get_timeline(&args).unwrap();

        let edits = result["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 3);
        assert_eq!(edits[0]["id"], 3);
        assert_eq!(result["total_count"], 10);
    }

    #[test]
    fn test_handle_get_timeline_with_file_filter() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "test-session", 9);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "test-session", "file_filter": "src/file_1.rs"});
        let result = ctx.handle_get_timeline(&args).unwrap();

        let edits = result["edits"].as_array().unwrap();
        assert_eq!(result["total_count"], 3);
        assert!(edits.iter().all(|e| e["file"] == "src/file_1.rs"));
    }

    #[test]
    fn test_handle_search_edits() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "test-session", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "test-session", "query": "new5"});
        let result = ctx.handle_search_edits(&args).unwrap();

        let edits = result["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["id"], 5);
    }

    #[test]
    fn test_handle_get_regression_window() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "test-session", 10);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({
            "session_id": "test-session",
            "file": "src/file_1.rs",
            "start_frame": 2,
            "end_frame": 8
        });
        let result = ctx.handle_get_regression_window(&args).unwrap();

        let frames = result["frames"].as_array().unwrap();
        // file_1.rs is edit ids 1, 4, 7 — within range 2..=8 that's 4 and 7
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["frame_id"], 4);
        assert_eq!(frames[1]["frame_id"], 7);
    }

    #[test]
    fn test_handle_get_frame() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // Create session with snapshots
        let session_dir = sessions_dir.join("test-session");
        std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
        std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();

        let store = crate::snapshot::store::SnapshotStore::new(session_dir.join("snapshots"));
        let hash1 = store.store(b"content at frame 1").unwrap();
        let hash2 = store.store(b"content at frame 2").unwrap();

        let meta = serde_json::json!({
            "id": "test-session",
            "project_path": "/tmp/project",
            "started_at": 1700000000,
            "mode": "enriched",
            "agents": []
        });
        std::fs::write(
            session_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        ).unwrap();

        // Write edits referencing snapshot hashes
        let edits_path = session_dir.join("edits.jsonl");
        let mut file = std::fs::File::create(&edits_path).unwrap();
        let events = vec![
            crate::event::EditEvent {
                id: 1, ts: 1000, file: "src/main.rs".to_string(),
                kind: EditKind::Create, patch: String::new(),
                before_hash: None, after_hash: hash1.clone(),
                intent: None, tool: None, lines_added: 1, lines_removed: 0,
                agent_id: None, agent_label: None, operation_id: None,
                operation_intent: None, tool_name: None, restore_id: None,
            },
            crate::event::EditEvent {
                id: 2, ts: 2000, file: "src/main.rs".to_string(),
                kind: EditKind::Modify, patch: String::new(),
                before_hash: Some(hash1), after_hash: hash2,
                intent: None, tool: None, lines_added: 1, lines_removed: 1,
                agent_id: None, agent_label: None, operation_id: None,
                operation_intent: None, tool_name: None, restore_id: None,
            },
        ];
        for event in &events {
            use std::io::Write;
            writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
        }

        let ctx = HandlerContext::new(sessions_dir);

        // Get frame at edit 1
        let args = serde_json::json!({"session_id": "test-session", "frame_id": 1});
        let result = ctx.handle_get_frame(&args).unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "src/main.rs");
        assert_eq!(files[0]["content"], "content at frame 1");

        // Get frame at edit 2
        let args = serde_json::json!({"session_id": "test-session", "frame_id": 2});
        let result = ctx.handle_get_frame(&args).unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files[0]["content"], "content at frame 2");
    }

    #[test]
    fn test_handle_diff_frames() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let session_dir = sessions_dir.join("test-session");
        std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
        std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();

        let store = crate::snapshot::store::SnapshotStore::new(session_dir.join("snapshots"));
        let hash1 = store.store(b"line1\n").unwrap();
        let hash2 = store.store(b"line1\nline2\n").unwrap();

        let meta = serde_json::json!({
            "id": "test-session", "project_path": "/tmp", "started_at": 0,
            "mode": "enriched", "agents": []
        });
        std::fs::write(session_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();

        let edits_path = session_dir.join("edits.jsonl");
        let mut file = std::fs::File::create(&edits_path).unwrap();
        let events = vec![
            crate::event::EditEvent {
                id: 1, ts: 1000, file: "a.rs".to_string(),
                kind: EditKind::Create, patch: String::new(),
                before_hash: None, after_hash: hash1,
                intent: None, tool: None, lines_added: 1, lines_removed: 0,
                agent_id: None, agent_label: None, operation_id: None,
                operation_intent: None, tool_name: None, restore_id: None,
            },
            crate::event::EditEvent {
                id: 2, ts: 2000, file: "a.rs".to_string(),
                kind: EditKind::Modify, patch: String::new(),
                before_hash: None, after_hash: hash2,
                intent: None, tool: None, lines_added: 1, lines_removed: 0,
                agent_id: None, agent_label: None, operation_id: None,
                operation_intent: None, tool_name: None, restore_id: None,
            },
        ];
        for event in &events {
            use std::io::Write;
            writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
        }

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "test-session", "frame_a": 1, "frame_b": 2});
        let result = ctx.handle_diff_frames(&args).unwrap();
        let diffs = result["diffs"].as_array().unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["path"], "a.rs");
        let diff_text = diffs[0]["diff"].as_str().unwrap();
        assert!(diff_text.contains("line2"));
    }

    #[test]
    fn test_session_not_found() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "nonexistent"});
        let result = ctx.handle_get_timeline(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_out_of_range() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        create_test_session(&sessions_dir, "test-session", 5);

        let ctx = HandlerContext::new(sessions_dir);
        let args = serde_json::json!({"session_id": "test-session", "frame_id": 99});
        let result = ctx.handle_get_frame(&args);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("5")); // should mention the actual count
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::handlers::tests -- --nocapture 2>&1 | head -20`
Expected: FAIL — `HandlerContext` doesn't exist

- [ ] **Step 3: Write the handler implementation**

```rust
// src/mcp/handlers.rs
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::event::EditEvent;
use crate::session::SessionManager;
use crate::snapshot::edit_log::EditLog;
use crate::snapshot::store::SnapshotStore;
use crate::watcher::differ::compute_diff;

use super::pagination::{read_edits_paged, PageParams};

/// Shared context for all tool handlers.
pub struct HandlerContext {
    sessions_dir: PathBuf,
}

impl HandlerContext {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    fn session_dir(&self, session_id: &str) -> Result<PathBuf> {
        let dir = self.sessions_dir.join(session_id);
        if !dir.exists() {
            anyhow::bail!("session '{}' not found", session_id);
        }
        Ok(dir)
    }

    fn edits_path(&self, session_id: &str) -> Result<PathBuf> {
        let path = self.session_dir(session_id)?.join("edits.jsonl");
        if !path.exists() {
            anyhow::bail!("no edit log found for session '{}'", session_id);
        }
        Ok(path)
    }

    fn snapshot_store(&self, session_id: &str) -> Result<SnapshotStore> {
        Ok(SnapshotStore::new(
            self.session_dir(session_id)?.join("snapshots"),
        ))
    }

    /// Extract pagination params from JSON arguments.
    fn page_params(args: &serde_json::Value) -> PageParams {
        PageParams {
            offset: args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            limit: args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as u32,
        }
    }

    // ── list_sessions ────────────────────────────────────────────────────────

    pub fn handle_list_sessions(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let manager = SessionManager::new(self.sessions_dir.clone());
        let all_metas = manager.list()?;

        let params = Self::page_params(args);
        let total_count = all_metas.len() as u32;
        let effective_limit = params.effective_limit() as usize;
        let offset = params.offset as usize;

        let page: Vec<serde_json::Value> = all_metas
            .into_iter()
            .skip(offset)
            .take(effective_limit)
            .map(|meta| {
                // Count edits from JSONL file
                let edit_count = self
                    .edits_path(&meta.id)
                    .ok()
                    .map(|p| EditLog::new(p).count().unwrap_or(0))
                    .unwrap_or(0);

                serde_json::json!({
                    "id": meta.id,
                    "project_path": meta.project_path,
                    "started_at": meta.started_at,
                    "mode": meta.mode,
                    "agent_count": meta.agents.len(),
                    "edit_count": edit_count,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "sessions": page,
            "total_count": total_count,
        }))
    }

    // ── get_timeline ─────────────────────────────────────────────────────────

    pub fn handle_get_timeline(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("missing session_id")?;
        let edits_path = self.edits_path(session_id)?;
        let params = Self::page_params(args);

        let file_filter = args.get("file_filter").and_then(|v| v.as_str()).map(|s| s.to_string());

        let filter: Option<Box<dyn Fn(&EditEvent) -> bool>> = file_filter.map(|pattern| {
            let glob = glob::Pattern::new(&pattern).ok();
            Box::new(move |e: &EditEvent| {
                if let Some(ref g) = glob {
                    g.matches(&e.file)
                } else {
                    e.file.contains(&pattern)
                }
            }) as Box<dyn Fn(&EditEvent) -> bool>
        });

        let result = read_edits_paged(
            &edits_path,
            &params,
            filter.as_ref().map(|f| f.as_ref()),
        )?;

        let edits: Vec<serde_json::Value> = result
            .events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "ts": e.ts,
                    "file": e.file,
                    "kind": e.kind,
                    "lines_added": e.lines_added,
                    "lines_removed": e.lines_removed,
                    "agent_label": e.agent_label,
                    "operation_id": e.operation_id,
                    "intent": e.intent,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "edits": edits,
            "total_count": result.total_count,
        }))
    }

    // ── get_frame ────────────────────────────────────────────────────────────

    pub fn handle_get_frame(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("missing session_id")?;
        let frame_id = args
            .get("frame_id")
            .and_then(|v| v.as_u64())
            .context("missing frame_id")?;
        let file_filter = args.get("file").and_then(|v| v.as_str());

        let edits_path = self.edits_path(session_id)?;
        let store = self.snapshot_store(session_id)?;

        // Read all edits up to frame_id.
        let all_edits = EditLog::read_all(&edits_path)?;
        let max_id = all_edits.last().map(|e| e.id).unwrap_or(0);

        if frame_id > max_id || frame_id == 0 {
            anyhow::bail!(
                "frame {} out of range (session has {} edits)",
                frame_id,
                max_id
            );
        }

        // Build file state at frame_id: for each file, find the last edit at or before frame_id.
        let mut file_state: HashMap<String, String> = HashMap::new(); // path -> after_hash

        for edit in &all_edits {
            if edit.id > frame_id {
                break;
            }
            if let Some(filter) = file_filter {
                if edit.file != filter {
                    continue;
                }
            }
            file_state.insert(edit.file.clone(), edit.after_hash.clone());
        }

        let mut files = Vec::new();
        for (path, hash) in &file_state {
            let content = store
                .retrieve(hash)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            files.push(serde_json::json!({
                "path": path,
                "content": content,
                "hash": hash,
            }));
        }

        // Sort by path for deterministic output.
        files.sort_by(|a, b| {
            a["path"]
                .as_str()
                .unwrap_or("")
                .cmp(b["path"].as_str().unwrap_or(""))
        });

        Ok(serde_json::json!({ "files": files }))
    }

    // ── diff_frames ──────────────────────────────────────────────────────────

    pub fn handle_diff_frames(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("missing session_id")?;
        let frame_a = args
            .get("frame_a")
            .and_then(|v| v.as_u64())
            .context("missing frame_a")?;
        let frame_b = args
            .get("frame_b")
            .and_then(|v| v.as_u64())
            .context("missing frame_b")?;
        let file_filter = args.get("file").and_then(|v| v.as_str());

        // Get file states at both frames.
        let state_a = self.handle_get_frame(&serde_json::json!({
            "session_id": session_id,
            "frame_id": frame_a,
            "file": file_filter,
        }))?;
        let state_b = self.handle_get_frame(&serde_json::json!({
            "session_id": session_id,
            "frame_id": frame_b,
            "file": file_filter,
        }))?;

        // Build maps: path -> content
        let map_a = Self::files_to_map(&state_a["files"]);
        let map_b = Self::files_to_map(&state_b["files"]);

        // Collect all file paths.
        let mut all_paths: Vec<String> = map_a.keys().chain(map_b.keys()).cloned().collect();
        all_paths.sort();
        all_paths.dedup();

        let mut diffs = Vec::new();
        for path in &all_paths {
            let content_a = map_a.get(path).map(|s| s.as_str()).unwrap_or("");
            let content_b = map_b.get(path).map(|s| s.as_str()).unwrap_or("");

            if content_a != content_b {
                let diff = compute_diff(content_a, content_b, path);
                diffs.push(serde_json::json!({
                    "path": path,
                    "diff": diff.patch,
                }));
            }
        }

        Ok(serde_json::json!({ "diffs": diffs }))
    }

    fn files_to_map(files_value: &serde_json::Value) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(arr) = files_value.as_array() {
            for f in arr {
                if let (Some(path), Some(content)) = (
                    f["path"].as_str(),
                    f["content"].as_str(),
                ) {
                    map.insert(path.to_string(), content.to_string());
                }
            }
        }
        map
    }

    // ── search_edits ─────────────────────────────────────────────────────────

    pub fn handle_search_edits(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("missing session_id")?;
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("missing query")?;
        let edits_path = self.edits_path(session_id)?;
        let params = Self::page_params(args);

        // Try to compile as regex, fall back to literal substring.
        let re = regex::Regex::new(query).ok();
        let query_owned = query.to_string();

        let filter = move |e: &EditEvent| {
            let fields = [
                e.patch.as_str(),
                e.file.as_str(),
                e.intent.as_deref().unwrap_or(""),
            ];
            if let Some(ref re) = re {
                fields.iter().any(|f| re.is_match(f))
            } else {
                fields.iter().any(|f| f.contains(&query_owned))
            }
        };

        let result = read_edits_paged(&edits_path, &params, Some(&filter))?;

        let edits: Vec<serde_json::Value> = result
            .events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "ts": e.ts,
                    "file": e.file,
                    "kind": e.kind,
                    "patch": e.patch,
                    "intent": e.intent,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "edits": edits,
            "total_count": result.total_count,
        }))
    }

    // ── get_regression_window ────────────────────────────────────────────────

    pub fn handle_get_regression_window(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .context("missing session_id")?;
        let file_filter = args.get("file").and_then(|v| v.as_str()).map(|s| s.to_string());
        let start_frame = args.get("start_frame").and_then(|v| v.as_u64());
        let end_frame = args.get("end_frame").and_then(|v| v.as_u64());

        let edits_path = self.edits_path(session_id)?;

        // Read all edits (no pagination — regression window needs full scan).
        let all_edits = EditLog::read_all(&edits_path)?;

        let frames: Vec<serde_json::Value> = all_edits
            .iter()
            .filter(|e| {
                if let Some(start) = start_frame {
                    if e.id < start {
                        return false;
                    }
                }
                if let Some(end) = end_frame {
                    if e.id > end {
                        return false;
                    }
                }
                if let Some(ref file) = file_filter {
                    if e.file != *file {
                        return false;
                    }
                }
                true
            })
            .map(|e| {
                serde_json::json!({
                    "frame_id": e.id,
                    "file": e.file,
                    "patch": e.patch,
                    "before_hash": e.before_hash,
                    "after_hash": e.after_hash,
                })
            })
            .collect();

        Ok(serde_json::json!({ "frames": frames }))
    }
}
```

Update `src/mcp/mod.rs`:
```rust
pub mod handlers;
pub mod pagination;
pub mod tools;
pub mod transport;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mcp::handlers::tests -- --nocapture`
Expected: all 10 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/handlers.rs src/mcp/mod.rs
git commit -m "feat(mcp): implement tool handlers for all 6 read-only tools"
```

---

### Task 6: MCP Server Main Loop (`src/mcp/mod.rs`)

**Files:**
- Modify: `src/mcp/mod.rs`
- Test: integration test `tests/integration/mcp_test.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
// tests/integration/mcp_test.rs
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Helper: start the MCP server as a child process with piped stdio.
fn start_mcp_server(project_dir: &std::path::Path) -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_vibetracer");
    Command::new(bin)
        .arg(project_dir.to_str().unwrap())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start vibetracer mcp")
}

/// Send a JSON-RPC request and read the response.
fn send_request(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    method: &str,
    id: u64,
    params: serde_json::Value,
) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[test]
fn test_mcp_initialize_and_list_tools() {
    let dir = tempfile::tempdir().unwrap();
    let vt_dir = dir.path().join(".vibetracer").join("sessions");
    std::fs::create_dir_all(&vt_dir).unwrap();

    let mut child = start_mcp_server(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize
    let resp = send_request(
        &mut stdin,
        &mut stdout,
        "initialize",
        1,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0.1"}
        }),
    );
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert!(resp["result"]["serverInfo"]["name"].as_str().unwrap().contains("vibetracer"));

    // Send initialized notification (no response expected, but we send tools/list next)
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .unwrap()
    )
    .unwrap();
    stdin.flush().unwrap();

    // List tools
    let resp = send_request(
        &mut stdin,
        &mut stdout,
        "tools/list",
        2,
        serde_json::json!({}),
    );
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);

    // Cleanup
    drop(stdin);
    child.wait().unwrap();
}

#[test]
fn test_mcp_tools_call_list_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join(".vibetracer").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // Create a test session
    let session_dir = sessions_dir.join("test-session-001");
    std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
    std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();
    std::fs::write(
        session_dir.join("meta.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "id": "test-session-001",
            "project_path": dir.path().to_str().unwrap(),
            "started_at": 1700000000,
            "mode": "enriched",
            "agents": []
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(session_dir.join("edits.jsonl"), "").unwrap();

    let mut child = start_mcp_server(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize
    send_request(
        &mut stdin,
        &mut stdout,
        "initialize",
        1,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0.1"}
        }),
    );

    // Call list_sessions
    let resp = send_request(
        &mut stdin,
        &mut stdout,
        "tools/call",
        2,
        serde_json::json!({
            "name": "list_sessions",
            "arguments": {}
        }),
    );

    let content = resp["result"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    let text: serde_json::Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["total_count"], 1);
    assert_eq!(text["sessions"][0]["id"], "test-session-001");

    drop(stdin);
    child.wait().unwrap();
}
```

Also add `mod mcp_test;` to `tests/integration/mod.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration_tests mcp_test -- --nocapture 2>&1 | head -20`
Expected: FAIL — `vibetracer mcp` subcommand doesn't exist yet

- [ ] **Step 3: Write the MCP server main loop**

```rust
// src/mcp/mod.rs — replace contents with:
pub mod handlers;
pub mod pagination;
pub mod streaming;
pub mod tools;
pub mod transport;
pub mod types;

use std::path::PathBuf;

use anyhow::{Context, Result};

use handlers::HandlerContext;
use tools::all_tool_definitions;
use transport::{StdioReader, StdioWriter};
use types::JsonRpcResponse;

/// Run the MCP server, reading JSON-RPC from stdin and writing to stdout.
pub fn run_mcp_server(project_path: PathBuf) -> Result<()> {
    let sessions_dir = project_path.join(".vibetracer").join("sessions");
    let ctx = HandlerContext::new(sessions_dir.clone());

    let mut reader = StdioReader::new(Box::new(std::io::stdin().lock()));
    let mut writer = StdioWriter::new(Box::new(std::io::stdout().lock()));

    loop {
        let request = match reader.read_message()? {
            Some(req) => req,
            None => break, // EOF — client disconnected
        };

        // Notifications (no id) don't get a response.
        if request.id.is_none() {
            // Handle notifications/initialized (no-op, just acknowledge).
            continue;
        }

        let id = request.id.clone();

        let response = match request.method.as_str() {
            "initialize" => handle_initialize(id),

            "tools/list" => {
                let tools = all_tool_definitions();
                JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
            }

            "tools/call" => {
                let params = request.params.unwrap_or(serde_json::json!({}));
                let tool_name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match dispatch_tool(&ctx, tool_name, &arguments, &project_path, &sessions_dir, &mut writer) {
                    Ok(result) => {
                        // MCP tools/call returns content array.
                        let content = serde_json::json!([{
                            "type": "text",
                            "text": serde_json::to_string(&result).unwrap_or_default(),
                        }]);
                        JsonRpcResponse::success(id, serde_json::json!({ "content": content }))
                    }
                    Err(e) => JsonRpcResponse::error(
                        id,
                        types::INVALID_PARAMS,
                        e.to_string(),
                    ),
                }
            }

            _ => JsonRpcResponse::error(
                id,
                types::METHOD_NOT_FOUND,
                format!("unknown method: {}", request.method),
            ),
        };

        writer.write_message(&response)?;
    }

    Ok(())
}

fn handle_initialize(id: Option<serde_json::Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "vibetracer-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )
}

fn dispatch_tool(
    ctx: &HandlerContext,
    tool_name: &str,
    arguments: &serde_json::Value,
    project_path: &PathBuf,
    sessions_dir: &PathBuf,
    writer: &mut StdioWriter,
) -> Result<serde_json::Value> {
    match tool_name {
        "list_sessions" => ctx.handle_list_sessions(arguments),
        "get_timeline" => ctx.handle_get_timeline(arguments),
        "get_frame" => ctx.handle_get_frame(arguments),
        "diff_frames" => ctx.handle_diff_frames(arguments),
        "search_edits" => ctx.handle_search_edits(arguments),
        "get_regression_window" => ctx.handle_get_regression_window(arguments),
        "subscribe_edits" => {
            streaming::handle_subscribe(arguments, project_path, sessions_dir, writer)
        }
        _ => anyhow::bail!("unknown tool: {}", tool_name),
    }
}
```

- [ ] **Step 4: Add the `Mcp` subcommand to `src/main.rs`**

Add to the `Commands` enum:
```rust
    /// Start MCP server (stdio JSON-RPC for AI coding assistants)
    Mcp,
```

Add the match arm in the `match cli.command` block (before `None =>`):
```rust
        Some(Commands::Mcp) => {
            let project_path = resolve_path(cli.path.as_deref())?;
            vibetracer::mcp::run_mcp_server(project_path)?;
        }
```

- [ ] **Step 5: Create a stub streaming module so it compiles**

```rust
// src/mcp/streaming.rs
use std::path::PathBuf;

use anyhow::Result;

use super::transport::StdioWriter;

/// Handle subscribe_edits tool call.
/// Stub — full implementation in Task 7.
pub fn handle_subscribe(
    _arguments: &serde_json::Value,
    _project_path: &PathBuf,
    _sessions_dir: &PathBuf,
    _writer: &mut StdioWriter,
) -> Result<serde_json::Value> {
    anyhow::bail!("subscribe_edits requires a running daemon — start one with `vibetracer daemon start`")
}
```

- [ ] **Step 6: Run the integration tests**

Run: `cargo test --test integration_tests mcp_test -- --nocapture`
Expected: all 2 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/mcp/mod.rs src/mcp/streaming.rs src/main.rs tests/integration/mcp_test.rs tests/integration/mod.rs
git commit -m "feat(mcp): implement MCP server main loop with tool dispatch"
```

---

### Task 7: Live Streaming via Daemon Socket (`src/mcp/streaming.rs` + daemon changes)

**Files:**
- Modify: `src/mcp/streaming.rs`
- Modify: `src/daemon/hook_listener.rs` — add `Subscribe` message type
- Modify: `src/daemon/mod.rs` — broadcast edits to subscribers
- Test: inline `#[cfg(test)] mod tests` in `hook_listener.rs` + `streaming.rs`

- [ ] **Step 1: Write the failing test for Subscribe message parsing in hook_listener**

Add to `src/daemon/hook_listener.rs` tests:
```rust
    #[test]
    fn parse_subscribe_message() {
        let json = r#"{"type":"subscribe","session_id":"test-session"}"#;
        let msg = parse_message(json, None).unwrap();
        match msg {
            SocketMessage::Subscribe { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("expected Subscribe message"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib daemon::hook_listener::tests::parse_subscribe_message -- --nocapture 2>&1 | head -20`
Expected: FAIL — no `Subscribe` variant on `SocketMessage`

- [ ] **Step 3: Add Subscribe to SocketMessage and parse_message**

In `src/daemon/hook_listener.rs`, add to the `SocketMessage` enum:
```rust
    /// A request to subscribe to live edit notifications.
    Subscribe {
        session_id: String,
        stream: Option<UnixStream>,
    },
```

Add to `parse_message` match block:
```rust
        "subscribe" => {
            let session_id = value
                .get("session_id")
                .and_then(|v| v.as_str())
                .context("subscribe message missing 'session_id'")?
                .to_string();
            Ok(SocketMessage::Subscribe {
                session_id,
                stream,
            })
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib daemon::hook_listener::tests::parse_subscribe_message -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add subscriber broadcasting to daemon main loop**

In `src/daemon/mod.rs`, add a subscribers list before the main loop:
```rust
    let mut subscribers: Vec<UnixStream> = Vec::new();
```

In the `SocketMessage::Subscribe` match arm (inside `7a. Drain socket messages`):
```rust
                SocketMessage::Subscribe { session_id, stream } => {
                    if session_id == session.id {
                        if let Some(s) = stream {
                            subscribers.push(s);
                        }
                    }
                }
```

After `edit_count += 1;` in the file change processing (step 7b), add:
```rust
                    // Broadcast to subscribers.
                    if !subscribers.is_empty() {
                        let notification = serde_json::json!({
                            "type": "edit_notification",
                            "event": _result.event,
                        });
                        let msg = format!("{}\n", serde_json::to_string(&notification).unwrap_or_default());
                        subscribers.retain(|s| {
                            let mut s_ref = s;
                            std::io::Write::write_all(&mut s_ref, msg.as_bytes()).is_ok()
                        });
                    }
```

(Note: need to rename `_result` to `result` in the `Ok(Some(_result))` pattern.)

- [ ] **Step 6: Implement full streaming handler in `src/mcp/streaming.rs`**

```rust
// src/mcp/streaming.rs
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::event::EditEvent;

use super::transport::StdioWriter;
use super::types::JsonRpcNotification;

/// Handle subscribe_edits tool call.
///
/// Connects to the daemon socket, sends a Subscribe message, then forwards
/// edit notifications as MCP notifications over stdio.
pub fn handle_subscribe(
    arguments: &serde_json::Value,
    project_path: &PathBuf,
    _sessions_dir: &PathBuf,
    writer: &mut StdioWriter,
) -> Result<serde_json::Value> {
    let session_id = arguments
        .get("session_id")
        .and_then(|v| v.as_str())
        .context("missing session_id")?;

    let sock_path = project_path.join(".vibetracer").join("daemon.sock");
    if !sock_path.exists() {
        anyhow::bail!(
            "daemon is not active — start with `vibetracer daemon start`"
        );
    }

    let mut stream = UnixStream::connect(&sock_path)
        .context("connect to daemon socket")?;

    // Send subscribe message.
    let subscribe_msg = serde_json::json!({
        "type": "subscribe",
        "session_id": session_id,
    });
    writeln!(stream, "{}", serde_json::to_string(&subscribe_msg)?)?;
    stream.flush()?;

    // Read notifications and forward as MCP notifications.
    // This blocks until the connection is closed.
    let reader = BufReader::new(stream);
    let mut buffer: Vec<serde_json::Value> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // connection closed
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if value.get("type").and_then(|v| v.as_str()) == Some("edit_notification") {
            let notification = JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "notifications/tools/edit_event".to_string(),
                params: Some(value["event"].clone()),
            };

            if writer.write_notification(&notification).is_err() {
                break; // MCP client disconnected
            }

            buffer.push(value["event"].clone());

            // Backpressure: if we've buffered too many, drop oldest
            if buffer.len() > 100 {
                buffer.drain(0..buffer.len() - 100);
            }
        }
    }

    Ok(serde_json::json!({
        "status": "subscription_ended",
        "events_forwarded": buffer.len(),
    }))
}
```

- [ ] **Step 7: Run all tests to verify nothing is broken**

Run: `cargo test -- --nocapture 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/mcp/streaming.rs src/daemon/hook_listener.rs src/daemon/mod.rs
git commit -m "feat(mcp): implement live edit streaming via daemon socket subscription"
```

---

### Task 8: Claude Skill File (`skills/vibetracer-review.md`)

**Files:**
- Create: `skills/vibetracer-review.md`

- [ ] **Step 1: Create the skills directory**

```bash
mkdir -p skills
```

- [ ] **Step 2: Write the skill file**

```markdown
---
name: vibetracer-review
description: Self-correction workflow — scrub through vibetracer edit history to identify and fix regressions introduced during AI-assisted coding
---

# vibetracer Self-Correction Review

Use this skill when tests fail or behavior regresses after a series of AI-assisted edits. It uses vibetracer's MCP tools to scrub through the edit timeline and surgically fix the regression at its source.

## Prerequisites

- vibetracer must be installed and recording (running as daemon or in TUI)
- The vibetracer MCP server must be configured:

```json
{
  "mcpServers": {
    "vibetracer": {
      "command": "vibetracer",
      "args": ["mcp"]
    }
  }
}
```

## Workflow

### Phase 1: Load Context

1. Call `list_sessions` to find the active or most recent session
2. Call `get_timeline` with the session ID to get the full edit history
3. Note the total number of edits, which files were touched, and the edit range

### Phase 2: Identify Scope

1. Group edits by `operation_id` to understand logical units of work
2. Group edits by file to see which files changed most
3. Identify the "before" state (frame 1 or the start of the current work)

### Phase 3: Run Verification

1. Run the project's test suite or build command
2. If everything passes, report success and stop
3. If there are failures, note the specific errors and failing tests

### Phase 4: Bisect the Regression

1. Call `get_regression_window` with the relevant file filter to narrow candidates
2. Start a binary search through the candidate frames:
   a. Pick the midpoint frame
   b. Call `get_frame` at that point to see the file state
   c. Use `diff_frames` to compare the midpoint against the known-good state
   d. Assess whether the regression-causing change is before or after this point
   e. Narrow the window and repeat
3. Once you identify the exact frame that introduced the issue, call `diff_frames` between it and the previous frame to see exactly what changed

### Phase 5: Fix Surgically

1. Call `get_frame` at the frame just before the regression to see the intended state
2. Understand what the edit was trying to do (check the `intent` field)
3. Write a targeted fix that preserves the intent but corrects the error
4. Do NOT revert the entire edit — fix the specific issue

### Phase 6: Verify Fix

1. Re-run the test suite to confirm the regression is fixed
2. Run `get_timeline` again to confirm your fix was recorded
3. Report what was found, what frame introduced it, and what was fixed

## Tips

- Use `search_edits` with a regex pattern to quickly find frames that touched a specific function or variable
- If multiple regressions exist, fix them one at a time, re-running tests after each
- Use `subscribe_edits` if you want live notifications as new edits are recorded
- The `file_filter` parameter on `get_timeline` is useful for narrowing to a specific file's history
```

- [ ] **Step 3: Commit**

```bash
git add skills/vibetracer-review.md
git commit -m "feat: add vibetracer-review Claude skill for AI self-correction"
```

---

### Task 9: Final Integration Test — Full Workflow

**Files:**
- Create: `tests/integration/mcp_workflow_test.rs`
- Modify: `tests/integration/mod.rs`

- [ ] **Step 1: Write the full workflow integration test**

```rust
// tests/integration/mcp_workflow_test.rs

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn start_mcp(project_dir: &std::path::Path) -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_vibetracer");
    Command::new(bin)
        .arg(project_dir.to_str().unwrap())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start vibetracer mcp")
}

fn send(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    method: &str,
    id: u64,
    params: serde_json::Value,
) -> serde_json::Value {
    let req = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    writeln!(stdin, "{}", serde_json::to_string(&req).unwrap()).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

fn parse_tool_result(resp: &serde_json::Value) -> serde_json::Value {
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

/// Create a session with realistic edits and snapshots.
fn create_realistic_session(sessions_dir: &std::path::Path) {
    let session_dir = sessions_dir.join("workflow-test");
    std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
    std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();

    let store = vibetracer::snapshot::store::SnapshotStore::new(session_dir.join("snapshots"));

    // Simulate 3 edits to main.rs
    let h1 = store.store(b"fn main() {}\n").unwrap();
    let h2 = store.store(b"fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    let h3 = store.store(b"fn main() {\n    println!(\"hello\");\n    broken_function();\n}\n").unwrap();

    let meta = serde_json::json!({
        "id": "workflow-test",
        "project_path": "/tmp",
        "started_at": 1700000000,
        "mode": "enriched",
        "agents": [{"agent_id":"a1","agent_label":"claude-1","tool_type":"claude-code","first_seen":1000,"last_seen":3000,"edit_count":3}]
    });
    std::fs::write(session_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let edits = vec![
        vibetracer::event::EditEvent {
            id: 1, ts: 1000, file: "src/main.rs".to_string(),
            kind: vibetracer::event::EditKind::Create,
            patch: "@@ -0,0 +1 @@\n+fn main() {}".to_string(),
            before_hash: None, after_hash: h1.clone(),
            intent: Some("scaffold main".to_string()), tool: None,
            lines_added: 1, lines_removed: 0,
            agent_id: Some("a1".to_string()), agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-1".to_string()), operation_intent: Some("initial setup".to_string()),
            tool_name: Some("Write".to_string()), restore_id: None,
        },
        vibetracer::event::EditEvent {
            id: 2, ts: 2000, file: "src/main.rs".to_string(),
            kind: vibetracer::event::EditKind::Modify,
            patch: "@@ -1 +1,3 @@\n-fn main() {}\n+fn main() {\n+    println!(\"hello\");\n+}".to_string(),
            before_hash: Some(h1), after_hash: h2.clone(),
            intent: Some("add greeting".to_string()), tool: None,
            lines_added: 3, lines_removed: 1,
            agent_id: Some("a1".to_string()), agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-2".to_string()), operation_intent: Some("add hello".to_string()),
            tool_name: Some("Edit".to_string()), restore_id: None,
        },
        vibetracer::event::EditEvent {
            id: 3, ts: 3000, file: "src/main.rs".to_string(),
            kind: vibetracer::event::EditKind::Modify,
            patch: "@@ -1,3 +1,4 @@\n fn main() {\n     println!(\"hello\");\n+    broken_function();\n }".to_string(),
            before_hash: Some(h2), after_hash: h3,
            intent: Some("add feature".to_string()), tool: None,
            lines_added: 1, lines_removed: 0,
            agent_id: Some("a1".to_string()), agent_label: Some("claude-1".to_string()),
            operation_id: Some("op-3".to_string()), operation_intent: Some("add feature".to_string()),
            tool_name: Some("Edit".to_string()), restore_id: None,
        },
    ];

    let edits_path = session_dir.join("edits.jsonl");
    let mut file = std::fs::File::create(&edits_path).unwrap();
    for e in &edits {
        writeln!(file, "{}", serde_json::to_string(e).unwrap()).unwrap();
    }
}

#[test]
fn test_full_self_correction_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join(".vibetracer").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    create_realistic_session(&sessions_dir);

    let mut child = start_mcp(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // 1. Initialize
    send(&mut stdin, &mut stdout, "initialize", 1,
        serde_json::json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}));

    // 2. List sessions
    let resp = send(&mut stdin, &mut stdout, "tools/call", 2,
        serde_json::json!({"name":"list_sessions","arguments":{}}));
    let result = parse_tool_result(&resp);
    assert_eq!(result["total_count"], 1);
    let session_id = result["sessions"][0]["id"].as_str().unwrap();
    assert_eq!(session_id, "workflow-test");

    // 3. Get timeline
    let resp = send(&mut stdin, &mut stdout, "tools/call", 3,
        serde_json::json!({"name":"get_timeline","arguments":{"session_id":"workflow-test"}}));
    let result = parse_tool_result(&resp);
    assert_eq!(result["total_count"], 3);

    // 4. Get regression window
    let resp = send(&mut stdin, &mut stdout, "tools/call", 4,
        serde_json::json!({"name":"get_regression_window","arguments":{"session_id":"workflow-test","file":"src/main.rs"}}));
    let result = parse_tool_result(&resp);
    assert_eq!(result["frames"].as_array().unwrap().len(), 3);

    // 5. Search for the broken function
    let resp = send(&mut stdin, &mut stdout, "tools/call", 5,
        serde_json::json!({"name":"search_edits","arguments":{"session_id":"workflow-test","query":"broken_function"}}));
    let result = parse_tool_result(&resp);
    assert_eq!(result["total_count"], 1);
    assert_eq!(result["edits"][0]["id"], 3);

    // 6. Get frame at edit 2 (last known good)
    let resp = send(&mut stdin, &mut stdout, "tools/call", 6,
        serde_json::json!({"name":"get_frame","arguments":{"session_id":"workflow-test","frame_id":2}}));
    let result = parse_tool_result(&resp);
    let content = result["files"][0]["content"].as_str().unwrap();
    assert!(content.contains("println"));
    assert!(!content.contains("broken_function"));

    // 7. Diff between frame 2 and 3 to see exactly what broke
    let resp = send(&mut stdin, &mut stdout, "tools/call", 7,
        serde_json::json!({"name":"diff_frames","arguments":{"session_id":"workflow-test","frame_a":2,"frame_b":3}}));
    let result = parse_tool_result(&resp);
    let diff = result["diffs"][0]["diff"].as_str().unwrap();
    assert!(diff.contains("broken_function"));

    drop(stdin);
    child.wait().unwrap();
}
```

Add `mod mcp_workflow_test;` to `tests/integration/mod.rs`.

- [ ] **Step 2: Run the workflow test**

Run: `cargo test --test integration_tests mcp_workflow_test -- --nocapture`
Expected: PASS

- [ ] **Step 3: Run the full test suite**

Run: `cargo test -- --nocapture 2>&1 | tail -30`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration/mcp_workflow_test.rs tests/integration/mod.rs
git commit -m "test: add full MCP self-correction workflow integration test"
```

---

### Task 10: Documentation and README Update

**Files:**
- Modify: `README.md` — add MCP section

- [ ] **Step 1: Read current README**

Read `README.md` to understand the existing structure.

- [ ] **Step 2: Add MCP documentation section**

Add a section after the existing CLI usage documentation:

```markdown
## MCP Server (AI Self-Correction)

vibetracer includes an MCP (Model Context Protocol) server that exposes trace data to AI coding assistants, enabling them to scrub through edit history and fix their own mistakes.

### Setup

Add to your `.claude.json` or MCP client configuration:

```json
{
  "mcpServers": {
    "vibetracer": {
      "command": "vibetracer",
      "args": ["mcp"]
    }
  }
}
```

### Available Tools

| Tool | Description |
|------|-------------|
| `list_sessions` | List recorded trace sessions |
| `get_timeline` | Get the edit timeline for a session |
| `get_frame` | Get file state at any point in the timeline |
| `diff_frames` | Diff between any two points |
| `search_edits` | Find frames where a pattern was modified |
| `get_regression_window` | Get candidate frames for bisecting a regression |
| `subscribe_edits` | Subscribe to live edit notifications |

### Claude Skill

Copy `skills/vibetracer-review.md` to your Claude skills directory to enable the self-correction workflow. When tests fail after a series of edits, the skill guides Claude through loading the trace, bisecting the regression, and fixing it surgically.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add MCP server setup and usage documentation"
```
