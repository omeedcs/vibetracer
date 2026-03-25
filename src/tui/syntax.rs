use ratatui::style::Color as RatColor;
use syntect::highlighting::{
    Color as SynColor, ScopeSelectors, StyleModifier, Theme as SynTheme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

use crate::theme::Theme;

// ── Public types ────────────────────────────────────────────────────────────

/// A single styled segment of a highlighted line.
#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub text: String,
    pub fg: RatColor,
    pub bold: bool,
    pub italic: bool,
}

/// A highlighted line is a sequence of styled segments.
pub type HighlightedLine = Vec<StyledSegment>;

// ── Highlighter ─────────────────────────────────────────────────────────────

/// Wraps syntect and provides syntax-highlighted lines using vibetracer themes.
pub struct Highlighter {
    syntax_set: SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// Create a new highlighter with the default syntax definitions.
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// Highlight the given file content using the vibetracer theme.
    ///
    /// Detects syntax from the file extension. Returns one `HighlightedLine` per
    /// line in `content`.
    pub fn highlight(&self, filename: &str, content: &str, theme: &Theme) -> Vec<HighlightedLine> {
        let syn_theme = build_syn_theme(theme);

        // Try to detect syntax from the file extension, fall back to plain text.
        let syntax = self
            .syntax_set
            .find_syntax_for_file(filename)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let syn_highlighter = syntect::highlighting::Highlighter::new(&syn_theme);
        let mut highlight_state = syntect::highlighting::HighlightState::new(
            &syn_highlighter,
            syntect::parsing::ScopeStack::new(),
        );
        let mut parse_state = syntect::parsing::ParseState::new(syntax);

        let default_fg = to_syn_color(theme.fg);

        content
            .lines()
            .map(|line| {
                let ops = parse_state
                    .parse_line(line, &self.syntax_set)
                    .unwrap_or_default();
                let regions = syntect::highlighting::HighlightIterator::new(
                    &mut highlight_state,
                    &ops,
                    line,
                    &syn_highlighter,
                )
                .collect::<Vec<_>>();

                regions
                    .into_iter()
                    .map(|(style, text)| {
                        let fg_color = if style.foreground.a == 0 {
                            default_fg
                        } else {
                            style.foreground
                        };

                        StyledSegment {
                            text: text.to_string(),
                            fg: syn_to_rat_color(fg_color),
                            bold: style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::BOLD),
                            italic: style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::ITALIC),
                        }
                    })
                    .collect()
            })
            .collect()
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Convert a ratatui `Color` to a syntect `Color`.
pub fn to_syn_color(c: RatColor) -> SynColor {
    match c {
        RatColor::Rgb(r, g, b) => SynColor { r, g, b, a: 0xFF },
        _ => SynColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
    }
}

/// Convert a syntect `Color` to a ratatui `Color`.
pub fn syn_to_rat_color(c: SynColor) -> RatColor {
    RatColor::Rgb(c.r, c.g, c.b)
}

/// Build a `ThemeItem` for the given scope string and color.
pub fn theme_item(scope_str: &str, color: RatColor, italic: bool) -> ThemeItem {
    let selector: ScopeSelectors = scope_str.parse().unwrap();
    let mut font_style = syntect::highlighting::FontStyle::empty();
    if italic {
        font_style |= syntect::highlighting::FontStyle::ITALIC;
    }
    ThemeItem {
        scope: selector,
        style: StyleModifier {
            foreground: Some(to_syn_color(color)),
            background: None,
            font_style: Some(font_style),
        },
    }
}

/// Build a syntect `Theme` from a vibetracer `Theme` by mapping semantic scopes.
fn build_syn_theme(theme: &Theme) -> SynTheme {
    let transparent = SynColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    let settings = ThemeSettings {
        foreground: Some(to_syn_color(theme.fg)),
        background: Some(transparent),
        ..Default::default()
    };

    let scopes = vec![
        // Primary scope mappings
        theme_item("keyword", theme.accent_warm, false),
        theme_item("storage", theme.accent_warm, false),
        theme_item("constant.numeric", theme.accent_warm, false),
        theme_item("string", theme.accent_green, false),
        theme_item("comment", theme.fg_dim, true),
        theme_item("entity.name.function", theme.accent_blue, false),
        theme_item("entity.name.type", theme.accent_purple, false),
        theme_item("entity.name.class", theme.accent_purple, false),
        theme_item("variable.parameter", theme.fg, false),
        theme_item("punctuation", theme.fg_muted, false),
    ];

    SynTheme {
        name: Some("vibetracer".to_string()),
        author: None,
        settings,
        scopes,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn highlighter_creates_without_panic() {
        let _h = Highlighter::new();
    }

    #[test]
    fn highlight_rust_code() {
        let h = Highlighter::new();
        let theme = Theme::dark();
        let code = "fn main() {\n    let x = 42;\n    println!(\"hello\");\n}\n";
        let lines = h.highlight("test.rs", code, &theme);
        // 4 non-empty lines (the trailing newline produces no extra line from .lines())
        assert_eq!(lines.len(), 4, "expected 4 highlighted lines");
        for (i, line) in lines.iter().enumerate() {
            assert!(
                !line.is_empty(),
                "line {} should have at least one segment",
                i
            );
        }
    }

    #[test]
    fn highlight_unknown_extension_falls_back() {
        let h = Highlighter::new();
        let theme = Theme::dark();
        let code = "hello world\nsecond line\nthird line\n";
        let lines = h.highlight("file.unknownext12345", code, &theme);
        assert_eq!(lines.len(), 3, "expected 3 lines for plain text fallback");
        for (i, line) in lines.iter().enumerate() {
            assert!(
                !line.is_empty(),
                "line {} should have at least one segment",
                i
            );
        }
    }
}
