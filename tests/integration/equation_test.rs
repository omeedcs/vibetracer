use vibetracer::equation::detect::extract_equations;

#[test]
fn test_detect_annotated_equation() {
    let source = "// @eq: F = G * (m1 * m2) / r^2\nfn gravity() {}";
    let equations = extract_equations(source);
    assert_eq!(equations.len(), 1, "expected exactly one equation");
    let eq = &equations[0];
    assert_eq!(eq.latex, "F = G * (m1 * m2) / r^2");
    assert_eq!(eq.line, 1, "expected equation on line 1");
}

#[test]
fn test_detect_latex_delimiters() {
    let source = "/// $a = \\frac{F}{m}$\nfn accel() {}";
    let equations = extract_equations(source);
    assert!(
        !equations.is_empty(),
        "expected at least one equation from inline math delimiter"
    );
    let eq = &equations[0];
    assert!(
        eq.latex.contains('F') || eq.latex.contains("frac"),
        "expected latex content in: {}",
        eq.latex
    );
    assert_eq!(eq.line, 1);
}

#[test]
fn test_detect_no_equations() {
    let source = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let equations = extract_equations(source);
    assert!(
        equations.is_empty(),
        "expected no equations in regular code, got: {:?}",
        equations
    );
}
