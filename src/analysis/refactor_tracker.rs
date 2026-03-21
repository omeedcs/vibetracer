use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

/// Progress of a function rename across a project.
#[derive(Debug, Clone)]
pub struct RenameStatus {
    pub old_name: String,
    pub new_name: String,
    pub remaining_old_refs: usize,
    pub updated_new_refs: usize,
    pub total_sites: usize,
    pub remaining_files: Vec<String>,
}

/// Tracks function renames across a project codebase.
pub struct RefactorTracker {
    /// old_name → RenameStatus
    renames: HashMap<String, RenameStatus>,
}

impl RefactorTracker {
    pub fn new() -> Self {
        Self {
            renames: HashMap::new(),
        }
    }

    /// Compare two source lines. If both contain `fn <name>` and the names differ,
    /// return `(old_name, new_name)`.
    pub fn detect_rename(old_line: &str, new_line: &str) -> Option<(String, String)> {
        let re = Regex::new(r"\bfn\s+(\w+)").ok()?;
        let old_name = re.captures(old_line)?.get(1)?.as_str().to_string();
        let new_name = re.captures(new_line)?.get(1)?.as_str().to_string();
        if old_name != new_name {
            Some((old_name, new_name))
        } else {
            None
        }
    }

    /// Walk `project_root`, count occurrences of `old_name` and `new_name` in all
    /// text files (skipping .git, node_modules, target directories).
    pub fn track_rename(&mut self, project_root: &Path, old_name: &str, new_name: &str) {
        let mut remaining_old_refs = 0usize;
        let mut updated_new_refs = 0usize;
        let mut remaining_files: Vec<String> = Vec::new();

        Self::walk_files(project_root, &mut |path: &Path| {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => return,
            };
            let old_count = Self::count_word(&content, old_name);
            let new_count = Self::count_word(&content, new_name);

            remaining_old_refs += old_count;
            updated_new_refs += new_count;

            if old_count > 0 {
                remaining_files
                    .push(path.to_string_lossy().to_string());
            }
        });

        let total_sites = remaining_old_refs + updated_new_refs;

        self.renames.insert(
            old_name.to_string(),
            RenameStatus {
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
                remaining_old_refs,
                updated_new_refs,
                total_sites,
                remaining_files,
            },
        );
    }

    /// Return the current rename status for `old_name`.
    pub fn get_status(&self, old_name: &str) -> Option<RenameStatus> {
        self.renames.get(old_name).cloned()
    }

    // ── helpers ────────────────────────────────────────────────────────────────

    fn count_word(content: &str, word: &str) -> usize {
        let pattern = format!(r"\b{}\b", regex::escape(word));
        Regex::new(&pattern)
            .map(|re| re.find_iter(content).count())
            .unwrap_or(0)
    }

    fn walk_files(dir: &Path, callback: &mut impl FnMut(&Path)) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if matches!(name, ".git" | "node_modules" | "target") {
                    continue;
                }
                Self::walk_files(&path, callback);
            } else {
                callback(&path);
            }
        }
    }
}

impl Default for RefactorTracker {
    fn default() -> Self {
        Self::new()
    }
}
