use anyhow::Result;
use std::io::BufRead;
use std::path::Path;

use crate::event::EditEvent;

pub const MAX_LIMIT: u32 = 1000;
pub const DEFAULT_LIMIT: u32 = 100;

/// Parameters for paginated reads from an edit log.
pub struct PageParams {
    pub offset: u32,
    pub limit: u32,
}

impl Default for PageParams {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

impl PageParams {
    /// Returns the effective limit, clamped to `MAX_LIMIT`.
    pub fn effective_limit(&self) -> u32 {
        self.limit.min(MAX_LIMIT)
    }
}

/// Result of a paginated read, containing a page of events and the total matching count.
pub struct PageResult {
    pub events: Vec<EditEvent>,
    pub total_count: u32,
}

/// Streaming paginated reader for JSONL edit logs.
///
/// Reads the file line by line, applying an optional filter. Skips the first
/// `offset` matching entries, collects up to `effective_limit` matching entries,
/// and counts all matching entries. Malformed lines are silently skipped.
/// Never loads the full file into memory.
pub fn read_edits_paged(
    jsonl_path: &Path,
    params: &PageParams,
    filter: Option<&dyn Fn(&EditEvent) -> bool>,
) -> Result<PageResult> {
    let file = std::fs::File::open(jsonl_path)?;
    let reader = std::io::BufReader::new(file);

    let effective_limit = params.effective_limit();
    let mut total_count: u32 = 0;
    let mut collected: Vec<EditEvent> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.trim().is_empty() {
            continue;
        }

        let event: EditEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // silently skip malformed lines
        };

        // Apply filter if provided
        if let Some(f) = &filter {
            if !f(&event) {
                continue;
            }
        }

        total_count += 1;

        // Skip entries before offset
        if total_count <= params.offset {
            continue;
        }

        // Collect up to effective_limit entries
        if (collected.len() as u32) < effective_limit {
            collected.push(event);
        }
        // Keep counting total even after we have enough collected entries
    }

    Ok(PageResult {
        events: collected,
        total_count,
    })
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EditKind;
    use tempfile::tempdir;

    /// Build a sample EditEvent with the given id and file name.
    fn sample_event(id: u64, file: &str) -> EditEvent {
        EditEvent {
            id,
            ts: 1_700_000_000_000 + id as i64,
            file: file.to_string(),
            kind: EditKind::Modify,
            patch: String::new(),
            before_hash: None,
            after_hash: "abc".to_string(),
            intent: None,
            tool: None,
            lines_added: 1,
            lines_removed: 0,
            agent_id: None,
            agent_label: None,
            operation_id: None,
            operation_intent: None,
            tool_name: None,
            restore_id: None,
        }
    }

    /// Write a slice of EditEvents to a temp JSONL file and return its path.
    fn write_jsonl(events: &[EditEvent]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        let mut content = String::new();
        for e in events {
            content.push_str(&serde_json::to_string(e).unwrap());
            content.push('\n');
        }
        std::fs::write(&path, &content).unwrap();
        (dir, path)
    }

    #[test]
    fn test_read_all_edits_default_pagination() {
        let events: Vec<EditEvent> = (1..=5).map(|i| sample_event(i, "src/main.rs")).collect();
        let (_dir, path) = write_jsonl(&events);

        let result = read_edits_paged(&path, &PageParams::default(), None).unwrap();
        assert_eq!(result.events.len(), 5);
        assert_eq!(result.total_count, 5);
        for (i, e) in result.events.iter().enumerate() {
            assert_eq!(e.id, (i + 1) as u64);
        }
    }

    #[test]
    fn test_pagination_with_offset_and_limit() {
        let events: Vec<EditEvent> = (1..=10).map(|i| sample_event(i, "src/main.rs")).collect();
        let (_dir, path) = write_jsonl(&events);

        let params = PageParams {
            offset: 3,
            limit: 2,
        };
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].id, 4);
        assert_eq!(result.events[1].id, 5);
        assert_eq!(result.total_count, 10);
    }

    #[test]
    fn test_pagination_offset_beyond_total() {
        let events: Vec<EditEvent> = (1..=3).map(|i| sample_event(i, "src/main.rs")).collect();
        let (_dir, path) = write_jsonl(&events);

        let params = PageParams {
            offset: 10,
            limit: DEFAULT_LIMIT,
        };
        let result = read_edits_paged(&path, &params, None).unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.total_count, 3);
    }

    #[test]
    fn test_pagination_with_filter() {
        // 9 edits with file pattern file_{i%3}.rs
        let events: Vec<EditEvent> = (0..9)
            .map(|i| sample_event(i as u64, &format!("file_{}.rs", i % 3)))
            .collect();
        let (_dir, path) = write_jsonl(&events);

        let filter = |e: &EditEvent| e.file == "file_1.rs";
        let result = read_edits_paged(&path, &PageParams::default(), Some(&filter)).unwrap();
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.total_count, 3);
        for e in &result.events {
            assert_eq!(e.file, "file_1.rs");
        }
    }

    #[test]
    fn test_limit_clamped_to_max() {
        let params = PageParams {
            offset: 0,
            limit: 5000,
        };
        assert_eq!(params.effective_limit(), MAX_LIMIT);
    }

    #[test]
    fn test_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");
        std::fs::write(&path, "").unwrap();

        let result = read_edits_paged(&path, &PageParams::default(), None).unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_skips_malformed_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edits.jsonl");

        let valid1 = serde_json::to_string(&sample_event(1, "a.rs")).unwrap();
        let valid2 = serde_json::to_string(&sample_event(2, "b.rs")).unwrap();
        let content = format!("{}\n{{not valid json\n{}\n", valid1, valid2);
        std::fs::write(&path, content).unwrap();

        let result = read_edits_paged(&path, &PageParams::default(), None).unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.events[0].id, 1);
        assert_eq!(result.events[1].id, 2);
    }
}
