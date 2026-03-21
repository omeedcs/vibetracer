use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{
    BlastRadiusConfig, Config, ManualDependency, WatchdogConfig, WatchdogConstant,
};

/// Directories to skip when walking the project tree.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    ".vibetracer",
];

/// Scan `project_root` and produce a populated `Config`.
pub fn auto_detect_config(project_root: &Path) -> Config {
    let mut config = Config::default();

    let source_files = collect_source_files(project_root);

    let mut constants: Vec<WatchdogConstant> = Vec::new();
    let mut schema_files: Vec<PathBuf> = Vec::new();
    let mut config_files: Vec<PathBuf> = Vec::new();

    // Maps a config file (relative glob-style path) to files that import it.
    let mut config_importers: HashMap<String, Vec<String>> = HashMap::new();

    for path in &source_files {
        let content = match read_limited(path) {
            Some(c) => c,
            None => continue,
        };

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let rel = relative_to(project_root, path);

        // ── Constant detection ───────────────────────────────────────────────
        let found = match ext {
            "py" => detect_python_constants(&content, &rel),
            "rs" => detect_rust_constants(&content, &rel),
            "js" | "ts" | "tsx" | "jsx" => detect_js_constants(&content, &rel),
            _ => Vec::new(),
        };
        constants.extend(found);

        // ── Schema / type definition detection ──────────────────────────────
        let is_schema = match ext {
            "py" => content.contains("BaseModel") || content.contains("@dataclass"),
            "ts" | "tsx" => content.contains("interface ") || has_type_alias_object(&content),
            _ => false,
        };
        if is_schema {
            schema_files.push(path.clone());
        }

        // ── Config file detection ────────────────────────────────────────────
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if stem.contains("config") && matches!(ext, "py" | "rs" | "js" | "ts" | "tsx") {
            config_files.push(path.clone());
        }
    }

    // ── Build import-dependency map for config files ─────────────────────────
    for cfg_path in &config_files {
        let cfg_rel = relative_to(project_root, cfg_path);
        let cfg_stem = cfg_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        for src_path in &source_files {
            if src_path == cfg_path {
                continue;
            }
            let content = match read_limited(src_path) {
                Some(c) => c,
                None => continue,
            };
            if imports_file(&content, &cfg_stem) {
                let src_rel = relative_to(project_root, src_path);
                config_importers
                    .entry(cfg_rel.clone())
                    .or_default()
                    .push(src_rel);
            }
        }
    }

    // ── Keep only the top-10 most interesting constants ──────────────────────
    constants.sort_by(|a, b| {
        let score_a = interest_score(&a.pattern, &a.expected);
        let score_b = interest_score(&b.pattern, &b.expected);
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    constants.dedup_by(|a, b| a.pattern == b.pattern && a.file == b.file);
    constants.truncate(10);

    config.watchdog = WatchdogConfig { constants };

    // ── Blast-radius: schema files → test files ──────────────────────────────
    let mut manual: Vec<ManualDependency> = Vec::new();

    for schema_path in &schema_files {
        let schema_rel = relative_to(project_root, schema_path);
        // Find test files that likely exercise this schema.
        let test_deps = find_test_files(project_root, &source_files);
        if !test_deps.is_empty() {
            manual.push(ManualDependency {
                source: schema_rel,
                dependents: test_deps,
            });
        }
    }

    // ── Blast-radius: config files → importers ───────────────────────────────
    for (cfg_rel, importers) in config_importers {
        if !importers.is_empty() {
            manual.push(ManualDependency {
                source: cfg_rel,
                dependents: importers,
            });
        }
    }

    // Deduplicate by source.
    manual.dedup_by(|a, b| a.source == b.source);
    manual.truncate(20);

    config.blast_radius = BlastRadiusConfig {
        auto_detect: true,
        manual,
    };

    config
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Walk `root` and collect all source files, skipping ignored dirs and files > 1 MB.
fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    collect_recursive(root, &mut result);
    result
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if path.is_dir() {
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            collect_recursive(&path, out);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(
                ext,
                "py" | "rs" | "ts" | "tsx" | "js" | "jsx" | "go" | "java"
            ) {
                out.push(path);
            }
        }
    }
}

/// Read a file, returning `None` if it is larger than 1 MB or unreadable.
fn read_limited(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 1_048_576 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Return `path` as a string relative to `root`, falling back to the full path.
fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// Detect `ALL_CAPS_NAME = <number>` in Python source.
fn detect_python_constants(content: &str, rel_path: &str) -> Vec<WatchdogConstant> {
    let mut results = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        // Match: IDENTIFIER = <number>  (no leading spaces so we skip class members etc.)
        if let Some((name, value)) = parse_assignment(line, "=") {
            if is_all_caps(name) && is_numeric(value) {
                let pattern = format!(r"{}\s*=\s*([\d.eE+\-]+)", regex_escape(name));
                results.push(WatchdogConstant {
                    file: format!("**/{}", filename_part(rel_path)),
                    pattern,
                    expected: value.to_string(),
                    severity: default_severity(name, value),
                });
            }
        }
    }
    results
}

/// Detect `const ALL_CAPS: type = <number>` in Rust source.
fn detect_rust_constants(content: &str, rel_path: &str) -> Vec<WatchdogConstant> {
    let mut results = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("const ") && !line.starts_with("pub const ") {
            continue;
        }
        // pub const NAME: type = value;
        let after_const = line
            .trim_start_matches("pub")
            .trim()
            .trim_start_matches("const")
            .trim();
        if let Some(colon_pos) = after_const.find(':') {
            let name = after_const[..colon_pos].trim();
            if !is_all_caps(name) {
                continue;
            }
            // Find = after the colon
            if let Some(eq_pos) = after_const[colon_pos..].find('=') {
                let rest = after_const[colon_pos + eq_pos + 1..]
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if is_numeric(rest) {
                    let pattern = format!(
                        r"const\s+{}\s*:\s*\w+\s*=\s*([\d.eE+\-_]+)",
                        regex_escape(name)
                    );
                    results.push(WatchdogConstant {
                        file: format!("**/{}", filename_part(rel_path)),
                        pattern,
                        expected: rest.replace('_', ""),
                        severity: default_severity(name, rest),
                    });
                }
            }
        }
    }
    results
}

