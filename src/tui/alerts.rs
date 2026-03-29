use serde::{Deserialize, Serialize};

/// Action to take when an alert fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlertAction {
    /// Status bar notification.
    Toast,
    /// Brief screen flash.
    Flash,
    /// Terminal bell.
    Bell,
}

/// A configured alert rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub name: String,
    pub when: String,
    pub action: AlertAction,
    #[serde(default)]
    pub message: Option<String>,
}

/// Condition that can be evaluated against app state.
#[derive(Debug, Clone, PartialEq)]
pub enum AlertCondition {
    SessionCostGreater(f64),
    SentinelFailuresGreater(u32),
    StaleCountGreater(u32),
    EditVelocityGreater(f64),
    EditCountGreater(u64),
    AgentIdleGreater(u64),
}

/// Snapshot of app state needed for alert evaluation.
pub struct AlertState {
    pub session_cost: f64,
    pub sentinel_failures: u32,
    pub stale_count: u32,
    pub edit_velocity: f64,
    pub edit_count: u64,
}

/// An alert that has just fired.
pub struct FiredAlert {
    pub name: String,
    pub action: AlertAction,
    pub message: String,
}

/// Alert evaluation state. Tracks which alerts have already fired to avoid
/// spamming the same notification repeatedly. An alert re-arms when its
/// condition becomes false.
pub struct AlertEvaluator {
    pub configs: Vec<AlertConfig>,
    pub conditions: Vec<Option<AlertCondition>>,
    pub fired: Vec<bool>,
}

impl AlertEvaluator {
    pub fn new(configs: Vec<AlertConfig>) -> Self {
        let conditions: Vec<Option<AlertCondition>> =
            configs.iter().map(|c| Self::parse_condition(&c.when)).collect();
        let fired = vec![false; configs.len()];
        AlertEvaluator {
            configs,
            conditions,
            fired,
        }
    }

    /// Create an empty evaluator with no configured alerts.
    pub fn empty() -> Self {
        AlertEvaluator {
            configs: Vec::new(),
            conditions: Vec::new(),
            fired: Vec::new(),
        }
    }

    /// Parse the "when" string into an AlertCondition.
    ///
    /// Supported expressions:
    ///   "session_cost > 1.00"
    ///   "sentinel_failures > 0"
    ///   "stale_count > 3"
    ///   "edit_velocity > 10"
    ///   "edit_count > 100"
    ///   "agent_idle > 60"
    fn parse_condition(when: &str) -> Option<AlertCondition> {
        let parts: Vec<&str> = when.split_whitespace().collect();
        if parts.len() != 3 {
            return None;
        }

        let metric = parts[0];
        let operator = parts[1];

        // Only ">" is supported for now.
        if operator != ">" {
            return None;
        }

        match metric {
            "session_cost" => {
                let val: f64 = parts[2].parse().ok()?;
                Some(AlertCondition::SessionCostGreater(val))
            }
            "sentinel_failures" => {
                let val: u32 = parts[2].parse().ok()?;
                Some(AlertCondition::SentinelFailuresGreater(val))
            }
            "stale_count" => {
                let val: u32 = parts[2].parse().ok()?;
                Some(AlertCondition::StaleCountGreater(val))
            }
            "edit_velocity" => {
                let val: f64 = parts[2].parse().ok()?;
                Some(AlertCondition::EditVelocityGreater(val))
            }
            "edit_count" => {
                let val: u64 = parts[2].parse().ok()?;
                Some(AlertCondition::EditCountGreater(val))
            }
            "agent_idle" => {
                let val: u64 = parts[2].parse().ok()?;
                Some(AlertCondition::AgentIdleGreater(val))
            }
            _ => None,
        }
    }

    /// Evaluate a single condition against the current state.
    fn condition_met(condition: &AlertCondition, state: &AlertState) -> bool {
        match condition {
            AlertCondition::SessionCostGreater(threshold) => state.session_cost > *threshold,
            AlertCondition::SentinelFailuresGreater(threshold) => {
                state.sentinel_failures > *threshold
            }
            AlertCondition::StaleCountGreater(threshold) => state.stale_count > *threshold,
            AlertCondition::EditVelocityGreater(threshold) => state.edit_velocity > *threshold,
            AlertCondition::EditCountGreater(threshold) => state.edit_count > *threshold,
            AlertCondition::AgentIdleGreater(_threshold) => {
                // Agent idle requires external idle-time tracking; not evaluated here.
                // A future extension could pass idle seconds into AlertState.
                false
            }
        }
    }

