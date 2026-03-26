use tempfile::tempdir;
use vibetracer::export::agent_trace::export_agent_trace;
use vibetracer::import::AgentImporter;
use vibetracer::import::agent_trace::AgentTraceImporter;

#[test]
fn agent_trace_roundtrip() {
    let tmp = tempdir().unwrap();
    let trace_dir = tmp.path().join(".agent-trace");
    std::fs::create_dir(&trace_dir).unwrap();

    let original = r#"{
        "version": "0.1",
        "contributions": [
            {
                "agent": "cursor",
                "model": "gpt-4",
                "timestamp": "2026-03-20T10:00:00Z",
                "file": "src/main.rs",
                "before": "fn main() {}",
                "after": "fn main() {\n    println!(\"hello\");\n}",
                "reasoning": "Add hello world print",
                "operation_id": "op-1"
            },
            {
                "agent": "cursor",
                "model": "gpt-4",
                "timestamp": "2026-03-20T10:01:00Z",
                "file": "src/lib.rs",
                "before": "",
                "after": "pub fn greet() -> &'static str { \"hello\" }",
                "reasoning": "Add greet function",
                "operation_id": "op-2"
            }
        ]
    }"#;
    std::fs::write(trace_dir.join("session.json"), original).unwrap();

    // Import
    let importer = AgentTraceImporter::new();
    let events = importer.import_edits(&trace_dir, tmp.path()).unwrap();
    assert_eq!(events.len(), 2);

    // Export back to Agent Trace
    let exported_json = export_agent_trace(&events, "test-session").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&exported_json).unwrap();

    // Verify roundtrip fidelity
    let contribs = parsed["contributions"].as_array().unwrap();
    assert_eq!(contribs.len(), 2);
    assert_eq!(contribs[0]["file"], "src/main.rs");
    assert_eq!(contribs[0]["agent"], "cursor");
    assert_eq!(contribs[0]["reasoning"], "Add hello world print");
    assert_eq!(contribs[0]["operation_id"], "op-1");
    assert_eq!(contribs[1]["file"], "src/lib.rs");
    assert_eq!(contribs[1]["operation_id"], "op-2");

    // Verify generator tag
    assert_eq!(parsed["generator"], "vibetracer");
    assert_eq!(parsed["version"], "0.1");
}

#[test]
fn claude_import_then_agent_trace_export() {
    use vibetracer::event::{EditEvent, EditKind};

    let claude_events = vec![EditEvent {
        id: 1,
        ts: 1_711_000_000_000,
        file: "src/main.rs".to_string(),
        kind: EditKind::Modify,
        patch: "@@ -1 +1 @@\n-old\n+new".to_string(),
        before_hash: Some("abc".to_string()),
        after_hash: "def".to_string(),
        intent: Some("fix startup bug".to_string()),
        tool: Some("claude-code".to_string()),
        lines_added: 1,
        lines_removed: 1,
        agent_id: Some("session-uuid-123".to_string()),
        agent_label: Some("claude-code-1".to_string()),
        operation_id: Some("msg-5".to_string()),
        operation_intent: Some("fix startup bug".to_string()),
        tool_name: Some("Edit".to_string()),
        restore_id: None,
    }];

    let json = export_agent_trace(&claude_events, "test").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let contrib = &parsed["contributions"][0];
    assert_eq!(contrib["file"], "src/main.rs");
    assert_eq!(contrib["agent"], "session-uuid-123");
    assert_eq!(contrib["reasoning"], "fix startup bug");
    assert!(contrib["diff"].as_str().unwrap().contains("+new"));
}
