/// A detected equation in source code.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedEquation {
    /// 1-indexed line number where the equation was found.
    pub line: usize,
    /// Extracted LaTeX text.
    pub latex: String,
    /// The raw comment line that contained the equation.
    pub raw_comment: String,
}

/// Extract equations from source code.
///
/// Detects:
/// - `// @eq: ...`, `# @eq: ...`, `/// @eq: ...` annotations
/// - `$$...$$` display math delimiters (anywhere)
/// - `$...$` inline math — only on comment lines (starting with //, #, ///, or *)
pub fn extract_equations(source: &str) -> Vec<DetectedEquation> {
    let mut results = Vec::new();

    for (zero_idx, raw_line) in source.lines().enumerate() {
        let line_num = zero_idx + 1;
        let trimmed = raw_line.trim();

        // Check for @eq: annotation
        if let Some(eq_text) = find_eq_annotation(trimmed) {
            results.push(DetectedEquation {
                line: line_num,
                latex: eq_text.trim().to_string(),
                raw_comment: raw_line.to_string(),
            });
            continue;
        }

        // Check for $$...$$ display math (anywhere in the line)
        if let Some(latex) = find_display_math(trimmed) {
            results.push(DetectedEquation {
                line: line_num,
                latex,
                raw_comment: raw_line.to_string(),
            });
            continue;
        }

        // Check for $...$ inline math — only on comment lines
        if is_comment_line(trimmed) {
            if let Some(latex) = find_inline_math(trimmed) {
                results.push(DetectedEquation {
                    line: line_num,
                    latex,
                    raw_comment: raw_line.to_string(),
                });
            }
        }
    }

    results
}

/// Returns the text after `@eq:` if the line contains such an annotation.
fn find_eq_annotation(line: &str) -> Option<String> {
    // Supported prefixes: `// @eq:`, `# @eq:`, `/// @eq:`
    let prefixes = ["/// @eq:", "// @eq:", "# @eq:"];
    for prefix in &prefixes {
        if let Some(rest) = line.find("@eq:").map(|_| ()) {
            let _ = rest;
            // More precise: find exactly "@eq:" possibly after a comment marker
            if let Some(pos) = line.find("@eq:") {
                // Ensure it is in a comment context (line starts with //, #, ///, or *)
                let before = line[..pos].trim();
                if before.is_empty()
                    || before == "//"
                    || before == "///"
                    || before == "#"
                    || before == "*"
                    || before.ends_with("//")
                    || before.ends_with("///")
                    || before.ends_with('#')
                    || before.ends_with('*')
                {
                    let after = &line[pos + "@eq:".len()..];
                    let _ = prefix;
                    return Some(after.trim().to_string());
                }
            }
        }
    }
    None
}

/// Returns LaTeX from `$$...$$` delimiters if present.
fn find_display_math(line: &str) -> Option<String> {
    let start = line.find("$$")?;
    let rest = &line[start + 2..];
    let end = rest.find("$$")?;
    Some(rest[..end].trim().to_string())
}

/// Returns LaTeX from `$...$` delimiters (single dollar signs) if present.
/// Only a single match per line — returns the first one found.
fn find_inline_math(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            // Make sure this is not a $$ (display math — handled separately)
            if i + 1 < bytes.len() && bytes[i + 1] == b'$' {
                // Skip display math delimiter
                i += 2;
                // Skip to closing $$
                while i + 1 < bytes.len() {
                    if bytes[i] == b'$' && bytes[i + 1] == b'$' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            // Single $: look for closing $
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                if bytes[j] == b'$' {
                    if j == start {
                        // Empty — skip
                        break;
                    }
                    let inner = &line[start..j];
                    return Some(inner.trim().to_string());
                }
                j += 1;
            }
        }
        i += 1;
    }
    None
}

/// Returns true if the line is a comment line (starts with //, #, ///, or *).
fn is_comment_line(line: &str) -> bool {
    line.starts_with("///")
        || line.starts_with("//")
        || line.starts_with('#')
        || line.starts_with('*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_basic() {
        let src = "// @eq: E = mc^2\nlet x = 1;";
        let eqs = extract_equations(src);
        assert_eq!(eqs.len(), 1);
        assert_eq!(eqs[0].latex, "E = mc^2");
        assert_eq!(eqs[0].line, 1);
    }

    #[test]
    fn test_display_math() {
        let src = "// $$a + b = c$$";
        let eqs = extract_equations(src);
        assert_eq!(eqs.len(), 1);
        assert_eq!(eqs[0].latex, "a + b = c");
    }

    #[test]
    fn test_inline_math_on_comment() {
        let src = "// compute $x^2 + y^2$ here";
        let eqs = extract_equations(src);
        assert_eq!(eqs.len(), 1);
        assert_eq!(eqs[0].latex, "x^2 + y^2");
    }

    #[test]
    fn test_inline_math_not_on_code_line() {
        // A code line with a $ (e.g. shell command) should not match
        let src = "let price = $100;";
        let eqs = extract_equations(src);
        assert_eq!(eqs.len(), 0);
    }
}
