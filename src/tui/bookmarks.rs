use serde::{Deserialize, Serialize};

/// A named position in the edit timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub label: String,
    pub edit_index: usize,
    pub timestamp: i64, // unix ms when created
}

/// Manages bookmarks for a session.
pub struct BookmarkManager {
    pub bookmarks: Vec<Bookmark>,
}

impl BookmarkManager {
    pub fn new() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }

    /// Add a bookmark at the given edit index.
    pub fn add(&mut self, label: String, edit_index: usize) {
        let timestamp = chrono::Utc::now().timestamp_millis();
        self.bookmarks.push(Bookmark {
            label,
            edit_index,
            timestamp,
        });
    }

    /// Remove bookmark at index. No-op if out of bounds.
    pub fn remove(&mut self, idx: usize) {
        if idx < self.bookmarks.len() {
            self.bookmarks.remove(idx);
        }
    }

    /// Get all bookmarks sorted by edit_index (descending, newest first).
    pub fn sorted(&self) -> Vec<&Bookmark> {
        let mut refs: Vec<&Bookmark> = self.bookmarks.iter().collect();
        refs.sort_by(|a, b| b.edit_index.cmp(&a.edit_index));
        refs
    }

    /// Save bookmarks to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self.bookmarks)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Load bookmarks from a JSON file.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let bookmarks: Vec<Bookmark> = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(Self { bookmarks })
    }

    /// Return the number of bookmarks.
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// Whether there are no bookmarks.
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn new_manager_is_empty() {
        let mgr = BookmarkManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn add_increases_count() {
        let mut mgr = BookmarkManager::new();
        mgr.add("first".to_string(), 10);
        assert_eq!(mgr.len(), 1);
        mgr.add("second".to_string(), 20);
        assert_eq!(mgr.len(), 2);
    }

    #[test]
    fn add_populates_fields() {
        let mut mgr = BookmarkManager::new();
        mgr.add("checkpoint alpha".to_string(), 42);
        let bm = &mgr.bookmarks[0];
        assert_eq!(bm.label, "checkpoint alpha");
        assert_eq!(bm.edit_index, 42);
        assert!(bm.timestamp > 0);
    }

    #[test]
    fn remove_valid_index() {
        let mut mgr = BookmarkManager::new();
        mgr.add("a".to_string(), 1);
        mgr.add("b".to_string(), 2);
        mgr.add("c".to_string(), 3);
        mgr.remove(1);
        assert_eq!(mgr.len(), 2);
        assert_eq!(mgr.bookmarks[0].label, "a");
        assert_eq!(mgr.bookmarks[1].label, "c");
    }

    #[test]
    fn remove_out_of_bounds_is_noop() {
        let mut mgr = BookmarkManager::new();
        mgr.add("only".to_string(), 5);
        mgr.remove(10);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn sorted_returns_descending_edit_index() {
        let mut mgr = BookmarkManager::new();
        mgr.add("low".to_string(), 5);
        mgr.add("high".to_string(), 42);
        mgr.add("mid".to_string(), 18);
        let sorted = mgr.sorted();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].edit_index, 42);
        assert_eq!(sorted[1].edit_index, 18);
        assert_eq!(sorted[2].edit_index, 5);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("vibetracer_bookmark_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bookmarks.json");

        let mut mgr = BookmarkManager::new();
        mgr.add("session start".to_string(), 0);
        mgr.add("things went wrong".to_string(), 42);
        mgr.save(&path).expect("save should succeed");

        let loaded = BookmarkManager::load(&path).expect("load should succeed");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.bookmarks[0].label, "session start");
        assert_eq!(loaded.bookmarks[0].edit_index, 0);
        assert_eq!(loaded.bookmarks[1].label, "things went wrong");
        assert_eq!(loaded.bookmarks[1].edit_index, 42);

        // cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let path = PathBuf::from("/tmp/vibetracer_does_not_exist_bookmark.json");
        assert!(BookmarkManager::load(&path).is_err());
    }

    #[test]
    fn default_trait() {
        let mgr = BookmarkManager::default();
        assert!(mgr.is_empty());
    }
}
