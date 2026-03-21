use std::collections::HashSet;
use vibetracer::analysis::blast_radius::BlastRadiusTracker;
use vibetracer::config::{BlastRadiusConfig, ManualDependency};

fn make_config() -> BlastRadiusConfig {
    BlastRadiusConfig {
        auto_detect: false,
        manual: vec![ManualDependency {
            source: "src/core.rs".to_string(),
            dependents: vec![
                "src/api.rs".to_string(),
                "src/cli.rs".to_string(),
                "src/tui.rs".to_string(),
            ],
        }],
    }
}

#[test]
fn test_manual_blast_radius() {
    let tracker = BlastRadiusTracker::new(make_config());

    let deps = tracker.get_dependents("src/core.rs");
    assert_eq!(deps.len(), 3);
    assert!(deps.contains(&"src/api.rs".to_string()));
    assert!(deps.contains(&"src/cli.rs".to_string()));
    assert!(deps.contains(&"src/tui.rs".to_string()));
}

#[test]
fn test_blast_radius_stale_detection() {
    let tracker = BlastRadiusTracker::new(make_config());

    // source + 1 of 3 dependents were edited
    let edited: HashSet<String> = ["src/core.rs".to_string(), "src/api.rs".to_string()]
        .into_iter()
        .collect();

    let status = tracker.check_staleness("src/core.rs", &edited);

    assert_eq!(status.updated.len(), 1, "expected 1 updated dependent");
    assert!(status.updated.contains(&"src/api.rs".to_string()));

    assert_eq!(status.stale.len(), 2, "expected 2 stale dependents");
    assert!(status.stale.contains(&"src/cli.rs".to_string()));
    assert!(status.stale.contains(&"src/tui.rs".to_string()));
}
