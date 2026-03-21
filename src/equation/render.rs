use std::process::Command;

/// Available rendering backends for LaTeX equations.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderBackend {
    /// Full LaTeX rendering via the Tectonic engine.
    Tectonic,
    /// Unicode substitution-based rendering (always available).
    Unicode,
}

/// Detect which rendering backend is available.
///
/// Tries `tectonic --version`; falls back to Unicode if unavailable.
pub fn detect_backend() -> RenderBackend {
    let ok = Command::new("tectonic")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        RenderBackend::Tectonic
    } else {
        RenderBackend::Unicode
    }
}

/// Render a LaTeX string as a Unicode approximation.
///
/// Applies a series of string substitutions for common LaTeX constructs.
pub fn render_unicode(latex: &str) -> String {
    let mut s = latex.to_string();

    // Fractions: \frac{...}{...} → .../...
    // Remove \frac{ and replace }{ with /
    s = s.replace(r"\frac{", "");
    s = s.replace("}{", "/");

    // Integrals and sums
    s = s.replace(r"\int", "∫");
    s = s.replace(r"\sum", "∑");
    s = s.replace(r"\sqrt", "√");
    s = s.replace(r"\infty", "∞");

    // Greek letters
    s = s.replace(r"\pi", "π");
    s = s.replace(r"\alpha", "α");
    s = s.replace(r"\beta", "β");
    s = s.replace(r"\gamma", "γ");
    s = s.replace(r"\delta", "δ");
    s = s.replace(r"\Delta", "Δ");

    // Superscripts
    s = s.replace("^2", "²");
    s = s.replace("^3", "³");

    // Subscripts
    s = s.replace("_0", "₀");
    s = s.replace("_1", "₁");
    s = s.replace("_2", "₂");

    // Operators
    s = s.replace(r"\cdot", "·");
    s = s.replace(r"\times", "×");

    // Comparators
    s = s.replace(r"\leq", "≤");
    s = s.replace(r"\geq", "≥");
    s = s.replace(r"\neq", "≠");
    s = s.replace(r"\approx", "≈");

    // Strip remaining braces and backslashes from unknown commands
    s = s.replace('{', "");
    s = s.replace('}', "");

    // Strip remaining backslash-commands (e.g. \foo → foo)
    // We do a simple pass: remove lone backslashes
    s = s.replace('\\', "");

    s
}

/// Render a LaTeX string as a PNG image using Tectonic.
///
/// This is a stub — full implementation deferred.
/// Returns `None` in all cases.
pub fn render_png(_latex: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_unicode_frac() {
        let result = render_unicode(r"\frac{F}{m}");
        assert!(result.contains('/'), "expected fraction slash in: {result}");
    }

    #[test]
    fn test_render_unicode_greek() {
        let result = render_unicode(r"\pi");
        assert_eq!(result, "π");
    }

    #[test]
    fn test_render_unicode_integral() {
        let result = render_unicode(r"\int f dx");
        assert!(result.contains('∫'), "expected integral in: {result}");
    }

    #[test]
    fn test_render_unicode_superscript() {
        let result = render_unicode("E = mc^2");
        assert!(result.contains('²'), "expected superscript 2 in: {result}");
    }

    #[test]
    fn test_render_png_stub() {
        assert_eq!(render_png("x^2"), None);
    }
}
