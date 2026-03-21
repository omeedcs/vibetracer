use std::fs;
use std::sync::mpsc;
use std::time::Duration;
use tempfile::tempdir;
use vibetracer::watcher::differ::compute_diff;
use vibetracer::watcher::fs_watcher::FsWatcher;

// ─── Differ tests ─────────────────────────────────────────────────────────────

#[test]
fn test_compute_unified_diff() {
    let old = "line1\nline2\nline3\n";
    let new = "line1\nline2 modified\nline3\nnew line4\n";
    let result = compute_diff(old, new, "test.txt");

    assert!(!result.patch.is_empty(), "Patch should not be empty");
    assert!(
        result.patch.contains('+'),
        "Patch should contain added lines"
    );
    assert!(
        result.patch.contains('-'),
        "Patch should contain removed lines"
    );
    assert!(result.lines_added > 0, "Should have lines added");
    assert!(result.lines_removed > 0, "Should have lines removed");
}

#[test]
fn test_no_diff_when_identical() {
    let content = "same content\nno changes here\n";
    let result = compute_diff(content, content, "test.txt");

    assert!(
        result.patch.is_empty(),
        "Patch should be empty for identical content"
    );
    assert_eq!(result.lines_added, 0, "Should have zero lines added");
    assert_eq!(result.lines_removed, 0, "Should have zero lines removed");
}

// ─── FsWatcher tests ─────────────────────────────────────────────────────────

#[test]
fn test_watcher_detects_file_create() {
    let dir = tempdir().unwrap();
    let (tx, rx) = mpsc::channel::<std::path::PathBuf>();

    let mut watcher = FsWatcher::new(dir.path().to_path_buf(), tx, 50).expect("create watcher");
    watcher.start().expect("start watcher");

    // Give the watcher a moment to initialize
    std::thread::sleep(Duration::from_millis(100));

    // Create a file in the watched directory
    let test_file = dir.path().join("test_file.txt");
    fs::write(&test_file, "hello").expect("write test file");

    // Wait up to 2 seconds for an event
    let received = rx.recv_timeout(Duration::from_secs(2));
    assert!(
        received.is_ok(),
        "Should receive a filesystem event within 2 seconds"
    );

    watcher.stop();
}

#[test]
fn test_watcher_respects_ignore_patterns() {
    let dir = tempdir().unwrap();
    let (tx, rx) = mpsc::channel::<std::path::PathBuf>();

    // Ignore .git directory
    let ignore = vec![".git".to_string()];
    let mut watcher =
        FsWatcher::with_ignore(dir.path().to_path_buf(), tx, 50, ignore).expect("create watcher");
    watcher.start().expect("start watcher");

    // Give the watcher a moment to initialize
    std::thread::sleep(Duration::from_millis(100));

    // Create a file inside the .git directory (should be ignored)
    let git_dir = dir.path().join(".git");
    fs::create_dir_all(&git_dir).expect("create .git dir");
    fs::write(git_dir.join("COMMIT_EDITMSG"), "initial commit").expect("write git file");

    // Create a regular file (should NOT be ignored)
    let regular_file = dir.path().join("regular_file.txt");
    fs::write(&regular_file, "regular content").expect("write regular file");

    // Collect events for up to 2 seconds
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut received_paths = Vec::new();

    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(path) => received_paths.push(path),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // If we have at least one event, we can check
                if !received_paths.is_empty() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    watcher.stop();

    // Ensure no .git paths were received
    for path in &received_paths {
        let has_git = path.components().any(|c| c.as_os_str() == ".git");
        assert!(
            !has_git,
            "Should not receive events for .git paths, got: {:?}",
            path
        );
    }

    // At least one event should have been received (the regular file)
    assert!(
        !received_paths.is_empty(),
        "Should receive at least one event for the regular file"
    );
}
