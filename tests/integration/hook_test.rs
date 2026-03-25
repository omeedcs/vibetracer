use tempfile::tempdir;
use vibetracer::hook::registration::{register_hook, unregister_hook};

#[test]
fn test_register_hook_creates_settings() {
    let tmp = tempdir().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let project_path = tmp.path();

    register_hook(&claude_dir, project_path).expect("register hook");

    let settings_path = claude_dir.join("settings.local.json");
    assert!(
        settings_path.exists(),
        "settings.local.json should be created"
    );

    let contents = std::fs::read_to_string(&settings_path).unwrap();
    assert!(
        contents.contains("PostToolUse"),
        "settings should contain PostToolUse matcher"
    );
    // The socket path is derived from project_path/.vibetracer/daemon.sock.
    let expected_sock = project_path
        .join(".vibetracer")
        .join("daemon.sock")
        .to_string_lossy()
        .into_owned();
    assert!(
        contents.contains(&expected_sock),
        "settings should contain the derived daemon socket path"
    );
}

#[test]
fn test_unregister_hook_removes_entry() {
    let tmp = tempdir().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let project_path = tmp.path();

    register_hook(&claude_dir, project_path).expect("register hook");
    unregister_hook(&claude_dir).expect("unregister hook");

    let settings_path = claude_dir.join("settings.local.json");
    assert!(settings_path.exists(), "settings file should still exist");

    let contents = std::fs::read_to_string(&settings_path).unwrap();
    assert!(
        !contents.contains("vibetracer"),
        "settings should not contain 'vibetracer' after unregistration"
    );
}
