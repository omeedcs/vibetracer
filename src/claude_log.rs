use chrono::DateTime;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ---- Structures ----

/// A parsed conversation turn (one user prompt -> one assistant response).
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// Unix timestamp in milliseconds for this turn.
    pub timestamp: i64,
    /// User prompt text, truncated to 200 chars.
    pub user_prompt: String,
    /// Tool calls made during this turn.
    pub tool_calls: Vec<ToolCall>,
    /// Assistant response text, truncated to 200 chars.
    pub assistant_text: String,
    /// Input tokens consumed.
    pub tokens_in: u64,
    /// Output tokens produced.
    pub tokens_out: u64,
    /// Tokens served from cache reads.
    pub cache_read: u64,
    /// Tokens written to cache.
    pub cache_write: u64,
    /// Model identifier (e.g. "claude-opus-4-6").
    pub model: String,
    /// Duration from first to last message in the turn, in milliseconds.
    pub duration_ms: u64,
}

/// A single tool call within a conversation turn.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// Tool name: "Read", "Edit", "Write", "Grep", "Bash", etc.
    pub tool_name: String,
    /// File path extracted from tool input, if applicable.
    pub file_path: Option<String>,
    /// Lines added (for Edit/Write tools).
    pub lines_added: Option<u32>,
    /// Lines removed (for Edit/Write tools).
    pub lines_removed: Option<u32>,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
    /// Truncated result summary (e.g. "12 matches" for Grep).
    pub result_summary: String,
}

/// Aggregate token usage statistics.
#[derive(Debug, Clone, Default)]
pub struct TokenStats {
    pub total_in: u64,
    pub total_out: u64,
    pub total_cache_read: u64,
    pub total_cache_write: u64,
    pub total_cost: f64,
}

// ---- Internal helpers ----

/// Truncate a string to at most `max` characters, appending "..." if truncated.
fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        let mut end = max;
        // Avoid splitting in the middle of a multi-byte char.
        while !trimmed.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &trimmed[..end])
    }
}

/// Parse an ISO 8601 / RFC 3339 timestamp string to unix milliseconds.
/// Falls back to trying a bare unix-seconds integer. Returns 0 on failure.
fn parse_timestamp(val: &Value) -> i64 {
    if let Some(s) = val.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return dt.timestamp_millis();
        }
        // Try parsing as unix seconds (integer in a string)
        if let Ok(secs) = s.parse::<i64>() {
            return secs * 1000;
        }
    }
    if let Some(n) = val.as_i64() {
        // If the number looks like milliseconds (> year 2001 in ms), use directly.
        // Otherwise treat as seconds.
        if n > 1_000_000_000_000 {
            return n;
        }
        return n * 1000;
    }
    0
}

/// Extract the record-level timestamp from a JSONL line value.
fn record_ts(val: &Value) -> i64 {
    val.get("timestamp")
        .map(|t| parse_timestamp(t))
        .unwrap_or(0)
}

/// Extract plain text from an assistant message's content blocks.
fn extract_text(message: &Value) -> String {
    let mut texts = Vec::new();
    if let Some(arr) = message.get("content").and_then(|c| c.as_array()) {
        for item in arr {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        texts.push(trimmed.to_string());
                    }
                }
            }
        }
    }
    // Also handle the case where content is a plain string
    if texts.is_empty() {
        if let Some(s) = message.get("content").and_then(|c| c.as_str()) {
            texts.push(s.trim().to_string());
        }
    }
    texts.join(" ")
}

/// Extract the user prompt text. Handles both string content and array-of-blocks.
fn extract_user_prompt(val: &Value) -> String {
    let message = match val.get("message") {
        Some(m) => m,
        None => return String::new(),
    };
    // Content can be a plain string
    if let Some(s) = message.get("content").and_then(|c| c.as_str()) {
        return truncate(s, 200);
    }
    // Or an array of content blocks
    if let Some(arr) = message.get("content").and_then(|c| c.as_array()) {
        for item in arr {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        return truncate(trimmed, 200);
                    }
                }
            }
        }
    }
    String::new()
}

