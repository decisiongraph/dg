//! Render a single equation - pass equation name as argument

use markdown_tui::MathWidget;
use ratatui::{
    backend::TestBackend,
    style::Style,
    widgets::{Block, Borders},
    Terminal,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let examples: &[(&str, &str)] = &[
        ("quadratic", r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}"),
        ("euler", r"e^{i\pi} + 1 = 0"),
        (
            "integral",
            r"\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}",
        ),
        (
            "sum",
            r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
        ),
        ("greek", r"\alpha + \beta = \gamma"),
        ("fraction", r"\frac{a + b}{c + d}"),
        ("sqrt", r"\sqrt{x^2 + y^2}"),
        ("derivative", r"\frac{d}{dx} x^n = nx^{n-1}"),
    ];

    let name = args.get(1).map(|s| s.as_str()).unwrap_or("quadratic");

    let (_, latex) = examples
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or(&examples[0]);

    // Create a test backend
    let backend = TestBackend::new(50, 8);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let widget = MathWidget::new(latex)
                .style(Style::default())
                .block(Block::default().borders(Borders::ALL).title(name));
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
        println!("{}", line.trim_end());
    }
}
