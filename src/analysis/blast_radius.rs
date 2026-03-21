use crate::config::BlastRadiusConfig;
use glob::Pattern;
use std::collections::{HashMap, HashSet};

/// Result of a staleness check for a source file's dependents.
#[derive(Debug, Clone)]
pub struct DependencyStatus {
    /// Dependent files that were also edited.
    pub updated: Vec<String>,
    /// Dependent files that were NOT edited (potentially stale).
    pub stale: Vec<String>,
    /// Files with no declared dependency on the source.
    pub untouched: Vec<String>,
}

/// Tracks blast radius for edited files using manual dependency declarations.
pub struct BlastRadiusTracker {
    config: BlastRadiusConfig,
    /// Pre-built lookup: source → dependents.
    manual_deps: HashMap<String, Vec<String>>,
}

impl BlastRadiusTracker {
    pub fn new(config: BlastRadiusConfig) -> Self {
        let mut manual_deps: HashMap<String, Vec<String>> = HashMap::new();
        for dep in &config.manual {
            manual_deps
                .entry(dep.source.clone())
                .or_default()
                .extend(dep.dependents.iter().cloned());
        }
        Self { config, manual_deps }
    }

    /// Return all declared dependents of `source` (exact match or glob).
    pub fn get_dependents(&self, source: &str) -> Vec<String> {
        let mut result = Vec::new();

        for dep in &self.config.manual {
            let matches = if dep.source.contains('*') || dep.source.contains('?') {
                Pattern::new(&dep.source)
                    .map(|p| p.matches(source))
                    .unwrap_or(false)
            } else {
                dep.source == source
            };

            if matches {
                result.extend(dep.dependents.iter().cloned());
            }
        }

        result
    }

    /// For each dependent of `source`, determine whether it was in `edited_files`.
    pub fn check_staleness(
        &self,
        source: &str,
        edited_files: &HashSet<String>,
    ) -> DependencyStatus {
        let dependents = self.get_dependents(source);
        let mut updated = Vec::new();
        let mut stale = Vec::new();

        for dep in &dependents {
            if edited_files.contains(dep) {
                updated.push(dep.clone());
            } else {
                stale.push(dep.clone());
            }
        }

        // untouched = edited_files that are NOT dependents of source and not source itself
        let dep_set: HashSet<&str> = dependents.iter().map(|s| s.as_str()).collect();
        let untouched = edited_files
            .iter()
            .filter(|f| !dep_set.contains(f.as_str()) && f.as_str() != source)
            .cloned()
            .collect();

        DependencyStatus {
            updated,
            stale,
            untouched,
        }
    }
}
