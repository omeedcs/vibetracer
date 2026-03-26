use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

fn start_mcp_server(project_dir: &Path) -> Child {
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

fn send_request(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    method: &str,
    id: u64,
    params: Value,
) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
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

#[test]
fn test_mcp_initialize_and_list_tools() {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join(".vibetracer").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let mut child = start_mcp_server(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send "initialize" request
    let resp = send_request(&mut stdin, &mut stdout, "initialize", 1, json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    let server_name = resp["result"]["serverInfo"]["name"].as_str().unwrap();
    assert!(
        server_name.contains("vibetracer"),
        "serverInfo.name should contain 'vibetracer', got: {}",
        server_name
    );

    // Send "notifications/initialized" notification (no response expected)
    send_notification(&mut stdin, "notifications/initialized");

    // Send "tools/list" request
    let resp = send_request(&mut stdin, &mut stdout, "tools/list", 2, json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 2);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7, "expected 7 tools, got: {}", tools.len());

    // Close stdin and wait for process
    drop(stdin);
    child.wait().unwrap();
}

#[test]
fn test_mcp_tools_call_list_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join(".vibetracer").join("sessions");
    let session_id = "20260325-120000-abcd";
    let session_dir = sessions_dir.join(session_id);
    std::fs::create_dir_all(&session_dir).unwrap();

    // Create meta.json
    let meta = json!({
        "id": session_id,
        "project_path": dir.path().to_str().unwrap(),
        "started_at": 1_700_000_000i64,
        "mode": "enriched",
        "agents": []
    });
    std::fs::write(
        session_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();

    // Create empty edits.jsonl
    std::fs::write(session_dir.join("edits.jsonl"), "").unwrap();

    let mut child = start_mcp_server(dir.path());
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize first
    let _resp = send_request(&mut stdin, &mut stdout, "initialize", 1, json!({}));

    // Call list_sessions tool
    let resp = send_request(
        &mut stdin,
        &mut stdout,
        "tools/call",
        2,
        json!({
            "name": "list_sessions",
            "arguments": {}
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 2);

    // The result.content[0].text is a JSON string we need to parse
    let content = resp["result"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    let inner_text = content[0]["text"].as_str().unwrap();
    let inner: Value = serde_json::from_str(inner_text).unwrap();

    assert_eq!(inner["total_count"], 1);
    let sessions = inner["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], session_id);

    // Close stdin and wait for process
    drop(stdin);
    child.wait().unwrap();
}
