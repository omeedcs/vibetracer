use std::collections::HashMap;

/// Manages the mapping between a global playhead position and per-file
/// playhead positions. Supports detaching individual files so they can
/// be scrubbed independently of the global timeline.
pub struct PlayheadManager {
    /// Current global playhead position (index into the flat edit list).
    global: usize,
    /// For each file, the sorted list of global edit indices that belong to it.
    file_edit_indices: HashMap<String, Vec<usize>>,
    /// Files that have been detached: maps filename -> per-file position index.
    detached: HashMap<String, usize>,
}

impl PlayheadManager {
    pub fn new() -> Self {
        Self {
            global: 0,
            file_edit_indices: HashMap::new(),
            detached: HashMap::new(),
        }
    }

    /// Register or replace the global edit indices for a file.
    /// The indices must be sorted in ascending order.
    pub fn register_file(&mut self, file: &str, global_indices: Vec<usize>) {
        self.file_edit_indices
            .insert(file.to_string(), global_indices);
    }

    /// Move the global playhead to a new position.
    pub fn set_global(&mut self, pos: usize) {
        self.global = pos;
    }

    /// Return the current global playhead position.
    pub fn global(&self) -> usize {
        self.global
    }

    /// Return the per-file playhead position for the given file.
    ///
    /// - For detached files: returns the stored detached position.
    /// - For attached files: counts how many of the file's edits are at or
    ///   before the current global position (i.e. the number of the file's
    ///   edits that have "happened" by the global playhead).
    ///
    /// Returns `None` if the file is not registered.
    pub fn file_position(&self, file: &str) -> Option<usize> {
        if let Some(&pos) = self.detached.get(file) {
            return Some(pos);
        }

        let indices = self.file_edit_indices.get(file)?;
        // Count how many of this file's edits are at or before the global pos.
        let count = indices.partition_point(|&idx| idx <= self.global);
        Some(count)
    }

    /// Detach a file so it gets an independent playhead position.
    pub fn detach(&mut self, file: &str, pos: usize) {
        self.detached.insert(file.to_string(), pos);
    }

    /// Reattach a file so it follows the global playhead again.
    pub fn reattach(&mut self, file: &str) {
        self.detached.remove(file);
    }

    /// Whether a file is currently detached.
    pub fn is_detached(&self, file: &str) -> bool {
        self.detached.contains_key(file)
    }

    /// Move a detached file's playhead one step left (earlier edit).
    /// If the file is not detached, this is a no-op.
    pub fn scrub_file_left(&mut self, file: &str) {
        if let Some(pos) = self.detached.get_mut(file) {
            if *pos > 0 {
                *pos -= 1;
            }
        }
    }

    /// Move a detached file's playhead one step right (later edit).
    /// If the file is not detached, this is a no-op.
    pub fn scrub_file_right(&mut self, file: &str) {
        if let Some(pos) = self.detached.get_mut(file) {
            let max = self
                .file_edit_indices
                .get(file)
                .map(|v| v.len())
                .unwrap_or(0);
            if *pos < max {
                *pos += 1;
            }
        }
    }

