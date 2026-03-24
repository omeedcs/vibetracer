use anyhow::{Context, Result};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::event::EditEvent;

/// Append-only JSONL file that records edit events.
pub struct EditLog {
    path: PathBuf,
}

impl EditLog {
    /// Create an `EditLog` backed by the file at `path`.
    ///
    /// The file is created lazily on the first `append` call.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append `event` as a single JSON line to the log file.
    pub fn append(&self, event: &EditEvent) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open edit log {:?}", self.path))?;

        let line = serde_json::to_string(event).context("serialize EditEvent")?;
        writeln!(file, "{}", line).with_context(|| format!("write to edit log {:?}", self.path))?;
        Ok(())
    }

    /// Read all events from the log file at `path`.
    ///
    /// Malformed lines are skipped with a warning rather than aborting, which
    /// allows graceful recovery from truncated writes at the end of a log file.
    pub fn read_all(path: &Path) -> anyhow::Result<Vec<EditEvent>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<EditEvent>(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!("skipping malformed line {} in edit log: {}", i + 1, e);
                }
            }
        }
        Ok(events)
    }

    /// Return the number of events recorded in the log.
    pub fn count(&self) -> Result<u64> {
        if !self.path.exists() {
            return Ok(0);
        }
        let file = std::fs::File::open(&self.path)
            .with_context(|| format!("open edit log {:?}", self.path))?;
        let reader = std::io::BufReader::new(file);
        let count = reader
            .lines()
            .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false))
            .count() as u64;
        Ok(count)
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EditKind;
    use tempfile::tempdir;

    fn sample_event(id: u64) -> EditEvent {
        EditEvent {
            id,
            ts: 1_700_000_000_000,
            file: "src/main.rs".to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: None,
            after_hash: "abc".to_string(),
            intent: None,
            tool: None,
            lines_added: 0,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        }
    }

    #[test]
    fn test_append_and_count() {
        let dir = tempdir().unwrap();
        let log = EditLog::new(dir.path().join("edits.jsonl"));
        log.append(&sample_event(1)).unwrap();
        log.append(&sample_event(2)).unwrap();
        assert_eq!(log.count().unwrap(), 2);
    }

    #[test]
    fn test_count_nonexistent_file_returns_zero() {
        let dir = tempdir().unwrap();
        let log = EditLog::new(dir.path().join("nonexistent.jsonl"));
        assert_eq!(log.count().unwrap(), 0);
    }

    #[test]
    fn test_read_all_skips_malformed_trailing_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("edits.jsonl");
        // Write a valid event followed by a truncated line
        let valid_event = EditEvent {
            id: 1,
            ts: 0,
            file: "a.rs".to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: None,
            after_hash: "x".to_string(),
            intent: None,
            tool: None,
            lines_added: 0,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        };
        let json = serde_json::to_string(&valid_event).unwrap();
        std::fs::write(&path, format!("{}\n{{truncated", json)).unwrap();
        let events = EditLog::read_all(&path).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, 1);
    }
}
