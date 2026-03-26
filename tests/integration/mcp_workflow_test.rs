use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

use vibetracer::event::{EditEvent, EditKind};
use vibetracer::session::{SessionMeta, SessionMode};
use vibetracer::snapshot::edit_log::EditLog;
use vibetracer::snapshot::store::SnapshotStore;

// ── helpers ──────────────────────────────────────────────────────────────────

fn start_mcp(project_dir: &Path) -> Child {
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
    params: Value,
) -> Value {
    let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    writeln!(stdin, "{}", serde_json::to_string(&req).unwrap()).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

fn send_notification(stdin: &mut impl Write, method: &str) {
    let notification = json!({"jsonrpc": "2.0", "method": method});
    writeln!(stdin, "{}", serde_json::to_string(&notification).unwrap()).unwrap();
    stdin.flush().unwrap();
}

fn parse_tool_result(resp: &Value) -> Value {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should contain content[0].text");
    serde_json::from_str(text).expect("tool result text should be valid JSON")
}

// ── session setup ────────────────────────────────────────────────────────────

fn create_realistic_session(sessions_dir: &Path) {
    let session_dir = sessions_dir.join("workflow-test");
    std::fs::create_dir_all(session_dir.join("snapshots")).unwrap();
    std::fs::create_dir_all(session_dir.join("checkpoints")).unwrap();

    // Store real file snapshots
    let store = SnapshotStore::new(session_dir.join("snapshots"));

    let h1 = store.store(b"fn main() {}\n").unwrap();
    let h2 = store
        .store(b"fn main() {\n    println!(\"hello\");\n}\n")
        .unwrap();
    let h3 = store
        .store(b"fn main() {\n    println!(\"hello\");\n    broken_function();\n}\n")
        .unwrap();

    // Write meta.json
    let meta = SessionMeta {
        id: "workflow-test".to_string(),
        project_path: sessions_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
        started_at: 1_700_000_000,
        mode: SessionMode::Enriched,
        agents: vec![vibetracer::event::AgentInfo {
            agent_id: "a1".to_string(),
            agent_label: "claude-1".to_string(),
            tool_type: "claude-code".to_string(),
            first_seen: 1_700_000_000_000,
            last_seen: 1_700_000_003_000,
            edit_count: 3,
        }],
    };
    std::fs::write(
        session_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();

    // Write edits.jsonl with 3 EditEvents
    let log = EditLog::new(session_dir.join("edits.jsonl"));

    // Edit 1: Create file with `fn main() {}\n`
    log.append(&EditEvent {
        id: 1,
        ts: 1_700_000_001_000,
        file: "src/main.rs".to_string(),
        kind: EditKind::Create,
        patch: "@@ -0,0 +1 @@\n+fn main() {}\n".to_string(),
        before_hash: None,
        after_hash: h1.clone(),
        intent: Some("scaffold main".to_string()),
        tool: None,
        lines_added: 1,
        lines_removed: 0,
        agent_id: Some("a1".to_string()),
        agent_label: Some("claude-1".to_string()),
        operation_id: Some("op-1".to_string()),
        operation_intent: Some("initial setup".to_string()),
        tool_name: None,
        restore_id: None,
    })
    .unwrap();

    // Edit 2: Modify to add println
    log.append(&EditEvent {
        id: 2,
        ts: 1_700_000_002_000,
        file: "src/main.rs".to_string(),
        kind: EditKind::Modify,
        patch: "@@ -1 +1,3 @@\n-fn main() {}\n+fn main() {\n+    println!(\"hello\");\n+}\n"
            .to_string(),
        before_hash: Some(h1),
        after_hash: h2.clone(),
        intent: Some("add greeting".to_string()),
        tool: None,
        lines_added: 3,
        lines_removed: 1,
        agent_id: Some("a1".to_string()),
        agent_label: Some("claude-1".to_string()),
        operation_id: Some("op-2".to_string()),
        operation_intent: Some("add hello".to_string()),
        tool_name: None,
        restore_id: None,
    })
    .unwrap();

    // Edit 3: Modify to add broken_function() call (the regression)
    log.append(&EditEvent {
        id: 3,
        ts: 1_700_000_003_000,
        file: "src/main.rs".to_string(),
        kind: EditKind::Modify,
        patch: "@@ -1,3 +1,4 @@\n fn main() {\n     println!(\"hello\");\n+    broken_function();\n }\n"
            .to_string(),
        before_hash: Some(h2),
        after_hash: h3,
        intent: Some("add feature".to_string()),
        tool: None,
        lines_added: 1,
        lines_removed: 0,
        agent_id: Some("a1".to_string()),
        agent_label: Some("claude-1".to_string()),
        operation_id: Some("op-3".to_string()),
        operation_intent: Some("add feature".to_string()),
        tool_name: None,
        restore_id: None,
    })
    .unwrap();
}

// ── main test ────────────────────────────────────────────────────────────────

#[test]
fn test_full_self_correction_workflow() {
    // 1. Set up temp dir with realistic session
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join(".vibetracer").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    create_realistic_session(&sessions_dir);

    // 2. Start MCP server
    let mut child = start_mcp(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // 3a. Initialize
    let resp = send(&mut stdin, &mut stdout, "initialize", 1, json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");

    // Send initialized notification
    send_notification(&mut stdin, "notifications/initialized");

    // 3b. list_sessions -- verify 1 session found
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        2,
        json!({
            "name": "list_sessions",
            "arguments": {}
        }),
    );
    let result = parse_tool_result(&resp);
    assert_eq!(
        result["total_count"], 1,
        "expected 1 session, got: {}",
        result
    );
    let sessions = result["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "workflow-test");

    // 3c. get_timeline -- verify 3 edits
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        3,
        json!({
            "name": "get_timeline",
            "arguments": {
                "session_id": "workflow-test"
            }
        }),
    );
    let result = parse_tool_result(&resp);
    assert_eq!(
        result["total_count"], 3,
        "expected 3 edits in timeline, got: {}",
        result
    );
    let edits = result["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 3);

    // 3d. get_regression_window for src/main.rs -- verify 3 frames
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        4,
        json!({
            "name": "get_regression_window",
            "arguments": {
                "session_id": "workflow-test",
                "file": "src/main.rs"
            }
        }),
    );
    let result = parse_tool_result(&resp);
    let frames = result["frames"].as_array().unwrap();
    assert_eq!(
        frames.len(),
        3,
        "expected 3 frames for src/main.rs, got: {}",
        result
    );

    // 3e. search_edits for "broken_function" -- verify finds edit 3
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        5,
        json!({
            "name": "search_edits",
            "arguments": {
                "session_id": "workflow-test",
                "query": "broken_function"
            }
        }),
    );
    let result = parse_tool_result(&resp);
    assert_eq!(
        result["total_count"], 1,
        "expected 1 search hit for broken_function, got: {}",
        result
    );
    let found_edits = result["edits"].as_array().unwrap();
    assert_eq!(found_edits.len(), 1);
    assert_eq!(
        found_edits[0]["id"], 3,
        "search should find edit 3 (the regression)"
    );

    // 3f. get_frame at frame 2 (last known good) -- verify content has println but NOT broken_function
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        6,
        json!({
            "name": "get_frame",
            "arguments": {
                "session_id": "workflow-test",
                "frame_id": 2
            }
        }),
    );
    let result = parse_tool_result(&resp);
    let files = result["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let content = files[0]["content"].as_str().unwrap();
    assert!(
        content.contains("println"),
        "frame 2 content should contain println, got: {}",
        content
    );
    assert!(
        !content.contains("broken_function"),
        "frame 2 content should NOT contain broken_function, got: {}",
        content
    );

    // 3g. diff_frames between frames 2 and 3 -- verify diff contains broken_function
    let resp = send(
        &mut stdin,
        &mut stdout,
        "tools/call",
        7,
        json!({
            "name": "diff_frames",
            "arguments": {
                "session_id": "workflow-test",
                "frame_a": 2,
                "frame_b": 3
            }
        }),
    );
    let result = parse_tool_result(&resp);
    let diffs = result["diffs"].as_array().unwrap();
    assert_eq!(diffs.len(), 1, "expected 1 diff entry, got: {}", result);
    let diff_text = diffs[0]["diff"].as_str().unwrap();
    assert!(
        diff_text.contains("broken_function"),
        "diff between frames 2 and 3 should contain broken_function, got: {}",
        diff_text
    );

    // Clean up
    drop(stdin);
    child.wait().unwrap();
}
