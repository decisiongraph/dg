//! Debug canvas rendering directly

use ratatui::{
    backend::TestBackend,
    style::Color,
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Line},
        Widget,
    },
    Terminal,
};

fn main() {
    // Create a simple test: draw a horizontal line at different x positions
    let backend = TestBackend::new(30, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let area = f.area();
            println!("Area: {}x{}", area.width, area.height);

            let canvas = Canvas::default()
                .marker(Marker::Braille)
                .x_bounds([0.0, 30.0])
                .y_bounds([0.0, 5.0])
                .paint(|ctx| {
                    // Draw single-cell lines at specific positions
                    for x in [0.0, 5.0, 10.0, 15.0, 20.0, 25.0] {
                        ctx.draw(&Line {
                            x1: x,
                            y1: 2.5,
                            x2: x + 1.0,
                            y2: 2.5,
                            color: Color::White,
                        });
                    }
                });

            canvas.render(area, f.buffer_mut());
        })
        .unwrap();

    // Print the buffer
    let buffer = terminal.backend().buffer();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
        }
        println!("Row {}: '{}'", y, line);
    }
}
