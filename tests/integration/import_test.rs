use std::io::Write;
use tempfile::tempdir;
use vibetracer::import::claude::{import_session, list_sessions};

// ─── helpers ──────────────────────────────────────────────────────────────────

fn write_jsonl(dir: &std::path::Path, name: &str, lines: &[&str]) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    for line in lines {
        writeln!(f, "{}", line).unwrap();
    }
    path
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_parse_edit_tool_use() {
    let tmp = tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    let file_path = project_root.join("src").join("main.py");
    let file_path_str = file_path.to_string_lossy();

    let assistant_line = format!(
        r#"{{"type":"assistant","timestamp":"2026-03-20T21:51:29.334Z","message":{{"content":[{{"type":"tool_use","name":"Edit","input":{{"file_path":"{file_path}","old_string":"original code","new_string":"modified code"}}}}]}}}}"#,
        file_path = file_path_str
    );

    let result_line = format!(
        r#"{{"type":"user","timestamp":"2026-03-20T21:51:30.000Z","toolUseResult":{{"type":"modify","filePath":"{file_path}","content":"modified code","originalFile":"original code"}}}}"#,
        file_path = file_path_str
    );

    let jsonl_path = write_jsonl(
        tmp.path(),
        "session.jsonl",
        &[&assistant_line, &result_line],
    );

    let events = import_session(&jsonl_path, &project_root).expect("import should succeed");

    assert_eq!(events.len(), 1, "Expected exactly 1 edit event");

    let ev = &events[0];
    assert_eq!(ev.tool.as_deref(), Some("Edit"));
    // The file should be relative to project_root
    assert!(
        ev.file.contains("main.py"),
        "Expected file to contain 'main.py', got: {}",
        ev.file
    );
    // Should have a non-empty patch since old != new
    assert!(
        !ev.patch.is_empty(),
        "Expected non-empty patch for Edit with different strings"
    );
}

#[test]
fn test_parse_write_tool_use() {
    let tmp = tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    let file_path = project_root.join("new_file.rs");
    let file_path_str = file_path.to_string_lossy();

    let assistant_line = format!(
        r#"{{"type":"assistant","timestamp":"2026-03-20T22:00:00.000Z","message":{{"content":[{{"type":"tool_use","name":"Write","input":{{"file_path":"{file_path}","content":"fn main() {{}}\n"}}}}]}}}}"#,
        file_path = file_path_str
    );

    let result_line = format!(
        r#"{{"type":"user","timestamp":"2026-03-20T22:00:01.000Z","toolUseResult":{{"type":"create","filePath":"{file_path}","content":"fn main() {{}}\n","originalFile":null}}}}"#,
        file_path = file_path_str
    );

    let jsonl_path = write_jsonl(
        tmp.path(),
        "session2.jsonl",
        &[&assistant_line, &result_line],
    );

    let events = import_session(&jsonl_path, &project_root).expect("import should succeed");

    assert_eq!(events.len(), 1, "Expected exactly 1 edit event");

    let ev = &events[0];
    assert_eq!(ev.tool.as_deref(), Some("Write"));
    assert!(
        ev.file.contains("new_file.rs"),
        "Expected file to contain 'new_file.rs', got: {}",
        ev.file
    );
    // Write with no original content — patch should add lines
    assert!(ev.lines_added > 0, "Expected lines_added > 0 for Write");
}

#[test]
fn test_list_sessions_empty() {
    let tmp = tempdir().unwrap();
    // Use a path that won't have a matching ~/.claude/projects/ directory
    let fake_project = tmp.path().join("some-random-nonexistent-project-xyz");
    let result = list_sessions(&fake_project).expect("list_sessions should not error");
    assert!(result.is_empty(), "Expected empty sessions list");
}

#[test]
fn test_import_empty_jsonl() {
    let tmp = tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    let jsonl_path = write_jsonl(tmp.path(), "empty.jsonl", &[]);

    let events = import_session(&jsonl_path, &project_root).expect("import empty should succeed");
    assert!(events.is_empty(), "Expected no events from empty JSONL");
}

#[test]
fn test_import_multiple_edits_sorted_by_timestamp() {
    let tmp = tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    let file_a = project_root.join("a.py");
    let file_b = project_root.join("b.py");
    let file_a_str = file_a.to_string_lossy();
    let file_b_str = file_b.to_string_lossy();

    // Second edit has an earlier timestamp than the first line
    let line1 = format!(
        r#"{{"type":"assistant","timestamp":"2026-03-20T22:00:00.000Z","message":{{"content":[{{"type":"tool_use","name":"Edit","input":{{"file_path":"{f}","old_string":"x","new_string":"y"}}}}]}}}}"#,
        f = file_b_str
    );
    let line2 = format!(
        r#"{{"type":"assistant","timestamp":"2026-03-20T21:00:00.000Z","message":{{"content":[{{"type":"tool_use","name":"Edit","input":{{"file_path":"{f}","old_string":"a","new_string":"b"}}}}]}}}}"#,
        f = file_a_str
    );

    let jsonl_path = write_jsonl(tmp.path(), "multi.jsonl", &[&line1, &line2]);

    let events = import_session(&jsonl_path, &project_root).expect("import should succeed");
    assert_eq!(events.len(), 2);

    // Should be sorted by timestamp: file_a (21:00) before file_b (22:00)
    assert!(
        events[0].ts <= events[1].ts,
        "Events should be sorted by timestamp"
    );
    assert!(
        events[0].file.contains("a.py"),
        "First event should be a.py (earlier timestamp)"
    );
}