/// Detect `const ALL_CAPS = <number>` in JS/TS source.
fn detect_js_constants(content: &str, rel_path: &str) -> Vec<WatchdogConstant> {
    let mut results = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        // Match: const NAME = value  or  export const NAME = value
        let stripped = line
            .trim_start_matches("export")
            .trim()
            .trim_start_matches("const")
            .trim();
        if stripped == line.trim() && !line.starts_with("const ") {
            continue;
        }
        if let Some((name, value)) = parse_assignment(stripped, "=") {
            if is_all_caps(name) && is_numeric(value) {
                let pattern = format!(r"const\s+{}\s*=\s*([\d.eE+\-]+)", regex_escape(name));
                results.push(WatchdogConstant {
                    file: format!("**/{}", filename_part(rel_path)),
                    pattern,
                    expected: value.trim_end_matches(';').to_string(),
                    severity: default_severity(name, value),
                });
            }
        }
    }
    results
}

/// Split `line` on the first occurrence of `sep`, returning (lhs, rhs) trimmed.
fn parse_assignment<'a>(line: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let pos = line.find(sep)?;
    let lhs = line[..pos].trim();
    let rhs = line[pos + sep.len()..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    Some((lhs, rhs))
}

/// True if `name` is ALL_CAPS (with optional underscores/digits).
fn is_all_caps(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && name.chars().any(|c| c.is_ascii_uppercase())
}

/// True if the value looks like a plain number (int or float, optional sign/exponent).
fn is_numeric(value: &str) -> bool {
    let v = value.trim().trim_end_matches(';').trim();
    // Strip Rust-style numeric underscores (e.g. 1_000_000).
    let no_underscores: String = v.chars().filter(|&c| c != '_').collect();
    // Strip a trailing Rust type suffix like u32, f64, i64, usize, etc.
    // Rust suffixes can contain digits (u32, i64) so we cannot just trim alpha chars.
    // Instead: trim the longest trailing sequence that looks like a type suffix.
    let stripped = strip_numeric_type_suffix(&no_underscores);
    stripped.parse::<f64>().is_ok()
}

