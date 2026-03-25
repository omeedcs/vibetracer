use std::collections::{HashMap, VecDeque};
use std::time::Instant;

/// Payload extracted from a hook message, describing an agent's intent for a
/// file edit.
#[derive(Debug, Clone)]
pub struct HookPayload {
    pub agent_id: String,
    pub operation_id: String,
    pub tool_name: String,
    pub intent: Option<String>,
}

/// A payload with an arrival timestamp so we can expire stale entries.
#[derive(Debug, Clone)]
struct TimedPayload {
    payload: HookPayload,
    received_at: Instant,
}

/// Maps hook messages to file-change events using a FIFO queue per filename.
///
/// When a hook message arrives for file X, the payload is pushed into the queue
/// for X. When a `notify` event arrives for file X, the daemon pops the oldest
/// enrichment (FIFO) and attaches it to the `EditEvent`. If no enrichment is
/// pending, the event gets `None` for all agent/operation fields.
///
/// The correlator also tracks pending restores. When a restore is registered,
/// the first file-change event for each listed file is tagged with the
/// `restore_id`. Restore tagging takes precedence over hook enrichment.
pub struct Correlator {
    /// Pending hook enrichments keyed by relative filename.
    pending: HashMap<String, VecDeque<TimedPayload>>,
    /// Files with a pending restore: filename -> restore_id.
    pending_restores: HashMap<String, u64>,
}

impl Correlator {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            pending_restores: HashMap::new(),
        }
    }

    /// Push a hook enrichment payload for a given file.
    pub fn push_enrichment(&mut self, file: &str, payload: HookPayload) {
        self.pending
            .entry(file.to_string())
            .or_default()
            .push_back(TimedPayload {
                payload,
                received_at: Instant::now(),
            });
    }

    /// Pop the oldest hook enrichment for a given file (FIFO).
    pub fn pop_enrichment(&mut self, file: &str) -> Option<HookPayload> {
        if let Some(queue) = self.pending.get_mut(file) {
            let item = queue.pop_front().map(|tp| tp.payload);
            if queue.is_empty() {
                self.pending.remove(file);
            }
            item
        } else {
            None
        }
    }

    /// Remove enrichment entries older than `max_age_ms` milliseconds.
    pub fn cleanup_stale(&mut self, max_age_ms: u64) {
        let cutoff = std::time::Duration::from_millis(max_age_ms);
        let now = Instant::now();

        self.pending.retain(|_file, queue| {
            queue.retain(|tp| now.duration_since(tp.received_at) < cutoff);
            !queue.is_empty()
        });
    }

    /// Register files that are about to be restored, associating each with a
    /// restore ID.
    pub fn register_restore(&mut self, restore_id: u64, files: &[String]) {
        for file in files {
            self.pending_restores.insert(file.clone(), restore_id);
        }
    }

    /// Check whether a file has a pending restore. If so, consume it (the first
    /// file-change event per file per restore gets tagged) and return the
    /// restore ID.
    pub fn pop_restore(&mut self, file: &str) -> Option<u64> {
        self.pending_restores.remove(file)
    }

    /// Clear all pending restore entries for a given restore ID.
    pub fn clear_restore(&mut self, restore_id: u64) {
        self.pending_restores
            .retain(|_file, rid| *rid != restore_id);
    }
}

impl Default for Correlator {
    fn default() -> Self {
        Self::new()
    }
}

// ---- unit tests ---------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(agent: &str, op: &str, tool: &str) -> HookPayload {
        HookPayload {
            agent_id: agent.to_string(),
            operation_id: op.to_string(),
            tool_name: tool.to_string(),
            intent: None,
        }
    }

    #[test]
    fn fifo_ordering() {
        let mut c = Correlator::new();
        c.push_enrichment("src/main.rs", payload("a1", "op-1", "Edit"));
        c.push_enrichment("src/main.rs", payload("a2", "op-2", "Write"));

        let first = c.pop_enrichment("src/main.rs").unwrap();
        assert_eq!(first.agent_id, "a1");
        assert_eq!(first.operation_id, "op-1");

        let second = c.pop_enrichment("src/main.rs").unwrap();
        assert_eq!(second.agent_id, "a2");
        assert_eq!(second.operation_id, "op-2");

        // Queue should now be empty.
        assert!(c.pop_enrichment("src/main.rs").is_none());
    }

    #[test]
    fn pop_from_empty_returns_none() {
        let mut c = Correlator::new();
        assert!(c.pop_enrichment("nonexistent.rs").is_none());
    }

    #[test]
    fn stale_cleanup() {
        let mut c = Correlator::new();

        // Push an enrichment that will become stale immediately.
        c.pending
            .entry("old.rs".to_string())
            .or_default()
            .push_back(TimedPayload {
                payload: payload("a1", "op-1", "Edit"),
                received_at: Instant::now() - std::time::Duration::from_secs(10),
            });

        // Push a fresh enrichment.
        c.push_enrichment("fresh.rs", payload("a2", "op-2", "Write"));

        // Cleanup with a 5-second threshold.
        c.cleanup_stale(5_000);

        assert!(c.pop_enrichment("old.rs").is_none());
        assert!(c.pop_enrichment("fresh.rs").is_some());
    }

    #[test]
    fn restore_registration_and_pop() {
        let mut c = Correlator::new();
        c.register_restore(42, &["src/a.rs".to_string(), "src/b.rs".to_string()]);

        // First pop for a.rs should return the restore_id.
        assert_eq!(c.pop_restore("src/a.rs"), Some(42));
        // Second pop should return None (consumed).
        assert_eq!(c.pop_restore("src/a.rs"), None);

        // b.rs still pending.
        assert_eq!(c.pop_restore("src/b.rs"), Some(42));
    }

    #[test]
    fn clear_restore_removes_all_entries() {
        let mut c = Correlator::new();
        c.register_restore(99, &["a.rs".to_string(), "b.rs".to_string()]);
        c.clear_restore(99);

        assert!(c.pop_restore("a.rs").is_none());
        assert!(c.pop_restore("b.rs").is_none());
    }

    #[test]
    fn restore_precedence_over_hook() {
        let mut c = Correlator::new();

        // Both a hook enrichment and a restore registered for the same file.
        c.push_enrichment("src/main.rs", payload("a1", "op-1", "Edit"));
        c.register_restore(77, &["src/main.rs".to_string()]);

        // Restore takes precedence -- check restore first.
        let restore_id = c.pop_restore("src/main.rs");
        assert_eq!(restore_id, Some(77));

        // The hook enrichment is still in the queue (but would be discarded by
        // the daemon when restore wins).
        let hook = c.pop_enrichment("src/main.rs");
        assert!(hook.is_some());
    }

    #[test]
    fn separate_files_are_independent() {
        let mut c = Correlator::new();
        c.push_enrichment("a.rs", payload("a1", "op-1", "Edit"));
        c.push_enrichment("b.rs", payload("a2", "op-2", "Write"));

        let a = c.pop_enrichment("a.rs").unwrap();
        assert_eq!(a.agent_id, "a1");

        let b = c.pop_enrichment("b.rs").unwrap();
        assert_eq!(b.agent_id, "a2");
    }
}
