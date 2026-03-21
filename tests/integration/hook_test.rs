use tempfile::tempdir;
use vibetracer::hook::registration::{register_hook, unregister_hook};

#[test]
fn test_register_hook_creates_settings() {
    let tmp = tempdir().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let socket_path = "/tmp/vibetracer_test.sock";

    register_hook(&claude_dir, socket_path).expect("register hook");

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
    assert!(
        contents.contains(socket_path),
        "settings should contain the socket path"
    );
}

#[test]
fn test_unregister_hook_removes_entry() {
    let tmp = tempdir().unwrap();
    let claude_dir = tmp.path().join(".claude");
    let socket_path = "/tmp/vibetracer_test2.sock";

    register_hook(&claude_dir, socket_path).expect("register hook");
    unregister_hook(&claude_dir).expect("unregister hook");

    let settings_path = claude_dir.join("settings.local.json");
    assert!(settings_path.exists(), "settings file should still exist");

    let contents = std::fs::read_to_string(&settings_path).unwrap();
    assert!(
        !contents.contains("vibetracer"),
        "settings should not contain 'vibetracer' after unregistration"
    );
}
