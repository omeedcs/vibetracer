use crate::config::WatchdogConstant;
use glob::Pattern;
use regex::Regex;

/// An alert raised when a watched constant drifts from its expected value.
#[derive(Debug, Clone)]
pub struct WatchdogAlert {
    pub constant_pattern: String,
    pub expected: String,
    pub actual: String,
    pub severity: String,
    pub file: String,
}

/// Watches a set of constants and raises alerts when they change unexpectedly.
pub struct Watchdog {
    pub rules: Vec<WatchdogConstant>,
}

impl Watchdog {
    pub fn new(rules: Vec<WatchdogConstant>) -> Self {
        Self { rules }
    }

    /// Check `filename` for constant drift between `old_content` and `new_content`.
    /// Returns one `WatchdogAlert` per rule where the value changed AND differs from expected.
    pub fn check(
        &self,
        filename: &str,
        old_content: &str,
        new_content: &str,
    ) -> Vec<WatchdogAlert> {
        let mut alerts = Vec::new();

        for rule in &self.rules {
            // Check whether the filename matches the rule's file glob.
            let matches = if rule.file.contains('*') || rule.file.contains('?') {
                Pattern::new(&rule.file)
                    .map(|p| p.matches(filename))
                    .unwrap_or(false)
            } else {
                filename == rule.file || filename.ends_with(&rule.file)
            };

            if !matches {
                continue;
            }

            let re = match Regex::new(&rule.pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let old_val = re
                .captures(old_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            let new_val = re
                .captures(new_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            // Alert if the value changed AND the new value differs from expected.
            if old_val != new_val {
                if let Some(actual) = &new_val {
                    if *actual != rule.expected {
                        alerts.push(WatchdogAlert {
                            constant_pattern: rule.pattern.clone(),
                            expected: rule.expected.clone(),
                            actual: actual.clone(),
                            severity: rule.severity.clone(),
                            file: filename.to_string(),
                        });
                    }
                }
            }
        }

        alerts
    }
}
