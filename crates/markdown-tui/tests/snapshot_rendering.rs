use markdown_tui::{render_latex, render_markdown_plain, render_mathml};

// ── Block rendering ─────────────────────────────────────────────

#[test]
fn heading_h1() {
    insta::assert_snapshot!(render_markdown_plain("# Hello World"));
}

#[test]
fn heading_h2() {
    insta::assert_snapshot!(render_markdown_plain("## Section Title"));
}

#[test]
fn heading_h3() {
    insta::assert_snapshot!(render_markdown_plain("### Subsection"));
}

#[test]
fn heading_with_inline() {
    insta::assert_snapshot!(render_markdown_plain("# Hello **bold** and *italic*"));
}

#[test]
fn unordered_list() {
    insta::assert_snapshot!(render_markdown_plain("- One\n- Two\n- Three"));
}

#[test]
fn ordered_list() {
    insta::assert_snapshot!(render_markdown_plain("1. First\n2. Second\n3. Third"));
}

#[test]
fn nested_list() {
    insta::assert_snapshot!(render_markdown_plain(
        "- Parent\n  - Child\n    - Grandchild"
    ));
}

#[test]
fn task_list() {
    insta::assert_snapshot!(render_markdown_plain("- [x] Done\n- [ ] Todo"));
}

#[test]
fn simple_table() {
    insta::assert_snapshot!(render_markdown_plain(
        "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |"
    ));
}

#[test]
fn aligned_table() {
    insta::assert_snapshot!(render_markdown_plain(
        "| Left | Center | Right |\n|:-----|:------:|------:|\n| a | b | c |"
    ));
}

#[test]
fn code_block_plain() {
    insta::assert_snapshot!(render_markdown_plain("```\nhello world\n```"));
}

#[test]
fn code_block_with_lang() {
    insta::assert_snapshot!(render_markdown_plain("```rust\nfn main() {}\n```"));
}

#[test]
fn inline_code() {
    insta::assert_snapshot!(render_markdown_plain("Use `code` in text"));
}

#[test]
fn simple_blockquote() {
    insta::assert_snapshot!(render_markdown_plain("> quoted text"));
}

#[test]
fn callout_note() {
    insta::assert_snapshot!(render_markdown_plain("> [!NOTE]\n> Some note"));
}

#[test]
fn callout_warning() {
    insta::assert_snapshot!(render_markdown_plain("> [!WARNING]\n> Be careful"));
}

#[test]
fn nested_blockquote() {
    insta::assert_snapshot!(render_markdown_plain("> outer\n> > inner"));
}

#[test]
fn horizontal_rule() {
    insta::assert_snapshot!(render_markdown_plain("---"));
}

#[test]
fn bold_italic_strike() {
    insta::assert_snapshot!(render_markdown_plain("**bold** *italic* ~~strike~~"));
}

#[test]
fn link_rendering() {
    insta::assert_snapshot!(render_markdown_plain("[text](http://example.com)"));
}

#[test]
fn footnote() {
    insta::assert_snapshot!(render_markdown_plain(
        "Text with ref[^1]\n\n[^1]: The footnote definition"
    ));
}

#[test]
fn html_details() {
    insta::assert_snapshot!(render_markdown_plain(
        "<details><summary>Title</summary>Body content</details>"
    ));
}

#[test]
fn html_kbd() {
    insta::assert_snapshot!(render_markdown_plain("Press <kbd>Ctrl</kbd>"));
}

#[test]
fn paragraph_wrapping() {
    let long = "This is a long paragraph that should exceed the default width of eighty characters and trigger word wrapping behavior in the renderer output.";
    insta::assert_snapshot!(render_markdown_plain(long));
}

#[test]
fn math_code_block() {
    insta::assert_snapshot!(render_markdown_plain("```math\n\\frac{a}{b}\n```"));
}

// ── Math rendering ──────────────────────────────────────────────

#[test]
fn math_subscript() {
    insta::assert_snapshot!(render_latex("x_1").unwrap());
}

#[test]
fn math_subsup_combined() {
    insta::assert_snapshot!(render_latex("x_1^2").unwrap());
}

#[test]
fn math_fraction_nested() {
    insta::assert_snapshot!(render_latex(r"\frac{\frac{a}{b}}{c}").unwrap());
}

#[test]
fn math_sqrt() {
    insta::assert_snapshot!(render_latex(r"\sqrt{x^2 + y^2}").unwrap());
}

#[test]
fn math_nthroot() {
    insta::assert_snapshot!(render_latex(r"\sqrt[3]{x}").unwrap());
}

#[test]
fn math_accent_hat() {
    insta::assert_snapshot!(render_latex(r"\hat{x}").unwrap());
}

#[test]
fn math_accent_bar() {
    insta::assert_snapshot!(render_latex(r"\bar{x}").unwrap());
}

#[test]
fn math_lim() {
    insta::assert_snapshot!(render_latex(r"\lim_{x \to 0}").unwrap());
}

#[test]
fn math_sum_limits() {
    insta::assert_snapshot!(render_latex(r"\sum_{i=0}^{n} i").unwrap());
}

#[test]
fn math_greek() {
    insta::assert_snapshot!(render_latex(r"\alpha + \beta = \gamma").unwrap());
}

#[test]
fn math_quadratic() {
    insta::assert_snapshot!(render_latex(r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}").unwrap());
}

#[test]
fn math_euler() {
    insta::assert_snapshot!(render_latex(r"e^{i\pi} + 1 = 0").unwrap());
}

#[test]
fn math_depth_limit() {
    // Build deeply nested MathML fractions directly (bypasses latex2mathml stack)
    let mut inner = "<mn>a</mn>".to_string();
    for _ in 0..100 {
        inner = format!("<mfrac>{}<mn>b</mn></mfrac>", inner);
    }
    let mathml = format!("<math>{}</math>", inner);
    let result = render_mathml(&mathml);
    assert!(
        result.is_err(),
        "expected depth limit error, got: {:?}",
        result
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("recursion depth"),
        "unexpected error: {}",
        err_msg
    );
}

#[test]
fn math_empty_input() {
    // Empty string may produce empty output or error
    let result = render_latex("");
    match result {
        Ok(s) => insta::assert_snapshot!(s),
        Err(e) => insta::assert_snapshot!(format!("ERROR: {}", e)),
    }
}

// ── MathBox baseline clamp ──────────────────────────────────────

#[test]
fn mathbox_baseline_clamp() {
    use markdown_tui::MathBox;
    let b = MathBox::empty(5, 3, 999);
    assert_eq!(b.baseline, 2, "baseline should be clamped to height-1");
}
