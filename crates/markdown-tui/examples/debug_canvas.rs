//! Debug canvas coordinate extraction

use markdown_tui::MathRenderer;

fn main() {
    let renderer = MathRenderer::new();
    let latex = r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}";
    let mbox = renderer.render_to_box(latex).unwrap();

    println!("MathBox dimensions: {}x{}", mbox.width, mbox.height);

    let lines = mbox.to_lines();
    for (row, line) in lines.iter().enumerate() {
        println!("\nRow {}:", row);
        for (col, ch) in line.chars().enumerate() {
            match ch {
                ' ' => {}
                '─' => println!("  col {}: HLINE", col),
                '╱' => println!("  col {}: DIAG_UP", col),
                '╲' => println!("  col {}: DIAG_DOWN", col),
                '│' => println!("  col {}: VLINE", col),
                _ => println!("  col {}: TEXT '{}'", col, ch),
            }
        }
    }
}
