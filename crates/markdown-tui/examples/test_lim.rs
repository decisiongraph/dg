//! Test limit rendering

use markdown_tui::MathWidget;
use ratatui::{
    backend::TestBackend,
    style::Style,
    widgets::{Block, Borders},
    Terminal,
};

fn main() {
    let latex = r"\lim_{x \to \infty} \frac{1}{x} = 0";

    let backend = TestBackend::new(50, 8);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let widget = MathWidget::new(latex)
                .style(Style::default())
                .block(Block::default().borders(Borders::ALL).title("limit"));
            f.render_widget(widget, f.area());
        })
        .unwrap();

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