/// Extract file_path from a tool_use input object.
/// Checks "file_path", "path", and "file" keys.
fn extract_file_path(input: &Value) -> Option<String> {
    for key in &["file_path", "path", "file"] {
        if let Some(s) = input.get(*key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// For Edit tools, count lines added/removed from old_string/new_string.
fn count_edit_lines(input: &Value) -> (Option<u32>, Option<u32>) {
    let old = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
    if old.is_empty() && new.is_empty() {
        return (None, None);
    }
    let old_lines = if old.is_empty() { 0 } else { old.lines().count() as u32 };
    let new_lines = if new.is_empty() { 0 } else { new.lines().count() as u32 };
    (Some(new_lines), Some(old_lines))
}

/// For Write tools, count lines from the content field.
fn count_write_lines(input: &Value) -> (Option<u32>, Option<u32>) {
    if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
        let lines = if content.is_empty() { 0 } else { content.lines().count() as u32 };
        (Some(lines), Some(0))
    } else {
        (None, None)
    }
}

/// Summarize a tool result. Returns a short description.
fn summarize_tool_result(tool_name: &str, result_content: &str) -> String {
    let trimmed = result_content.trim();
    match tool_name {
        "Grep" => {
            // Try to count matches from output lines
            let line_count = trimmed.lines().count();
            if line_count > 0 {
                format!("{} lines", line_count)
            } else {
                "no matches".to_string()
            }
        }
        "Bash" => {
            // Show truncated output or exit status hint
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.is_empty() {
                "(empty output)".to_string()
            } else if lines.len() == 1 {
                truncate(lines[0], 80)
            } else {
                format!("{} lines of output", lines.len())
            }
        }
        "Read" => {
            let line_count = trimmed.lines().count();
            format!("{} lines read", line_count)
        }
        "Edit" => "applied".to_string(),
        "Write" => "written".to_string(),
        "Glob" => {
            let count = trimmed.lines().count();
            format!("{} files", count)
        }
        _ => truncate(trimmed, 60),
    }
}

/// Represents the type of a JSONL record.
#[derive(Debug, PartialEq)]
enum RecordType {
    Human,
    Assistant,
    ToolResult,
    Unknown,
}

fn classify_record(val: &Value) -> RecordType {
    match val.get("type").and_then(|v| v.as_str()) {
        Some("human") | Some("user") => RecordType::Human,
        Some("assistant") => RecordType::Assistant,
        Some("tool_result") => RecordType::ToolResult,
        _ => RecordType::Unknown,
    }
}

// ---- Internal turn-assembly state ----

/// Accumulator for building a ConversationTurn from sequential JSONL records.
struct TurnBuilder {
    user_prompt: String,
    user_ts: i64,
    assistant_text: String,
    tool_calls: Vec<ToolCall>,
    tokens_in: u64,
    tokens_out: u64,
    cache_read: u64,
    cache_write: u64,
    model: String,
    last_ts: i64,
    /// Map from tool_use_id to index in tool_calls, for attaching results.
    pending_tools: Vec<(String, usize)>,
}

impl TurnBuilder {
    fn new(user_prompt: String, user_ts: i64) -> Self {
        Self {
            user_prompt,
            user_ts,
            assistant_text: String::new(),
            tool_calls: Vec::new(),
            tokens_in: 0,
            tokens_out: 0,
            cache_read: 0,
            cache_write: 0,
            model: String::new(),
            last_ts: user_ts,
            pending_tools: Vec::new(),
        }
    }

    /// Ingest an assistant record.
    fn add_assistant(&mut self, val: &Value) {
        let ts = record_ts(val);
        if ts > self.last_ts {
            self.last_ts = ts;
        }

        let message = match val.get("message") {
            Some(m) => m,
            None => return,
        };

        // Extract text
        let text = extract_text(message);
        if !text.is_empty() {
            if !self.assistant_text.is_empty() {
                self.assistant_text.push(' ');
            }
            self.assistant_text.push_str(&text);
        }

        // Extract model
        if let Some(m) = message.get("model").and_then(|v| v.as_str()) {
            if !m.is_empty() {
                self.model = m.to_string();
            }
        }

        // Extract token usage
        if let Some(usage) = message.get("usage") {
            self.tokens_in += usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.tokens_out += usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.cache_read += usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.cache_write += usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
        }

        // Extract tool calls
        if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
            for item in content {
                if item.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                    continue;
                }
                let tool_name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let input = item.get("input").cloned().unwrap_or(Value::Null);
                let file_path = extract_file_path(&input);

                let (lines_added, lines_removed) = match tool_name.as_str() {
                    "Edit" => count_edit_lines(&input),
                    "Write" => count_write_lines(&input),
                    _ => (None, None),
                };

                let tool_use_id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let idx = self.tool_calls.len();
                self.tool_calls.push(ToolCall {
                    tool_name: tool_name.clone(),
                    file_path,
                    lines_added,
                    lines_removed,
                    timestamp: ts,
                    result_summary: String::new(),
                });

                if !tool_use_id.is_empty() {
                    self.pending_tools.push((tool_use_id, idx));
                }
            }
        }
    }

    /// Ingest a tool_result record, attaching the result summary to the matching tool call.
    fn add_tool_result(&mut self, val: &Value) {
        let ts = record_ts(val);
        if ts > self.last_ts {
            self.last_ts = ts;
        }

        let tool_use_id = val
            .get("tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Find the matching tool call
        let idx = self
            .pending_tools
            .iter()
            .find(|(id, _)| id == tool_use_id)
            .map(|(_, idx)| *idx);

        if let Some(idx) = idx {
            let result_content = val
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tool_name = self.tool_calls[idx].tool_name.clone();
            self.tool_calls[idx].result_summary =
                summarize_tool_result(&tool_name, result_content);
        }
    }

    /// Finalize into a ConversationTurn.
    fn build(self) -> ConversationTurn {
        let duration_ms = if self.last_ts > self.user_ts {
            (self.last_ts - self.user_ts) as u64
        } else {
            0
        };

        ConversationTurn {
            timestamp: self.user_ts,
            user_prompt: truncate(&self.user_prompt, 200),
            tool_calls: self.tool_calls,
            assistant_text: truncate(&self.assistant_text, 200),
            tokens_in: self.tokens_in,
            tokens_out: self.tokens_out,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            model: self.model,
            duration_ms,
        }
    }
}

// ---- Public API ----

/// Find the Claude Code conversation log directory for the given project path.
///
/// Looks in `~/.claude/projects/` for a directory matching the project path
/// (converted to hyphen-separated form, e.g. `/Users/foo/bar` -> `-Users-foo-bar`).
/// Returns the path to the most recently modified `.jsonl` file in that directory.
pub fn find_log_path(project_path: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let claude_projects = home.join(".claude").join("projects");
    let converted = project_path.to_string_lossy().replace('/', "-");
    let sessions_dir = claude_projects.join(&converted);

    if !sessions_dir.exists() {
        return None;
    }

    // Find the most recently modified .jsonl file
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;

    let entries = fs::read_dir(&sessions_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
        if let Some(mod_time) = modified {
            match &best {
                None => best = Some((path, mod_time)),
                Some((_, prev_time)) => {
                    if mod_time > *prev_time {
                        best = Some((path, mod_time));
                    }
                }
            }
        }
    }

    best.map(|(p, _)| p)
}

/// Parse a Claude Code JSONL log file into conversation turns.
///
/// Handles malformed lines gracefully by skipping them. Groups records into
/// turns based on human/assistant/tool_result sequences.
pub fn parse_log(path: &Path) -> Vec<ConversationTurn> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_lines(content.lines())
}

