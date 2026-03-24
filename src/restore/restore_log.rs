use anyhow::{Context, Result};
use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::event::{RestoreEvent, RestoreFileEntry, RestoreScope};

/// Append-only JSONL log for restore events.
pub struct RestoreLog {
    path: PathBuf,
    next_id: u64,
}

impl RestoreLog {
    /// Create a `RestoreLog` backed by the file at `path`.
    ///
    /// `next_id` is the first ID to assign; callers should initialise this
    /// by reading existing events if continuity is required.
    pub fn new(path: PathBuf) -> Self {
        Self { path, next_id: 0 }
    }

    /// Initialise `next_id` from the existing log so IDs stay monotonic across
    /// process restarts.  Call once after `new`.
    pub fn init_next_id(&mut self) -> Result<()> {
        let events = self.read_all()?;
        if let Some(last) = events.last() {
            self.next_id = last.id + 1;
        }
        Ok(())
    }

    /// Append a restore event and return it.
    pub fn append(
        &mut self,
        scope: RestoreScope,
        files: Vec<RestoreFileEntry>,
    ) -> Result<RestoreEvent> {
        let id = self.next_id;
        self.next_id += 1;

        let ts = chrono::Utc::now().timestamp_millis();

        let event = RestoreEvent {
            id,
            ts,
            scope,
            files_restored: files,
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open restore log {:?}", self.path))?;

        let line = serde_json::to_string(&event).context("serialize RestoreEvent")?;
        writeln!(file, "{}", line)
            .with_context(|| format!("write to restore log {:?}", self.path))?;

        Ok(event)
    }

    /// Read all restore events from the log.
    ///
    /// Malformed lines are skipped with a warning rather than aborting.
    pub fn read_all(&self) -> Result<Vec<RestoreEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = std::fs::File::open(&self.path)
            .with_context(|| format!("open restore log {:?}", self.path))?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<RestoreEvent>(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!("skipping malformed line {} in restore log: {}", i + 1, e);
                }
            }
        }

        Ok(events)
    }

    /// Get the last N restore events (useful as an undo stack).
    pub fn last_n(&self, n: usize) -> Result<Vec<RestoreEvent>> {
        let all = self.read_all()?;
        let start = all.len().saturating_sub(n);
        Ok(all[start..].to_vec())
    }
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::RestoreScope;
    use tempfile::tempdir;

    fn make_log(dir: &std::path::Path) -> RestoreLog {
        RestoreLog::new(dir.join("restores.jsonl"))
    }

    fn file_scope(id: u64) -> RestoreScope {
        RestoreScope::File { path: "src/lib.rs".to_string(), target_edit_id: id }
    }

    fn entry(path: &str, from: &str, to: &str) -> RestoreFileEntry {
        RestoreFileEntry {
            path: path.to_string(),
            from_hash: from.to_string(),
            to_hash: to.to_string(),
        }
    }

    #[test]
    fn test_append_and_read_all() {
        let dir = tempdir().unwrap();
        let mut log = make_log(dir.path());

        let e1 = log.append(file_scope(1), vec![entry("a.rs", "h1", "h2")]).unwrap();
        let e2 = log.append(file_scope(2), vec![entry("b.rs", "h3", "")]).unwrap();

        assert_eq!(e1.id, 0);
        assert_eq!(e2.id, 1);

        let all = log.read_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, 0);
        assert_eq!(all[1].id, 1);
        assert_eq!(all[0].files_restored[0].path, "a.rs");
    }

    #[test]
    fn test_read_all_nonexistent_returns_empty() {
        let dir = tempdir().unwrap();
        let log = make_log(dir.path());
        let all = log.read_all().unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn test_malformed_line_is_skipped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("restores.jsonl");

        let event = RestoreEvent {
            id: 0,
            ts: 0,
            scope: file_scope(10),
            files_restored: vec![],
        };
        let good = serde_json::to_string(&event).unwrap();
        std::fs::write(&path, format!("{}\n{{bad json\n", good)).unwrap();

        let log = RestoreLog::new(path);
        let all = log.read_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, 0);
    }

    #[test]
    fn test_last_n() {
        let dir = tempdir().unwrap();
        let mut log = make_log(dir.path());

        for i in 0..5 {
            log.append(file_scope(i), vec![]).unwrap();
        }

        let last3 = log.last_n(3).unwrap();
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].id, 2);
        assert_eq!(last3[2].id, 4);
    }

    #[test]
    fn test_last_n_more_than_total() {
        let dir = tempdir().unwrap();
        let mut log = make_log(dir.path());

        log.append(file_scope(0), vec![]).unwrap();
        log.append(file_scope(1), vec![]).unwrap();

        let result = log.last_n(10).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_init_next_id_resumes_from_existing() {
        let dir = tempdir().unwrap();

        // Write two events in a first log instance
        {
            let mut log = make_log(dir.path());
            log.append(file_scope(0), vec![]).unwrap();
            log.append(file_scope(1), vec![]).unwrap();
        }

        // Re-open and initialise next_id
        let mut log2 = make_log(dir.path());
        log2.init_next_id().unwrap();
        let e = log2.append(file_scope(2), vec![]).unwrap();
        // Should continue from id=2
        assert_eq!(e.id, 2);
    }

    #[test]
    fn test_scope_variants_roundtrip() {
        let dir = tempdir().unwrap();
        let mut log = make_log(dir.path());

        let op_scope = RestoreScope::Operation { operation_id: "op-1".to_string() };
        let range_scope = RestoreScope::AgentRange {
            agent_id: "a1".to_string(),
            from_ts: 1000,
            to_ts: 2000,
        };

        log.append(op_scope, vec![]).unwrap();
        log.append(range_scope, vec![]).unwrap();

        let all = log.read_all().unwrap();
        assert_eq!(all.len(), 2);
        assert!(matches!(&all[0].scope, RestoreScope::Operation { .. }));
        assert!(matches!(&all[1].scope, RestoreScope::AgentRange { .. }));
    }
}