    /// Return the list of all registered file names.
    pub fn files(&self) -> Vec<&str> {
        self.file_edit_indices.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PlayheadManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> PlayheadManager {
        let mut pm = PlayheadManager::new();
        // Global edit list looks like:
        //   0: a.rs, 1: b.rs, 2: a.rs, 3: a.rs, 4: b.rs, 5: c.rs
        pm.register_file("a.rs", vec![0, 2, 3]);
        pm.register_file("b.rs", vec![1, 4]);
        pm.register_file("c.rs", vec![5]);
        pm
    }

    #[test]
    fn global_to_file_position_at_start() {
        let mut pm = setup();
        pm.set_global(0);
        // a.rs has edits [0,2,3], at global 0: 1 edit at/before pos 0
        assert_eq!(pm.file_position("a.rs"), Some(1));
        // b.rs has edits [1,4], at global 0: 0 edits at/before pos 0
        assert_eq!(pm.file_position("b.rs"), Some(0));
        // c.rs has edits [5], at global 0: 0 edits
        assert_eq!(pm.file_position("c.rs"), Some(0));
    }

    #[test]
    fn global_to_file_position_mid() {
        let mut pm = setup();
        pm.set_global(3);
        // a.rs: edits 0,2,3 are all <= 3, so position = 3
        assert_eq!(pm.file_position("a.rs"), Some(3));
        // b.rs: edit 1 is <= 3, edit 4 is not, so position = 1
        assert_eq!(pm.file_position("b.rs"), Some(1));
        // c.rs: edit 5 > 3, so position = 0
        assert_eq!(pm.file_position("c.rs"), Some(0));
    }

    #[test]
    fn global_to_file_position_at_end() {
        let mut pm = setup();
        pm.set_global(5);
        assert_eq!(pm.file_position("a.rs"), Some(3));
        assert_eq!(pm.file_position("b.rs"), Some(2));
        assert_eq!(pm.file_position("c.rs"), Some(1));
    }

    #[test]
    fn unknown_file_returns_none() {
        let pm = setup();
        assert_eq!(pm.file_position("unknown.rs"), None);
    }

    #[test]
    fn detach_overrides_global() {
        let mut pm = setup();
        pm.set_global(5);
        // Normally a.rs would be at position 3
        assert_eq!(pm.file_position("a.rs"), Some(3));
        // Detach it at position 1
        pm.detach("a.rs", 1);
        assert_eq!(pm.file_position("a.rs"), Some(1));
        assert!(pm.is_detached("a.rs"));
        // b.rs is still attached
        assert!(!pm.is_detached("b.rs"));
        assert_eq!(pm.file_position("b.rs"), Some(2));
    }

    #[test]
    fn reattach_restores_global_tracking() {
        let mut pm = setup();
        pm.set_global(3);
        pm.detach("a.rs", 0);
        assert_eq!(pm.file_position("a.rs"), Some(0));

        pm.reattach("a.rs");
        assert!(!pm.is_detached("a.rs"));
        // Back to global-derived position
        assert_eq!(pm.file_position("a.rs"), Some(3));
    }

    #[test]
    fn scrub_file_left_right() {
        let mut pm = setup();
        pm.detach("a.rs", 2);

        pm.scrub_file_left("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(1));

        pm.scrub_file_left("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(0));

        // Should not go below 0
        pm.scrub_file_left("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(0));

        pm.scrub_file_right("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(1));

        // Move to max (a.rs has 3 edits, max position is 3)
        pm.scrub_file_right("a.rs");
        pm.scrub_file_right("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(3));

        // Should not go above max
        pm.scrub_file_right("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(3));
    }

    #[test]
    fn scrub_noop_for_attached_files() {
        let mut pm = setup();
        pm.set_global(3);
        let before = pm.file_position("a.rs");

        // These should be no-ops since a.rs is not detached
        pm.scrub_file_left("a.rs");
        pm.scrub_file_right("a.rs");

        assert_eq!(pm.file_position("a.rs"), before);
    }

    #[test]
    fn detach_reattach_multiple_files() {
        let mut pm = setup();
        pm.set_global(4);

        pm.detach("a.rs", 1);
        pm.detach("b.rs", 0);

        assert_eq!(pm.file_position("a.rs"), Some(1));
        assert_eq!(pm.file_position("b.rs"), Some(0));

        // Global move should not affect detached files
        pm.set_global(5);
        assert_eq!(pm.file_position("a.rs"), Some(1));
        assert_eq!(pm.file_position("b.rs"), Some(0));

        // c.rs is still attached and should reflect new global
        assert_eq!(pm.file_position("c.rs"), Some(1));

        pm.reattach("a.rs");
        assert_eq!(pm.file_position("a.rs"), Some(3)); // global=5, all 3 a.rs edits <= 5
    }
}
