use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::tui::alerts::AlertConfig;

/// Filesystem watch configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
    /// Paths/patterns to ignore.
    pub ignore: Vec<String>,
    /// Auto-checkpoint after this many edits (0 = disabled).
    pub auto_checkpoint_every: u32,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 100,
            ignore: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".vibetracer".to_string(),
            ],
            auto_checkpoint_every: 25,
        }
    }
}

/// A file + regex pair used in sentinel rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSpec {
    pub file: String,
    pub regex: String,
}

/// A sentinel rule that watches for a condition in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelRule {
    pub description: String,
    pub watch: String,
    pub rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_a: Option<PatternSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_b: Option<PatternSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assert: Option<String>,
}

/// A constant that must always match an expected pattern count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConstant {
    pub file: String,
    pub pattern: String,
    pub expected: String,
    pub severity: String,
}

/// Watchdog configuration: constants that must never drift.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WatchdogConfig {
    pub constants: Vec<WatchdogConstant>,
}

/// A manually declared dependency relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualDependency {
    pub source: String,
    pub dependents: Vec<String>,
}

/// Blast-radius detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BlastRadiusConfig {
    pub auto_detect: bool,
    pub manual: Vec<ManualDependency>,
}

impl Default for BlastRadiusConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            manual: Vec::new(),
        }
    }
}

/// Color theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Which color preset to use: "dark", "catppuccin", "gruvbox", or "light".
    pub preset: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            preset: "dark".into(),
        }
    }
}

/// Top-level vibetracer configuration, loaded from `.vibetracer/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub watch: WatchConfig,
    pub sentinels: HashMap<String, SentinelRule>,
    pub watchdog: WatchdogConfig,
    pub blast_radius: BlastRadiusConfig,
    pub theme: ThemeConfig,
    #[serde(default)]
    pub alerts: Vec<AlertConfig>,
}

impl Config {
    /// Load a `Config` from a TOML file at `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Serialize this `Config` as TOML and write it to `path`.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}