    /// Evaluate all conditions against the current state.
    /// Returns a list of alerts that just transitioned from not-fired to fired.
    pub fn evaluate(&mut self, state: &AlertState) -> Vec<FiredAlert> {
        let mut result = Vec::new();

        for i in 0..self.configs.len() {
            let condition = match &self.conditions[i] {
                Some(c) => c,
                None => continue, // unparseable condition -- skip
            };

            let met = Self::condition_met(condition, state);

            if met && !self.fired[i] {
                // Transition: not-fired -> fired. Emit alert.
                self.fired[i] = true;
                let config = &self.configs[i];
                result.push(FiredAlert {
                    name: config.name.clone(),
                    action: config.action.clone(),
                    message: config
                        .message
                        .clone()
                        .unwrap_or_else(|| config.name.clone()),
                });
            } else if !met && self.fired[i] {
                // Condition no longer met -- re-arm so it can fire again.
                self.fired[i] = false;
            }
        }

        result
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_condition ────────────────────────────────────────────────────

    #[test]
    fn parse_session_cost() {
        let cond = AlertEvaluator::parse_condition("session_cost > 1.50");
        assert_eq!(cond, Some(AlertCondition::SessionCostGreater(1.50)));
    }

    #[test]
    fn parse_sentinel_failures() {
        let cond = AlertEvaluator::parse_condition("sentinel_failures > 0");
        assert_eq!(cond, Some(AlertCondition::SentinelFailuresGreater(0)));
    }

    #[test]
    fn parse_stale_count() {
        let cond = AlertEvaluator::parse_condition("stale_count > 3");
        assert_eq!(cond, Some(AlertCondition::StaleCountGreater(3)));
    }

    #[test]
    fn parse_edit_velocity() {
        let cond = AlertEvaluator::parse_condition("edit_velocity > 10.5");
        assert_eq!(cond, Some(AlertCondition::EditVelocityGreater(10.5)));
    }

    #[test]
    fn parse_edit_count() {
        let cond = AlertEvaluator::parse_condition("edit_count > 100");
        assert_eq!(cond, Some(AlertCondition::EditCountGreater(100)));
    }

    #[test]
    fn parse_agent_idle() {
        let cond = AlertEvaluator::parse_condition("agent_idle > 60");
        assert_eq!(cond, Some(AlertCondition::AgentIdleGreater(60)));
    }

    #[test]
    fn parse_unknown_metric() {
        let cond = AlertEvaluator::parse_condition("unknown_metric > 5");
        assert_eq!(cond, None);
    }

    #[test]
    fn parse_bad_operator() {
        let cond = AlertEvaluator::parse_condition("session_cost < 1.00");
        assert_eq!(cond, None);
    }

    #[test]
    fn parse_bad_value() {
        let cond = AlertEvaluator::parse_condition("session_cost > abc");
        assert_eq!(cond, None);
    }

    #[test]
    fn parse_too_few_parts() {
        let cond = AlertEvaluator::parse_condition("session_cost");
        assert_eq!(cond, None);
    }

    #[test]
    fn parse_empty_string() {
        let cond = AlertEvaluator::parse_condition("");
        assert_eq!(cond, None);
    }

    // ── evaluate ───────────────────────────────────────────────────────────

    fn make_state(cost: f64, sentinel_fail: u32, stale: u32, velocity: f64, edits: u64) -> AlertState {
        AlertState {
            session_cost: cost,
            sentinel_failures: sentinel_fail,
            stale_count: stale,
            edit_velocity: velocity,
            edit_count: edits,
        }
    }

    #[test]
    fn evaluate_fires_when_threshold_exceeded() {
        let configs = vec![AlertConfig {
            name: "cost alert".to_string(),
            when: "session_cost > 1.00".to_string(),
            action: AlertAction::Toast,
            message: Some("session cost exceeded $1".to_string()),
        }];
        let mut evaluator = AlertEvaluator::new(configs);

        // Below threshold -- no alerts.
        let fired = evaluator.evaluate(&make_state(0.50, 0, 0, 0.0, 0));
        assert!(fired.is_empty());
        assert!(!evaluator.fired[0]);

        // Above threshold -- fires.
        let fired = evaluator.evaluate(&make_state(1.50, 0, 0, 0.0, 0));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].name, "cost alert");
        assert_eq!(fired[0].message, "session cost exceeded $1");
        assert_eq!(fired[0].action, AlertAction::Toast);
        assert!(evaluator.fired[0]);
    }

    #[test]
    fn evaluate_does_not_fire_twice() {
        let configs = vec![AlertConfig {
            name: "cost alert".to_string(),
            when: "session_cost > 1.00".to_string(),
            action: AlertAction::Flash,
            message: None,
        }];
        let mut evaluator = AlertEvaluator::new(configs);

        // Fire once.
        let fired = evaluator.evaluate(&make_state(2.00, 0, 0, 0.0, 0));
        assert_eq!(fired.len(), 1);

        // Same state -- should not fire again.
        let fired = evaluator.evaluate(&make_state(2.50, 0, 0, 0.0, 0));
        assert!(fired.is_empty());
    }

    #[test]
    fn evaluate_rearms_when_condition_clears() {
        let configs = vec![AlertConfig {
            name: "sentinel alert".to_string(),
            when: "sentinel_failures > 0".to_string(),
            action: AlertAction::Bell,
            message: None,
        }];
        let mut evaluator = AlertEvaluator::new(configs);

        // Fire.
        let fired = evaluator.evaluate(&make_state(0.0, 1, 0, 0.0, 0));
        assert_eq!(fired.len(), 1);

        // Condition clears (re-arm).
        let fired = evaluator.evaluate(&make_state(0.0, 0, 0, 0.0, 0));
        assert!(fired.is_empty());
        assert!(!evaluator.fired[0]);

        // Condition met again -- fires again.
        let fired = evaluator.evaluate(&make_state(0.0, 2, 0, 0.0, 0));
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn evaluate_uses_name_when_no_message() {
        let configs = vec![AlertConfig {
            name: "stale files".to_string(),
            when: "stale_count > 5".to_string(),
            action: AlertAction::Toast,
            message: None,
        }];
        let mut evaluator = AlertEvaluator::new(configs);

        let fired = evaluator.evaluate(&make_state(0.0, 0, 10, 0.0, 0));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].message, "stale files");
    }

    #[test]
    fn evaluate_skips_unparseable_conditions() {
        let configs = vec![AlertConfig {
            name: "bad rule".to_string(),
            when: "garbage".to_string(),
            action: AlertAction::Toast,
            message: None,
        }];
        let mut evaluator = AlertEvaluator::new(configs);
        assert!(evaluator.conditions[0].is_none());

        let fired = evaluator.evaluate(&make_state(100.0, 100, 100, 100.0, 100));
        assert!(fired.is_empty());
    }

    #[test]
    fn evaluate_multiple_alerts() {
        let configs = vec![
            AlertConfig {
                name: "cost".to_string(),
                when: "session_cost > 1.00".to_string(),
                action: AlertAction::Toast,
                message: None,
            },
            AlertConfig {
                name: "velocity".to_string(),
                when: "edit_velocity > 5.0".to_string(),
                action: AlertAction::Flash,
                message: Some("edits are fast".to_string()),
            },
            AlertConfig {
                name: "edits".to_string(),
                when: "edit_count > 50".to_string(),
                action: AlertAction::Bell,
                message: None,
            },
        ];
        let mut evaluator = AlertEvaluator::new(configs);

        // Only cost and velocity should fire.
        let fired = evaluator.evaluate(&make_state(2.00, 0, 0, 10.0, 30));
        assert_eq!(fired.len(), 2);
        assert_eq!(fired[0].name, "cost");
        assert_eq!(fired[1].name, "velocity");
        assert_eq!(fired[1].message, "edits are fast");

        // Now edit_count crosses threshold too.
        let fired = evaluator.evaluate(&make_state(2.00, 0, 0, 10.0, 60));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].name, "edits");
    }

    #[test]
    fn evaluator_empty() {
        let evaluator = AlertEvaluator::empty();
        assert!(evaluator.configs.is_empty());
        assert!(evaluator.conditions.is_empty());
        assert!(evaluator.fired.is_empty());
    }

    #[test]
    fn evaluate_exact_threshold_does_not_fire() {
        // ">" is strict, so exactly equal should NOT fire.
        let configs = vec![AlertConfig {
            name: "cost".to_string(),
            when: "session_cost > 1.00".to_string(),
            action: AlertAction::Toast,
            message: None,
        }];
        let mut evaluator = AlertEvaluator::new(configs);

        let fired = evaluator.evaluate(&make_state(1.00, 0, 0, 0.0, 0));
        assert!(fired.is_empty());
    }
}