/// Core parsing logic: takes an iterator of JSONL lines and produces turns.
fn parse_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<ConversationTurn> {
    let mut turns: Vec<ConversationTurn> = Vec::new();
    let mut current: Option<TurnBuilder> = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let val: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue, // Skip malformed lines
        };

        match classify_record(&val) {
            RecordType::Human => {
                // Finalize previous turn if any
                if let Some(builder) = current.take() {
                    turns.push(builder.build());
                }
                let prompt = extract_user_prompt(&val);
                let ts = record_ts(&val);
                current = Some(TurnBuilder::new(prompt, ts));
            }
            RecordType::Assistant => {
                if let Some(ref mut builder) = current {
                    builder.add_assistant(&val);
                } else {
                    // Orphaned assistant message -- create a turn with no user prompt
                    let ts = record_ts(&val);
                    let mut builder = TurnBuilder::new(String::new(), ts);
                    builder.add_assistant(&val);
                    current = Some(builder);
                }
            }
            RecordType::ToolResult => {
                if let Some(ref mut builder) = current {
                    builder.add_tool_result(&val);
                }
                // Orphaned tool results are discarded
            }
            RecordType::Unknown => {
                // Skip unknown record types
            }
        }
    }

    // Finalize the last turn
    if let Some(builder) = current.take() {
        turns.push(builder.build());
    }

    turns
}

