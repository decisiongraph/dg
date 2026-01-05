//! Debug MathML output

use latex2mathml::{latex_to_mathml, DisplayStyle};

fn main() {
    let formulas = [
        ("derivative", r"\frac{d}{dx} x^n = nx^{n-1}"),
        ("sum", r"\sum_{n=1}^{\infty} \frac{1}{n^2}"),
    ];

    for (name, latex) in formulas {
        println!("=== {} ===", name);
        println!("LaTeX: {}", latex);
        let mathml = latex_to_mathml(latex, DisplayStyle::Inline).unwrap();
        println!("MathML:\n{}\n", mathml);
    }
}
