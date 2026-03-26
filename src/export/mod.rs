pub mod agent_trace;
pub mod git_notes;

use clap::ValueEnum;

/// Supported export formats.
#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat {
    /// Agent Trace JSON (Cursor/git-ai ecosystem)
    AgentTrace,
    /// git-ai compatible git notes
    GitNotes,
}
