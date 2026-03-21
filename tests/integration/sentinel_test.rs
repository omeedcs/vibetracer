use tempfile::tempdir;
use vibetracer::analysis::sentinels::SentinelEngine;
use vibetracer::config::{PatternSpec, SentinelRule};

fn make_rule(description: &str, file_a: &str, file_b: &str, assertion: &str) -> SentinelRule {
    SentinelRule {
        description: description.to_string(),
        watch: "*".to_string(),
        rule: "grep_match".to_string(),
        pattern_a: Some(PatternSpec {
            file: file_a.to_string(),
            regex: r"VERSION\s*=\s*(\d+)".to_string(),
        }),
        pattern_b: Some(PatternSpec {
            file: file_b.to_string(),
            regex: r"VERSION\s*=\s*(\d+)".to_string(),
        }),
        assert: Some(assertion.to_string()),
    }
}

#[test]
fn test_grep_match_sentinel_passes() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.py"), "VERSION = 4\n").unwrap();
    std::fs::write(dir.path().join("b.py"), "VERSION = 4\n").unwrap();

    let engine = SentinelEngine::new(dir.path().to_path_buf());
    let rule = make_rule("versions must match", "a.py", "b.py", "a == b");

    let violations = engine.evaluate("version_sync", &rule);
    assert!(
        violations.is_empty(),
        "expected no violations when values match"
    );
}

#[test]
fn test_grep_match_sentinel_fails() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.py"), "VERSION = 4\n").unwrap();
    std::fs::write(dir.path().join("b.py"), "VERSION = 3\n").unwrap();

    let engine = SentinelEngine::new(dir.path().to_path_buf());
    let rule = make_rule("versions must match", "a.py", "b.py", "a == b");

    let violations = engine.evaluate("version_sync", &rule);
    assert_eq!(
        violations.len(),
        1,
        "expected one violation when values differ"
    );
    let v = &violations[0];
    assert_eq!(v.value_a, "4");
    assert_eq!(v.value_b, "3");
    assert_eq!(v.assertion, "a == b");
}
