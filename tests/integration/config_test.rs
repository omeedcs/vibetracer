use vibetracer::config::{
    BlastRadiusConfig, Config, ManualDependency, PatternSpec, SentinelRule, WatchConfig,
    WatchdogConfig, WatchdogConstant,
};

#[test]
fn test_default_config() {
    let config = Config::default();

    // WatchConfig defaults
    assert_eq!(config.watch.debounce_ms, 100);
    assert_eq!(
        config.watch.ignore,
        vec![".git", "node_modules", "target", "__pycache__", ".vibetracer"]
    );
    assert_eq!(config.watch.auto_checkpoint_every, 25);

    // sentinels default to empty
    assert!(config.sentinels.is_empty());

    // watchdog default: no constants
    assert!(config.watchdog.constants.is_empty());

    // blast_radius defaults
    assert!(config.blast_radius.auto_detect);
    assert!(config.blast_radius.manual.is_empty());
}

#[test]
fn test_config_from_toml() {
    let toml_str = r#"
[watch]
debounce_ms = 200
ignore = [".git", "target"]
auto_checkpoint_every = 10

[[watchdog.constants]]
file = "src/main.rs"
pattern = "fn main"
expected = "1"
severity = "error"

[[watchdog.constants]]
file = "Cargo.toml"
pattern = "version"
expected = "1"
severity = "warning"

[blast_radius]
auto_detect = false

[[blast_radius.manual]]
source = "src/lib.rs"
dependents = ["src/main.rs", "src/config.rs"]
"#;

    let config: Config = toml::from_str(toml_str).expect("Failed to parse TOML");

    assert_eq!(config.watch.debounce_ms, 200);
    assert_eq!(config.watch.ignore, vec![".git", "target"]);
    assert_eq!(config.watch.auto_checkpoint_every, 10);

    assert_eq!(config.watchdog.constants.len(), 2);
    assert_eq!(config.watchdog.constants[0].file, "src/main.rs");
    assert_eq!(config.watchdog.constants[0].pattern, "fn main");
    assert_eq!(config.watchdog.constants[0].expected, "1");
    assert_eq!(config.watchdog.constants[0].severity, "error");

    assert!(!config.blast_radius.auto_detect);
    assert_eq!(config.blast_radius.manual.len(), 1);
    assert_eq!(config.blast_radius.manual[0].source, "src/lib.rs");
    assert_eq!(
        config.blast_radius.manual[0].dependents,
        vec!["src/main.rs", "src/config.rs"]
    );
}

#[test]
fn test_config_generates_default_toml() {
    let config = Config::default();
    let toml_str = toml::to_string(&config).expect("Failed to serialize to TOML");

    // Verify the serialized string contains expected keys
    assert!(toml_str.contains("debounce_ms"));
    assert!(toml_str.contains("auto_checkpoint_every"));
    assert!(toml_str.contains("auto_detect"));

    // Verify it round-trips
    let config2: Config = toml::from_str(&toml_str).expect("Failed to re-parse TOML");
    assert_eq!(config2.watch.debounce_ms, 100);
    assert_eq!(config2.watch.auto_checkpoint_every, 25);
    assert!(config2.blast_radius.auto_detect);
}
