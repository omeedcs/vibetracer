use tempfile::tempdir;
use vibetracer::analysis::refactor_tracker::RefactorTracker;

#[test]
fn test_detect_rename() {
    let result = RefactorTracker::detect_rename(
        "fn check_auth(user: &str) -> bool {",
        "fn validate_token(user: &str) -> bool {",
    );

    assert!(result.is_some(), "expected a rename to be detected");
    let (old, new) = result.unwrap();
    assert_eq!(old, "check_auth");
    assert_eq!(new, "validate_token");
}

#[test]
fn test_track_propagation() {
    let dir = tempdir().unwrap();

    // File 1: still uses the old name
    std::fs::write(
        dir.path().join("file1.rs"),
        "let result = check_auth(user);\n",
    )
    .unwrap();

    // File 2: still uses the old name
    std::fs::write(
        dir.path().join("file2.rs"),
        "if check_auth(admin) { return; }\n",
    )
    .unwrap();

    // File 3: already uses the new name
    std::fs::write(
        dir.path().join("file3.rs"),
        "fn validate_token(user: &str) -> bool { true }\n",
    )
    .unwrap();

    let mut tracker = RefactorTracker::new();
    tracker.track_rename(dir.path(), "check_auth", "validate_token");

    let status = tracker.get_status("check_auth").expect("status should exist");

    assert_eq!(status.old_name, "check_auth");
    assert_eq!(status.new_name, "validate_token");
    assert_eq!(status.remaining_old_refs, 2, "2 old references remain");
    assert_eq!(status.updated_new_refs, 1, "1 new reference found");
    assert_eq!(status.total_sites, 3);
    assert_eq!(
        status.remaining_files.len(),
        2,
        "2 files still contain the old name"
    );
}
