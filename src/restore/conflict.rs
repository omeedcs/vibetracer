use crate::config::BlastRadiusConfig;
use std::collections::HashSet;

/// A suggested additional file that should be included in a restore to maintain
/// consistency across coupled files.
pub struct ConflictSuggestion {
    pub coupled_files: Vec<String>,
    pub reason: String,
}

/// Checks blast radius config for coupled files that might be left inconsistent
/// after a partial restore.
pub struct ConflictChecker {
    config: BlastRadiusConfig,
}

impl ConflictChecker {
    pub fn new(config: BlastRadiusConfig) -> Self {
        Self { config }
    }

    /// Check if restoring `files_to_restore` would leave coupled files in an
    /// inconsistent state.
    ///
    /// For each manual dependency rule where the source file is being restored,
    /// check if any dependents were also edited but are NOT included in the
    /// restore set.  Those missing dependents are returned as suggestions.
    pub fn check_restore_conflicts(
        &self,
        files_to_restore: &[String],
        edited_files: &HashSet<String>,
    ) -> Vec<ConflictSuggestion> {
        let restore_set: HashSet<&str> =
            files_to_restore.iter().map(String::as_str).collect();

        let mut suggestions = Vec::new();

        for dep in &self.config.manual {
            // Only act if the source file is being restored.
            if !restore_set.contains(dep.source.as_str()) {
                continue;
            }

            // Find dependents that were edited but are not being restored.
            let missing: Vec<String> = dep
                .dependents
                .iter()
                .filter(|d| {
                    edited_files.contains(d.as_str()) && !restore_set.contains(d.as_str())
                })
                .cloned()
                .collect();

            if !missing.is_empty() {
                suggestions.push(ConflictSuggestion {
                    coupled_files: missing.clone(),
                    reason: format!(
                        "'{}' depends on '{}' which is being restored; \
                         also consider restoring: {}",
                        missing.join(", "),
                        dep.source,
                        missing.join(", ")
                    ),
                });
            }
        }

        suggestions
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BlastRadiusConfig, ManualDependency};

    fn config_with_deps(deps: Vec<(&str, Vec<&str>)>) -> BlastRadiusConfig {
        BlastRadiusConfig {
            auto_detect: false,
            manual: deps
                .into_iter()
                .map(|(src, dependents)| ManualDependency {
                    source: src.to_string(),
                    dependents: dependents.into_iter().map(str::to_string).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn test_no_conflicts_when_all_coupled_files_restored() {
        let config = config_with_deps(vec![("api.rs", vec!["client.rs", "tests.rs"])]);
        let checker = ConflictChecker::new(config);

        let files_to_restore = vec![
            "api.rs".to_string(),
            "client.rs".to_string(),
            "tests.rs".to_string(),
        ];
        let edited: HashSet<String> =
            ["api.rs", "client.rs", "tests.rs"].iter().map(|s| s.to_string()).collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_conflict_detected_when_dependent_not_restored() {
        let config = config_with_deps(vec![("schema.rs", vec!["migration.rs", "model.rs"])]);
        let checker = ConflictChecker::new(config);

        // Restore only the source; dependents were edited but not included
        let files_to_restore = vec!["schema.rs".to_string()];
        let edited: HashSet<String> =
            ["schema.rs", "migration.rs", "model.rs"].iter().map(|s| s.to_string()).collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert_eq!(suggestions.len(), 1);

        let coupled = &suggestions[0].coupled_files;
        assert!(coupled.contains(&"migration.rs".to_string()));
        assert!(coupled.contains(&"model.rs".to_string()));
    }

    #[test]
    fn test_no_suggestion_for_unedited_dependent() {
        let config = config_with_deps(vec![("a.rs", vec!["b.rs", "c.rs"])]);
        let checker = ConflictChecker::new(config);

        let files_to_restore = vec!["a.rs".to_string()];
        // b.rs was edited, c.rs was not
        let edited: HashSet<String> = ["a.rs", "b.rs"].iter().map(|s| s.to_string()).collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert_eq!(suggestions.len(), 1);
        // Only b.rs should be flagged (c.rs was never edited)
        assert_eq!(suggestions[0].coupled_files, vec!["b.rs".to_string()]);
    }

    #[test]
    fn test_no_conflicts_when_source_not_being_restored() {
        let config = config_with_deps(vec![("source.rs", vec!["dep.rs"])]);
        let checker = ConflictChecker::new(config);

        // Restoring something else entirely
        let files_to_restore = vec!["other.rs".to_string()];
        let edited: HashSet<String> =
            ["source.rs", "dep.rs"].iter().map(|s| s.to_string()).collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_empty_config_no_conflicts() {
        let config = BlastRadiusConfig::default();
        let checker = ConflictChecker::new(config);

        let files_to_restore = vec!["anything.rs".to_string()];
        let edited: HashSet<String> = ["anything.rs", "other.rs"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_multiple_dependency_rules() {
        let config = config_with_deps(vec![
            ("a.rs", vec!["b.rs"]),
            ("c.rs", vec!["d.rs"]),
        ]);
        let checker = ConflictChecker::new(config);

        // Restoring both sources but missing both dependents
        let files_to_restore = vec!["a.rs".to_string(), "c.rs".to_string()];
        let edited: HashSet<String> =
            ["a.rs", "b.rs", "c.rs", "d.rs"].iter().map(|s| s.to_string()).collect();

        let suggestions = checker.check_restore_conflicts(&files_to_restore, &edited);
        assert_eq!(suggestions.len(), 2);
    }
}
