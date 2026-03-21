use similar::TextDiff;

/// Result of computing a diff between two text versions.
pub struct DiffResult {
    pub patch: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

/// Compute a unified diff between `old` and `new` for the given `filename`.
/// Returns an empty patch and zero counts if the content is identical.
pub fn compute_diff(old: &str, new: &str, filename: &str) -> DiffResult {
    if old == new {
        return DiffResult {
            patch: String::new(),
            lines_added: 0,
            lines_removed: 0,
        };
    }

    let diff = TextDiff::from_lines(old, new);
    let mut lines_added: u32 = 0;
    let mut lines_removed: u32 = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => lines_added += 1,
            similar::ChangeTag::Delete => lines_removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }

    let patch = diff
        .unified_diff()
        .header(&format!("a/{}", filename), &format!("b/{}", filename))
        .to_string();

    DiffResult {
        patch,
        lines_added,
        lines_removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_unified_diff() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2 modified\nline3\nnew line4\n";
        let result = compute_diff(old, new, "test.txt");

        assert!(!result.patch.is_empty(), "Patch should not be empty");
        assert!(result.patch.contains('+'), "Patch should contain added lines");
        assert!(result.patch.contains('-'), "Patch should contain removed lines");
        assert!(result.lines_added > 0, "Should have lines added");
        assert!(result.lines_removed > 0, "Should have lines removed");
    }

    #[test]
    fn test_no_diff_when_identical() {
        let content = "same content\nno changes here\n";
        let result = compute_diff(content, content, "test.txt");

        assert!(result.patch.is_empty(), "Patch should be empty for identical content");
        assert_eq!(result.lines_added, 0, "Should have zero lines added");
        assert_eq!(result.lines_removed, 0, "Should have zero lines removed");
    }
}
