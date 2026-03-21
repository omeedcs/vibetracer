use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::config::{
    BlastRadiusConfig, Config, ManualDependency, PatternSpec, SentinelRule, WatchConfig,
    WatchdogConfig, WatchdogConstant,
};

/// Run a scripted demo session that showcases vibetracer features.
///
/// Creates a temporary project directory, writes initial files, sets up a
/// vibetracer config with watchdog rules and a sentinel, then starts the TUI
/// while a background thread makes timed scripted edits.
pub fn run_demo() -> Result<()> {
    // ── create temp project directory ─────────────────────────────────────────
    let project_path: PathBuf = std::env::temp_dir().join(format!(
        "vibetracer-demo-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    let src_dir = project_path.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // ── write initial file contents ───────────────────────────────────────────
    let physics_path = src_dir.join("physics.py");
    let config_path = src_dir.join("config.py");
    let model_path = src_dir.join("model.py");

    std::fs::write(
        &physics_path,
        r#"# Gravitational simulation
# @eq: F = G * m1 * m2 / r^2

GRAVITY_CONSTANT = 6.674e-11
EARTH_RADIUS_KM = 6371.0
SPEED_OF_LIGHT = 299792.458

def gravitational_force(m1, m2, r):
    return GRAVITY_CONSTANT * (m1 * m2) / r**2
"#,
    )?;

    std::fs::write(
        &config_path,
        r#"N_FEATURES = 3
TEMPORAL_FEATURES = ["snr", "latency", "loss"]
"#,
    )?;

    std::fs::write(
        &model_path,
        r#"class Predictor:
    def __init__(self):
        self.input_size = 3
"#,
    )?;

    // ── build vibetracer config ────────────────────────────────────────────────
    let mut sentinels = HashMap::new();
    sentinels.insert(
        "feature_count_sync".to_string(),
        SentinelRule {
            description: "N_FEATURES must equal len(TEMPORAL_FEATURES)".to_string(),
            watch: "src/config.py".to_string(),
            rule: "count_match".to_string(),
            pattern_a: Some(PatternSpec {
                file: "src/config.py".to_string(),
                regex: r#"N_FEATURES\s*=\s*(\d+)"#.to_string(),
            }),
            pattern_b: Some(PatternSpec {
                file: "src/config.py".to_string(),
                regex: r#""[^"]+""#.to_string(),
            }),
            assert: Some("a == b".to_string()),
        },
    );
    sentinels.insert(
        "model_input_size".to_string(),
        SentinelRule {
            description: "Predictor.input_size must match N_FEATURES".to_string(),
            watch: "src/model.py".to_string(),
            rule: "count_match".to_string(),
            pattern_a: Some(PatternSpec {
                file: "src/config.py".to_string(),
                regex: r#"N_FEATURES\s*=\s*(\d+)"#.to_string(),
            }),
            pattern_b: Some(PatternSpec {
                file: "src/model.py".to_string(),
                regex: r#"input_size\s*=\s*(\d+)"#.to_string(),
            }),
            assert: Some("a == b".to_string()),
        },
    );

    let config = Config {
        watch: WatchConfig {
            debounce_ms: 80,
            ignore: vec![
                ".git".to_string(),
                ".vibetracer".to_string(),
                "__pycache__".to_string(),
            ],
            auto_checkpoint_every: 10,
        },
        sentinels,
        watchdog: WatchdogConfig {
            constants: vec![
                WatchdogConstant {
                    file: "src/physics.py".to_string(),
                    pattern: r"EARTH_RADIUS_KM\s*=\s*[\d.]+".to_string(),
                    expected: "6371.0".to_string(),
                    severity: "warning".to_string(),
                },
                WatchdogConstant {
                    file: "src/physics.py".to_string(),
                    pattern: r"SPEED_OF_LIGHT\s*=\s*[\d.]+".to_string(),
                    expected: "299792.458".to_string(),
                    severity: "error".to_string(),
                },
            ],
        },
        blast_radius: BlastRadiusConfig {
            auto_detect: true,
            manual: vec![ManualDependency {
                source: "src/config.py".to_string(),
                dependents: vec!["src/model.py".to_string()],
            }],
        },
        theme: crate::config::ThemeConfig::default(),
    };

    // Write the config to .vibetracer/config.toml inside the temp project.
    let vt_dir = project_path.join(".vibetracer");
    std::fs::create_dir_all(&vt_dir)?;
    config.save(vt_dir.join("config.toml"))?;

    // ── spawn background thread for scripted edits ────────────────────────────
    let physics_path_bg = physics_path.clone();
    let config_path_bg = config_path.clone();
    let model_path_bg = model_path.clone();

    thread::spawn(move || {
        // Wait for TUI to initialize.
        thread::sleep(Duration::from_secs(2));

        // Edit 1: change EARTH_RADIUS_KM — triggers watchdog.
        let _ = std::fs::write(
            &physics_path_bg,
            r#"# Gravitational simulation
# @eq: F = G * m1 * m2 / r^2

GRAVITY_CONSTANT = 6.674e-11
EARTH_RADIUS_KM = 6400.0
SPEED_OF_LIGHT = 299792.458

def gravitational_force(m1, m2, r):
    return GRAVITY_CONSTANT * (m1 * m2) / r**2
"#,
        );

        thread::sleep(Duration::from_secs(2));

        // Edit 2: add "humidity" to features and bump N_FEATURES to 4 in config.py.
        let _ = std::fs::write(
            &config_path_bg,
            r#"N_FEATURES = 4
TEMPORAL_FEATURES = ["snr", "latency", "loss", "humidity"]
"#,
        );

        thread::sleep(Duration::from_secs(2));

        // Edit 3: touch model.py WITHOUT updating input_size — triggers sentinel.
        let _ = std::fs::write(
            &model_path_bg,
            r#"class Predictor:
    def __init__(self):
        self.input_size = 3  # TODO: update this

    def predict(self, x):
        return x[:self.input_size]
"#,
        );

        thread::sleep(Duration::from_secs(2));

        // Edit 4: add a new function with an equation comment to physics.py.
        let _ = std::fs::write(
            &physics_path_bg,
            r#"# Gravitational simulation
# @eq: F = G * m1 * m2 / r^2

GRAVITY_CONSTANT = 6.674e-11
EARTH_RADIUS_KM = 6400.0
SPEED_OF_LIGHT = 299792.458

def gravitational_force(m1, m2, r):
    return GRAVITY_CONSTANT * (m1 * m2) / r**2

# @eq: E = m * c^2
def rest_energy(mass):
    """Mass-energy equivalence."""
    c = SPEED_OF_LIGHT * 1000  # convert km/s to m/s
    return mass * c**2
"#,
        );

        thread::sleep(Duration::from_secs(2));

        // Edit 5: fix model.py — update input_size to 4 (sentinel clears).
        let _ = std::fs::write(
            &model_path_bg,
            r#"class Predictor:
    def __init__(self):
        self.input_size = 4

    def predict(self, x):
        return x[:self.input_size]
"#,
        );
    });

    // ── run the TUI on the main thread ────────────────────────────────────────
    let result = crate::tui::run_tui(project_path.clone(), config);

    // Clean up the temporary project directory.
    let _ = std::fs::remove_dir_all(&project_path);

    result
}
