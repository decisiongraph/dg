//! Render math equations to text using ratatui's TestBackend

use markdown_tui::MathWidget;
use ratatui::{
    backend::TestBackend,
    style::Style,
    widgets::{Block, Borders},
    Terminal,
};

const EXAMPLES: &[(&str, &str)] = &[
    (
        "Quadratic Formula",
        r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    ),
    ("Euler's Identity", r"e^{i\pi} + 1 = 0"),
    (
        "Integral",
        r"\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}",
    ),
    (
        "Sum",
        r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
    ),
    ("Greek Letters", r"\alpha + \beta = \gamma"),
    ("Fraction", r"\frac{a + b}{c + d}"),
    ("Square Root", r"\sqrt{x^2 + y^2}"),
];

fn main() {
    for (name, latex) in EXAMPLES {
        println!("═══ {} ═══", name);
        println!("LaTeX: {}", latex);
        println!();

        // Create a test backend with enough space
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = MathWidget::new(latex)
                    .style(Style::default())
                    .block(Block::default().borders(Borders::ALL).title("Rendered"));
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Print the buffer contents
        let buffer = terminal.backend().buffer();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                let cell = buffer.cell((x, y)).unwrap();
                line.push_str(cell.symbol());
            }
            // Trim trailing spaces but keep the line
            println!("{}", line.trim_end());
        }
        println!();
    }
}