/// Compute aggregate token statistics from a list of conversation turns.
pub fn compute_stats(turns: &[ConversationTurn]) -> TokenStats {
    let mut stats = TokenStats::default();

    for turn in turns {
        stats.total_in += turn.tokens_in;
        stats.total_out += turn.tokens_out;
        stats.total_cache_read += turn.cache_read;
        stats.total_cache_write += turn.cache_write;
        stats.total_cost += estimate_cost(
            &turn.model,
            turn.tokens_in,
            turn.tokens_out,
            turn.cache_read,
        );
    }

    stats
}

/// Estimate cost in USD based on model name and token counts.
///
/// Uses approximate Claude pricing (per million tokens):
/// - opus:   $15 input, $75 output, $1.875 cache read
/// - sonnet: $3 input,  $15 output, $0.375 cache read
/// - haiku:  $0.25 input, $1.25 output, $0.03125 cache read
///
/// Falls back to sonnet pricing for unrecognized models.
pub fn estimate_cost(model: &str, tokens_in: u64, tokens_out: u64, cache_read: u64) -> f64 {
    let model_lower = model.to_lowercase();

    let (price_in, price_out, price_cache) = if model_lower.contains("opus") {
        (15.0, 75.0, 1.875)
    } else if model_lower.contains("haiku") {
        (0.25, 1.25, 0.03125)
    } else {
        // Default to sonnet pricing (covers "sonnet" and unknown models)
        (3.0, 15.0, 0.375)
    };

    let million = 1_000_000.0;
    (tokens_in as f64 * price_in / million)
        + (tokens_out as f64 * price_out / million)
        + (cache_read as f64 * price_cache / million)
}

