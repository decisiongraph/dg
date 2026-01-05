//! Simple example showing LaTeX to terminal Unicode rendering

use markdown_tui::render_latex;

fn main() {
    let examples = [
        ("Simple", r"x + y = z"),
        ("Superscript", r"x^2 + y^2 = z^2"),
        ("Subscript", r"a_1 + a_2 + a_3"),
        ("Fraction", r"\frac{a + b}{c}"),
        ("Square root", r"\sqrt{x^2 + y^2}"),
        ("Greek", r"\alpha + \beta = \gamma"),
        ("Integral", r"\int_0^\infty e^{-x} dx"),
        ("Sum", r"\sum_{i=1}^n i^2"),
        ("Quadratic", r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}"),
        ("Euler", r"e^{i\pi} + 1 = 0"),
    ];

    for (name, latex) in examples {
        println!("── {} ──", name);
        println!("LaTeX: {}", latex);
        println!("Rendered:");
        match render_latex(latex) {
            Ok(rendered) => println!("{}", rendered),
            Err(e) => println!("Error: {}", e),
        }
        println!();
    }
}