/// Remove a Rust-style numeric type suffix (u8, u32, i64, f64, usize, …).
/// The suffix is the trailing alphabetic+digit run after the last digit in the numeric body.
fn strip_numeric_type_suffix(s: &str) -> &str {
    // Known Rust numeric suffixes.
    let suffixes = [
        "usize", "isize", "u128", "i128", "u64", "i64", "f64", "u32", "i32", "f32", "u16", "i16",
        "u8", "i8",
    ];
    for suffix in &suffixes {
        if let Some(trimmed) = s.strip_suffix(suffix) {
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    s
}

/// Determine severity based on name heuristics.
fn default_severity(name: &str, value: &str) -> String {
    let important_keywords = [
        "RADIUS",
        "SPEED",
        "GRAVITY",
        "PI",
        "EPSILON",
        "TOLERANCE",
        "MAX_ITER",
        "MAX_STEPS",
        "LEARNING_RATE",
        "THRESHOLD",
        "LIMIT",
        "TIMEOUT",
        "PORT",
        "VERSION",
    ];
    let is_float = value.contains('.');
    let name_upper = name.to_ascii_uppercase();
    if is_float || important_keywords.iter().any(|k| name_upper.contains(k)) {
        "critical".to_string()
    } else {
        "warning".to_string()
    }
}

/// Compute an "interest score" to rank constants for the top-10.
/// Higher = more interesting.
fn interest_score(pattern: &str, value: &str) -> f64 {
    let important = [
        "RADIUS",
        "SPEED",
        "GRAVITY",
        "PI",
        "EPSILON",
        "TOLERANCE",
        "RATE",
        "THRESHOLD",
        "LIMIT",
        "TIMEOUT",
        "VERSION",
    ];
    let mut score: f64 = 0.0;
    let pat_upper = pattern.to_ascii_uppercase();
    for kw in &important {
        if pat_upper.contains(kw) {
            score += 10.0;
        }
    }
    if value.contains('.') {
        score += 5.0; // float values are more likely physics/math constants
    }
    score
}

/// Escape a string for use as a literal in a regex pattern.
fn regex_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if "^$.|?*+()[]{}\\".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

/// Return just the filename component of a relative path.
fn filename_part(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// True if the file content imports `module_stem` in any common style.
fn imports_file(content: &str, module_stem: &str) -> bool {
    // Python: import config / from config import / from .config import
    // JS/TS: import ... from './config' / require('./config')
    // Rust: use crate::config / mod config
    let stem_lower = module_stem.to_lowercase();
    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains(&format!("import {}", stem_lower))
            || line_lower.contains(&format!("from {}", stem_lower))
            || line_lower.contains(&format!("from './{}", stem_lower))
            || line_lower.contains(&format!("from \"./{}", stem_lower))
            || line_lower.contains(&format!("require('./{}", stem_lower))
            || line_lower.contains(&format!("require(\"./{}", stem_lower))
            || line_lower.contains(&format!("use crate::{}", stem_lower))
            || line_lower.contains(&format!("mod {}", stem_lower))
        {
            return true;
        }
    }
    false
}

/// Return test files among `source_files` as relative strings.
fn find_test_files(root: &Path, source_files: &[PathBuf]) -> Vec<String> {
    source_files
        .iter()
        .filter(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            name.starts_with("test_")
                || name.ends_with("_test.py")
                || name.ends_with(".test.ts")
                || name.ends_with(".spec.ts")
                || name.ends_with(".test.js")
                || name.ends_with(".spec.js")
                || name.contains("_test.rs")
        })
        .map(|p| relative_to(root, p))
        .collect()
}

/// Check whether a TS/JS file has a `type Foo = { ... }` pattern.
fn has_type_alias_object(content: &str) -> bool {
    content.lines().any(|line| {
        let l = line.trim();
        l.starts_with("type ") && l.contains('=') && l.contains('{')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_all_caps() {
        assert!(is_all_caps("EARTH_RADIUS"));
        assert!(is_all_caps("PI"));
        assert!(is_all_caps("MAX_RETRIES_3"));
        assert!(!is_all_caps("earthRadius"));
        assert!(!is_all_caps("earth_radius"));
        assert!(!is_all_caps(""));
    }

    #[test]
    fn test_is_numeric() {
        assert!(is_numeric("6371.0"));
        assert!(is_numeric("42"));
        assert!(is_numeric("3.14e-5"));
        assert!(is_numeric("100;"));
        assert!(is_numeric("6371_u32"));
        assert!(!is_numeric("\"hello\""));
        assert!(!is_numeric("some_var"));
    }

    #[test]
    fn test_detect_python_constants() {
        let src = "EARTH_RADIUS = 6371.0\nGRAVITY = 9.81\nNAME = \"Alice\"\nlow_var = 1\n";
        let results = detect_python_constants(src, "physics.py");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].expected, "6371.0");
        assert_eq!(results[1].expected, "9.81");
    }

    #[test]
    fn test_detect_rust_constants() {
        let src = "pub const MAX_SIZE: usize = 1024;\npub const PI: f64 = 3.14159;\nconst INTERNAL_VAR: &str = \"hello\";\n";
        let results = detect_rust_constants(src, "lib.rs");
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results
            .iter()
            .map(|r| {
                if r.expected == "1024" {
                    "MAX_SIZE"
                } else {
                    "PI"
                }
            })
            .collect();
        assert!(names.contains(&"MAX_SIZE"));
        assert!(names.contains(&"PI"));
    }

    #[test]
    fn test_detect_js_constants() {
        let src =
            "export const MAX_RETRIES = 3;\nconst TIMEOUT_MS = 5000;\nconst name = \"foo\";\n";
        let results = detect_js_constants(src, "config.ts");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_imports_file() {
        assert!(imports_file("from config import settings", "config"));
        assert!(imports_file("import config", "config"));
        assert!(imports_file("import { Foo } from './config'", "config"));
        assert!(!imports_file("import { Foo } from './utils'", "config"));
    }

    #[test]
    fn test_auto_detect_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = auto_detect_config(dir.path());
        assert!(cfg.watchdog.constants.is_empty());
        assert!(cfg.blast_radius.manual.is_empty());
    }

    #[test]
    fn test_auto_detect_with_python_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("physics.py"),
            "EARTH_RADIUS = 6371.0\nGRAVITY = 9.81\n",
        )
        .unwrap();
        let cfg = auto_detect_config(dir.path());
        assert!(!cfg.watchdog.constants.is_empty());
    }
}
