use std::collections::HashMap;

use crate::event::AgentInfo;

/// Tracks agents (e.g. Claude Code sessions) that have sent hook messages.
///
/// Each agent gets an auto-assigned human-readable label ("{tool_type}-1",
/// "{tool_type}-2", ...) in the order they first appear. The registry persists
/// `AgentInfo` entries that are serialized into `meta.json`.
pub struct AgentRegistry {
    agents: HashMap<String, AgentInfo>,
    next_label: u32,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            next_label: 1,
        }
    }

    /// Register a new agent or update an existing one.
    ///
    /// If the agent is new, a label is auto-assigned ("{tool_type}-N").
    /// If the agent already exists, `last_seen` is updated.
    ///
    /// Returns a reference to the `AgentInfo` for this agent.
    pub fn register_or_update(&mut self, agent_id: &str, tool_type: &str, ts: i64) -> &AgentInfo {
        let next_label = &mut self.next_label;
        self.agents
            .entry(agent_id.to_string())
            .and_modify(|info| {
                info.last_seen = ts;
            })
            .or_insert_with(|| {
                let label = format!("{}-{}", tool_type, *next_label);
                *next_label += 1;
                AgentInfo {
                    agent_id: agent_id.to_string(),
                    agent_label: label,
                    tool_type: tool_type.to_string(),
                    first_seen: ts,
                    last_seen: ts,
                    edit_count: 0,
                }
            })
    }

    /// Increment the edit count for an agent and update `last_seen`.
    pub fn increment_edit_count(&mut self, agent_id: &str, ts: i64) {
        if let Some(info) = self.agents.get_mut(agent_id) {
            info.edit_count += 1;
            info.last_seen = ts;
        }
    }

    /// Look up an agent by ID.
    pub fn get(&self, agent_id: &str) -> Option<&AgentInfo> {
        self.agents.get(agent_id)
    }

    /// Return all agents as a `Vec<AgentInfo>`, sorted by label.
    pub fn to_vec(&self) -> Vec<AgentInfo> {
        let mut agents: Vec<AgentInfo> = self.agents.values().cloned().collect();
        agents.sort_by(|a, b| a.agent_label.cmp(&b.agent_label));
        agents
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---- unit tests ---------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_agent_registration() {
        let mut registry = AgentRegistry::new();
        let info = registry.register_or_update("pid-100", "claude-code", 1000);

        assert_eq!(info.agent_id, "pid-100");
        assert_eq!(info.agent_label, "claude-code-1");
        assert_eq!(info.tool_type, "claude-code");
        assert_eq!(info.first_seen, 1000);
        assert_eq!(info.last_seen, 1000);
        assert_eq!(info.edit_count, 0);
    }

    #[test]
    fn label_numbering_increments() {
        let mut registry = AgentRegistry::new();

        registry.register_or_update("agent-a", "claude-code", 1000);
        registry.register_or_update("agent-b", "claude-code", 2000);
        registry.register_or_update("agent-c", "claude-code", 3000);

        assert_eq!(registry.get("agent-a").unwrap().agent_label, "claude-code-1");
        assert_eq!(registry.get("agent-b").unwrap().agent_label, "claude-code-2");
        assert_eq!(registry.get("agent-c").unwrap().agent_label, "claude-code-3");
    }

    #[test]
    fn update_existing_agent() {
        let mut registry = AgentRegistry::new();

        registry.register_or_update("agent-a", "claude-code", 1000);
        registry.register_or_update("agent-a", "claude-code", 5000);

        let info = registry.get("agent-a").unwrap();
        assert_eq!(info.agent_label, "claude-code-1"); // Label unchanged.
        assert_eq!(info.first_seen, 1000); // First seen unchanged.
        assert_eq!(info.last_seen, 5000); // Last seen updated.
    }

    #[test]
    fn update_does_not_consume_label() {
        let mut registry = AgentRegistry::new();

        registry.register_or_update("agent-a", "claude-code", 1000);
        // Re-registering the same agent should NOT increment the label counter.
        registry.register_or_update("agent-a", "claude-code", 2000);
        registry.register_or_update("agent-b", "claude-code", 3000);

        // agent-b should be claude-code-2, not claude-code-3.
        assert_eq!(registry.get("agent-b").unwrap().agent_label, "claude-code-2");
    }

    #[test]
    fn increment_edit_count() {
        let mut registry = AgentRegistry::new();
        registry.register_or_update("agent-a", "claude-code", 1000);

        registry.increment_edit_count("agent-a", 2000);
        registry.increment_edit_count("agent-a", 3000);

        let info = registry.get("agent-a").unwrap();
        assert_eq!(info.edit_count, 2);
        assert_eq!(info.last_seen, 3000);
    }

    #[test]
    fn increment_noop_for_unknown_agent() {
        let mut registry = AgentRegistry::new();
        // Should not panic.
        registry.increment_edit_count("nonexistent", 1000);
    }

    #[test]
    fn to_vec_sorted_by_label() {
        let mut registry = AgentRegistry::new();
        registry.register_or_update("z", "claude-code", 1000);
        registry.register_or_update("a", "claude-code", 2000);

        let agents = registry.to_vec();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].agent_label, "claude-code-1"); // z was first registered
        assert_eq!(agents[1].agent_label, "claude-code-2");
    }

    #[test]
    fn get_unknown_returns_none() {
        let registry = AgentRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn label_uses_tool_type_prefix() {
        let mut registry = AgentRegistry::new();

        let info = registry.register_or_update("cursor-abc", "cursor", 1000);
        assert_eq!(info.agent_label, "cursor-1");

        let info = registry.register_or_update("codex-def", "codex", 2000);
        assert_eq!(info.agent_label, "codex-2");

        // Claude agents still work
        let info = registry.register_or_update("claude-ghi", "claude-code", 3000);
        assert_eq!(info.agent_label, "claude-code-3");
    }
}
