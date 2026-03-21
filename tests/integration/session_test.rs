use tempfile::tempdir;
use vibetracer::session::{Session, SessionManager};

#[test]
fn test_session_id_format() {
    let id = Session::generate_id();
    // Expected format: YYYYMMDD-HHMMSS-xxxx
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(parts.len(), 3, "ID should have 3 parts separated by '-': got '{}'", id);

    let date_part = parts[0];
    let time_part = parts[1];
    let hex_part = parts[2];

    assert_eq!(date_part.len(), 8, "Date part should be 8 chars (YYYYMMDD): got '{}'", date_part);
    assert!(date_part.chars().all(|c| c.is_ascii_digit()), "Date part should be all digits");

    assert_eq!(time_part.len(), 6, "Time part should be 6 chars (HHMMSS): got '{}'", time_part);
    assert!(time_part.chars().all(|c| c.is_ascii_digit()), "Time part should be all digits");

    assert_eq!(hex_part.len(), 4, "Hex part should be 4 chars: got '{}'", hex_part);
    assert!(
        hex_part.chars().all(|c| c.is_ascii_hexdigit()),
        "Hex part should be valid hex chars: got '{}'",
        hex_part
    );
}

#[test]
fn test_session_create_and_list() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());

    let s1 = manager.create().expect("create session 1");
    let s2 = manager.create().expect("create session 2");

    // Both session directories should exist
    assert!(s1.dir.exists(), "Session 1 dir should exist");
    assert!(s2.dir.exists(), "Session 2 dir should exist");

    let sessions = manager.list().expect("list sessions");
    assert_eq!(sessions.len(), 2, "Should list 2 sessions");
}

#[test]
fn test_session_meta_persists() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());

    let session = manager.create().expect("create session");
    let session_id = session.id.clone();

    let meta = manager.load_meta(&session_id).expect("load meta");
    assert_eq!(meta.id, session_id, "Loaded meta ID should match session ID");
}
