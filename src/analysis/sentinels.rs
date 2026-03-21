use crate::config::{PatternSpec, SentinelRule};
use regex::Regex;
use std::path::PathBuf;

/// A violation raised when an invariant sentinel rule is broken.
#[derive(Debug, Clone)]
pub struct SentinelViolation {
    pub rule_name: String,
    pub description: String,
    pub value_a: String,
    pub value_b: String,
    pub assertion: String,
}

/// Evaluates sentinel rules against the current state of the project.
pub struct SentinelEngine {
    pub project_root: PathBuf,
}

impl SentinelEngine {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Dispatch a rule to the appropriate evaluator.
    pub fn evaluate(&self, rule_name: &str, rule: &SentinelRule) -> Vec<SentinelViolation> {
        match rule.rule.as_str() {
            "grep_match" => self.evaluate_grep_match(rule_name, rule),
            _ => Vec::new(),
        }
    }

    /// Extract a value from each pattern spec and compare per the assertion.
    pub fn evaluate_grep_match(
        &self,
        rule_name: &str,
        rule: &SentinelRule,
    ) -> Vec<SentinelViolation> {
        let pattern_a = match &rule.pattern_a {
            Some(p) => p,
            None => return Vec::new(),
        };
        let pattern_b = match &rule.pattern_b {
            Some(p) => p,
            None => return Vec::new(),
        };
        let assertion = match &rule.assert {
            Some(a) => a.as_str(),
            None => "a == b",
        };

        let val_a = match self.extract_value(pattern_a) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let val_b = match self.extract_value(pattern_b) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let passes = match assertion {
            "a == b" => val_a == val_b,
            "a != b" => val_a != val_b,
            _ => return Vec::new(),
        };

        if passes {
            Vec::new()
        } else {
            vec![SentinelViolation {
                rule_name: rule_name.to_string(),
                description: rule.description.clone(),
                value_a: val_a,
                value_b: val_b,
                assertion: assertion.to_string(),
            }]
        }
    }

    /// Read a file and extract the first capture group of the regex.
    pub fn extract_value(&self, spec: &PatternSpec) -> Option<String> {
        let path = self.project_root.join(&spec.file);
        let content = std::fs::read_to_string(&path).ok()?;
        let re = Regex::new(&spec.regex).ok()?;
        re.captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }
}
