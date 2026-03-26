use anyhow::Result;
use std::path::Path;
use std::sync::mpsc;

use crate::event::EditEvent;

/// Base trait for all agent importers. Supports batch import of edit events
/// from an agent's log files.
pub trait AgentImporter: Send + Sync {
    /// Human-readable name of the agent (e.g. "claude-code", "cursor").
    fn agent_name(&self) -> &str;

    /// Format version this importer understands, if known.
    fn format_version(&self) -> Option<&str>;

    /// Check whether this importer can handle the file/directory at `path`.
    fn can_import(&self, path: &Path) -> bool;

    /// Batch-import all edit events from the log at `path`.
    fn import_edits(&self, path: &Path, project_root: &Path) -> Result<Vec<EditEvent>>;
}

/// Extended trait for importers that support live tailing (e.g. Claude Code hooks).
/// Batch-only importers (e.g. Cursor Agent Trace) do NOT implement this.
pub trait LiveAgentImporter: AgentImporter {
    /// Spawn a background thread that tails the log and sends events over the channel.
    /// Returns the receiver end. The thread should stop when the sender is dropped.
    fn watch_live(&self, path: &Path, project_root: &Path) -> Result<mpsc::Receiver<EditEvent>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockImporter;

    impl AgentImporter for MockImporter {
        fn agent_name(&self) -> &str {
            "mock-agent"
        }
        fn format_version(&self) -> Option<&str> {
            Some("1.0")
        }
        fn can_import(&self, _path: &Path) -> bool {
            true
        }
        fn import_edits(&self, _path: &Path, _project_root: &Path) -> Result<Vec<EditEvent>> {
            Ok(vec![])
        }
    }

    #[test]
    fn trait_object_works() {
        let importer: Box<dyn AgentImporter> = Box::new(MockImporter);
        assert_eq!(importer.agent_name(), "mock-agent");
        assert_eq!(importer.format_version(), Some("1.0"));
        assert!(importer.can_import(Path::new("/tmp")));
        assert!(importer.import_edits(Path::new("/tmp"), Path::new("/tmp")).unwrap().is_empty());
    }
}