/// Tail a log file for new entries.
///
/// Returns all existing conversation turns plus a receiver that yields new
/// turns as they appear. Spawns a background thread that polls the file for
/// growth every 500ms.
///
/// The background thread stops when the receiver is dropped.
pub fn tail_log(path: &Path) -> (Vec<ConversationTurn>, mpsc::Receiver<ConversationTurn>) {
    let existing = parse_log(path);
    let (tx, rx) = mpsc::channel();

    let path_buf = path.to_path_buf();

    // Track where we left off -- start after the current file size.
    let initial_size = fs::metadata(&path_buf)
        .map(|m| m.len())
        .unwrap_or(0);

    thread::spawn(move || {
        let mut offset = initial_size;
        // Buffer to accumulate partial lines across polls
        let mut leftover = String::new();
        // Carry forward the last human prompt so we can build turns from tailed data
        let mut current_builder: Option<TurnBuilder> = None;

        loop {
            thread::sleep(Duration::from_millis(500));

            let current_size = match fs::metadata(&path_buf) {
                Ok(m) => m.len(),
                Err(_) => continue,
            };

            if current_size <= offset {
                // File truncated or unchanged
                if current_size < offset {
                    // File was truncated; reset
                    offset = 0;
                    leftover.clear();
                    current_builder = None;
                }
                continue;
            }

            // Read new bytes
            let mut file = match fs::File::open(&path_buf) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if file.seek(SeekFrom::Start(offset)).is_err() {
                continue;
            }
            let reader = std::io::BufReader::new(&mut file);
            let mut new_data = String::new();
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        new_data.push_str(&l);
                        new_data.push('\n');
                    }
                    Err(_) => break,
                }
            }

            offset = current_size;

            // Prepend any leftover from last poll
            let combined = if leftover.is_empty() {
                new_data
            } else {
                let mut c = std::mem::take(&mut leftover);
                c.push_str(&new_data);
                c
            };

            for line in combined.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let val: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(_) => {
                        // Could be a partial line; stash it
                        leftover.push_str(line);
                        leftover.push('\n');
                        continue;
                    }
                };

                match classify_record(&val) {
                    RecordType::Human => {
                        // Emit any previous turn
                        if let Some(builder) = current_builder.take() {
                            let turn = builder.build();
                            if tx.send(turn).is_err() {
                                return; // Receiver dropped
                            }
                        }
                        let prompt = extract_user_prompt(&val);
                        let ts = record_ts(&val);
                        current_builder = Some(TurnBuilder::new(prompt, ts));
                    }
                    RecordType::Assistant => {
                        if let Some(ref mut builder) = current_builder {
                            builder.add_assistant(&val);
                        }
                    }
                    RecordType::ToolResult => {
                        if let Some(ref mut builder) = current_builder {
                            builder.add_tool_result(&val);
                        }
                    }
                    RecordType::Unknown => {}
                }
            }

            // If we have a complete-looking turn (has assistant response and
            // no pending tool calls), flush it. Otherwise keep accumulating.
            // We use a heuristic: if the builder has assistant_text and the
            // last record was not a tool call, emit it.
            // Actually, for simplicity in the tail scenario, we defer flushing
            // until the next human message arrives (handled above).
        }
    });

    (existing, rx)
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    fn make_human(prompt: &str, ts: &str) -> String {
        serde_json::json!({
            "type": "human",
            "message": {
                "role": "user",
                "content": prompt
            },
            "timestamp": ts
        })
        .to_string()
    }

    fn make_assistant(text: &str, tools: &[(&str, &str, &str)], ts: &str) -> String {
        let mut content = Vec::new();
        if !text.is_empty() {
            content.push(serde_json::json!({
                "type": "text",
                "text": text
            }));
        }
        for (id, name, file_path) in tools {
            let mut input = serde_json::json!({});
            if !file_path.is_empty() {
                input["file_path"] = serde_json::json!(file_path);
            }
            content.push(serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));
        }

        serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": content,
                "model": "claude-sonnet-4-20250514",
                "usage": {
                    "input_tokens": 1000,
                    "output_tokens": 200,
                    "cache_creation_input_tokens": 0,
                    "cache_read_input_tokens": 500
                }
            },
            "timestamp": ts
        })
        .to_string()
    }

    fn make_tool_result(tool_use_id: &str, content: &str, ts: &str) -> String {
        serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "timestamp": ts
        })
        .to_string()
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 200), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let long = "a".repeat(300);
        let result = truncate(&long, 200);
        assert_eq!(result.len(), 203); // 200 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_multibyte() {
        // Ensure truncation does not split a multi-byte character
        let s = "Hello \u{1F600} world and more text that is long enough";
        let result = truncate(s, 10);
        // Should not panic and should be valid UTF-8
        assert!(result.len() <= 20); // rough upper bound
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_parse_timestamp_rfc3339() {
        let val = serde_json::json!("2026-03-28T14:32:01Z");
        let ts = parse_timestamp(&val);
        assert!(ts > 0);
        // 2026-03-28T14:32:01Z in millis
        assert!(ts > 1_774_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_unix_seconds() {
        let val = serde_json::json!(1700000000);
        let ts = parse_timestamp(&val);
        assert_eq!(ts, 1_700_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_unix_millis() {
        let val = serde_json::json!(1700000000000_i64);
        let ts = parse_timestamp(&val);
        assert_eq!(ts, 1_700_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_string_seconds() {
        let val = serde_json::json!("1700000000");
        let ts = parse_timestamp(&val);
        assert_eq!(ts, 1_700_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let val = serde_json::json!("not-a-date");
        assert_eq!(parse_timestamp(&val), 0);
    }

    #[test]
    fn test_estimate_cost_opus() {
        let cost = estimate_cost("claude-opus-4-6", 1_000_000, 100_000, 500_000);
        // 1M * 15/M + 0.1M * 75/M + 0.5M * 1.875/M = 15 + 7.5 + 0.9375 = 23.4375
        let expected = 15.0 + 7.5 + 0.9375;
        assert!((cost - expected).abs() < 0.001, "got {cost}");
    }

    #[test]
    fn test_estimate_cost_sonnet() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 100_000, 500_000);
        let expected = 3.0 + 1.5 + 0.1875;
        assert!((cost - expected).abs() < 0.001, "got {cost}");
    }

    #[test]
    fn test_estimate_cost_haiku() {
        let cost = estimate_cost("claude-haiku-3-5", 1_000_000, 100_000, 0);
        let expected = 0.25 + 0.125;
        assert!((cost - expected).abs() < 0.001, "got {cost}");
    }

    #[test]
    fn test_estimate_cost_unknown_model() {
        // Unknown models fall back to sonnet pricing
        let cost = estimate_cost("gpt-4o", 1_000_000, 0, 0);
        assert!((cost - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_simple_turn() {
        let lines = vec![
            make_human("fix the auth middleware", "2026-03-28T14:32:01Z"),
            make_assistant(
                "I'll fix the auth middleware.",
                &[("toolu_123", "Read", "src/auth.rs")],
                "2026-03-28T14:32:03Z",
            ),
            make_tool_result("toolu_123", "file contents here...", "2026-03-28T14:32:04Z"),
        ];
        let input = lines.join("\n");
        let turns = parse_lines(input.lines());

        assert_eq!(turns.len(), 1);
        let turn = &turns[0];
        assert_eq!(turn.user_prompt, "fix the auth middleware");
        assert!(turn.assistant_text.contains("I'll fix"));
        assert_eq!(turn.tokens_in, 1000);
        assert_eq!(turn.tokens_out, 200);
        assert_eq!(turn.cache_read, 500);
        assert_eq!(turn.model, "claude-sonnet-4-20250514");
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].tool_name, "Read");
        assert_eq!(
            turn.tool_calls[0].file_path,
            Some("src/auth.rs".to_string())
        );
        assert!(turn.tool_calls[0].result_summary.contains("lines read"));
        assert!(turn.duration_ms > 0);
    }

    #[test]
    fn test_parse_multiple_turns() {
        let lines = vec![
            make_human("first question", "2026-03-28T14:00:00Z"),
            make_assistant("first answer", &[], "2026-03-28T14:00:02Z"),
            make_human("second question", "2026-03-28T14:01:00Z"),
            make_assistant("second answer", &[], "2026-03-28T14:01:02Z"),
        ];
        let input = lines.join("\n");
        let turns = parse_lines(input.lines());

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].user_prompt, "first question");
        assert_eq!(turns[1].user_prompt, "second question");
    }

    #[test]
    fn test_parse_skips_malformed_lines() {
        let lines = vec![
            make_human("hello", "2026-03-28T14:00:00Z"),
            "this is not json".to_string(),
            "{\"incomplete\": true".to_string(),
            make_assistant("world", &[], "2026-03-28T14:00:02Z"),
        ];
        let input = lines.join("\n");
        let turns = parse_lines(input.lines());

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_prompt, "hello");
        assert!(turns[0].assistant_text.contains("world"));
    }

    #[test]
    fn test_parse_empty_file() {
        let turns = parse_lines("".lines());
        assert!(turns.is_empty());
    }

    #[test]
    fn test_parse_only_blank_lines() {
        let turns = parse_lines("\n\n  \n".lines());
        assert!(turns.is_empty());
    }

    #[test]
    fn test_compute_stats() {
        let turns = vec![
            ConversationTurn {
                timestamp: 1000,
                user_prompt: "a".to_string(),
                tool_calls: vec![],
                assistant_text: "b".to_string(),
                tokens_in: 1000,
                tokens_out: 200,
                cache_read: 500,
                cache_write: 100,
                model: "claude-sonnet-4-20250514".to_string(),
                duration_ms: 2000,
            },
            ConversationTurn {
                timestamp: 5000,
                user_prompt: "c".to_string(),
                tool_calls: vec![],
                assistant_text: "d".to_string(),
                tokens_in: 2000,
                tokens_out: 400,
                cache_read: 300,
                cache_write: 50,
                model: "claude-sonnet-4-20250514".to_string(),
                duration_ms: 1500,
            },
        ];

        let stats = compute_stats(&turns);
        assert_eq!(stats.total_in, 3000);
        assert_eq!(stats.total_out, 600);
        assert_eq!(stats.total_cache_read, 800);
        assert_eq!(stats.total_cache_write, 150);
        assert!(stats.total_cost > 0.0);
    }

    #[test]
    fn test_extract_file_path_variants() {
        let v1 = serde_json::json!({"file_path": "src/main.rs"});
        assert_eq!(extract_file_path(&v1), Some("src/main.rs".to_string()));

        let v2 = serde_json::json!({"path": "/tmp/foo.txt"});
        assert_eq!(extract_file_path(&v2), Some("/tmp/foo.txt".to_string()));

        let v3 = serde_json::json!({"file": "bar.py"});
        assert_eq!(extract_file_path(&v3), Some("bar.py".to_string()));

        let v4 = serde_json::json!({"command": "ls"});
        assert_eq!(extract_file_path(&v4), None);
    }

    #[test]
    fn test_count_edit_lines() {
        let input = serde_json::json!({
            "old_string": "line1\nline2\nline3",
            "new_string": "line1\nnewline2"
        });
        let (added, removed) = count_edit_lines(&input);
        assert_eq!(added, Some(2));
        assert_eq!(removed, Some(3));
    }

    #[test]
    fn test_count_write_lines() {
        let input = serde_json::json!({
            "content": "fn main() {\n    println!(\"hello\");\n}"
        });
        let (added, removed) = count_write_lines(&input);
        assert_eq!(added, Some(3));
        assert_eq!(removed, Some(0));
    }

    #[test]
    fn test_summarize_grep_result() {
        let summary = summarize_tool_result("Grep", "file1.rs:10:match\nfile2.rs:20:match");
        assert_eq!(summary, "2 lines");
    }

    #[test]
    fn test_summarize_bash_result() {
        let summary = summarize_tool_result("Bash", "hello world");
        assert_eq!(summary, "hello world");
    }

    #[test]
    fn test_summarize_bash_multiline() {
        let summary = summarize_tool_result("Bash", "line1\nline2\nline3");
        assert_eq!(summary, "3 lines of output");
    }

    #[test]
    fn test_summarize_empty_bash() {
        let summary = summarize_tool_result("Bash", "");
        assert_eq!(summary, "(empty output)");
    }

    #[test]
    fn test_orphaned_assistant_creates_turn() {
        // An assistant record without a preceding human record
        let lines = vec![make_assistant(
            "orphaned response",
            &[],
            "2026-03-28T14:00:00Z",
        )];
        let input = lines.join("\n");
        let turns = parse_lines(input.lines());

        assert_eq!(turns.len(), 1);
        assert!(turns[0].user_prompt.is_empty());
        assert!(turns[0].assistant_text.contains("orphaned response"));
    }

    #[test]
    fn test_multiple_tool_calls_in_one_turn() {
        let lines = vec![
            make_human("refactor auth", "2026-03-28T14:00:00Z"),
            make_assistant(
                "I'll read both files.",
                &[
                    ("toolu_1", "Read", "src/auth.rs"),
                    ("toolu_2", "Grep", ""),
                ],
                "2026-03-28T14:00:02Z",
            ),
            make_tool_result("toolu_1", "mod auth { ... }", "2026-03-28T14:00:03Z"),
            make_tool_result(
                "toolu_2",
                "src/main.rs:5:use auth",
                "2026-03-28T14:00:04Z",
            ),
        ];
        let input = lines.join("\n");
        let turns = parse_lines(input.lines());

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].tool_calls.len(), 2);
        assert_eq!(turns[0].tool_calls[0].tool_name, "Read");
        assert_eq!(turns[0].tool_calls[1].tool_name, "Grep");
        assert!(turns[0].tool_calls[0].result_summary.contains("lines read"));
        assert!(turns[0].tool_calls[1].result_summary.contains("1 lines"));
    }

    #[test]
    fn test_parse_log_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.jsonl");
        let mut f = fs::File::create(&log_path).unwrap();

        writeln!(
            f,
            "{}",
            make_human("hello", "2026-03-28T14:00:00Z")
        )
        .unwrap();
        writeln!(
            f,
            "{}",
            make_assistant("hi there", &[], "2026-03-28T14:00:01Z")
        )
        .unwrap();

        let turns = parse_log(&log_path);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_prompt, "hello");
    }

    #[test]
    fn test_parse_log_nonexistent_file() {
        let turns = parse_log(Path::new("/nonexistent/path/session.jsonl"));
        assert!(turns.is_empty());
    }

    #[test]
    fn test_tail_log_receives_new_turns() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.jsonl");

        // Write initial content
        {
            let mut f = fs::File::create(&log_path).unwrap();
            writeln!(f, "{}", make_human("initial", "2026-03-28T14:00:00Z")).unwrap();
            writeln!(
                f,
                "{}",
                make_assistant("initial response", &[], "2026-03-28T14:00:01Z")
            )
            .unwrap();
        }

        let (existing, rx) = tail_log(&log_path);
        assert_eq!(existing.len(), 1);

        // Append a new turn after a small delay
        thread::sleep(Duration::from_millis(100));
        {
            let mut f = fs::OpenOptions::new().append(true).open(&log_path).unwrap();
            writeln!(f, "{}", make_human("new question", "2026-03-28T14:01:00Z")).unwrap();
            writeln!(
                f,
                "{}",
                make_assistant("new answer", &[], "2026-03-28T14:01:01Z")
            )
            .unwrap();
            // Write another human message to flush the previous turn
            writeln!(
                f,
                "{}",
                make_human("trigger flush", "2026-03-28T14:02:00Z")
            )
            .unwrap();
        }

        // Wait for the poller to pick it up
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(turn) => {
                assert_eq!(turn.user_prompt, "new question");
                assert!(turn.assistant_text.contains("new answer"));
            }
            Err(_) => panic!("timed out waiting for tailed turn"),
        }
    }

    #[test]
    fn test_user_prompt_plain_string_content() {
        // Some log formats use a plain string for the user content
        let line = serde_json::json!({
            "type": "human",
            "message": {
                "role": "user",
                "content": "just a string"
            },
            "timestamp": "2026-03-28T14:00:00Z"
        })
        .to_string();

        let turns = parse_lines(std::iter::once(line.as_str()));
        // Single human with no assistant still creates a turn
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_prompt, "just a string");
    }

    #[test]
    fn test_classify_record_user_alias() {
        let val = serde_json::json!({"type": "user"});
        assert_eq!(classify_record(&val), RecordType::Human);
    }

    #[test]
    fn test_token_stats_default() {
        let stats = TokenStats::default();
        assert_eq!(stats.total_in, 0);
        assert_eq!(stats.total_out, 0);
        assert_eq!(stats.total_cache_read, 0);
        assert_eq!(stats.total_cache_write, 0);
        assert!((stats.total_cost - 0.0).abs() < f64::EPSILON);
    }
}
