use std::path::PathBuf;

use anyhow::Result;

use super::transport::StdioWriter;

pub fn handle_subscribe(
    _arguments: &serde_json::Value,
    _project_path: &PathBuf,
    _sessions_dir: &PathBuf,
    _writer: &mut StdioWriter,
) -> Result<serde_json::Value> {
    anyhow::bail!(
        "subscribe_edits requires a running daemon — start one with `vibetracer daemon start`"
    )
}
