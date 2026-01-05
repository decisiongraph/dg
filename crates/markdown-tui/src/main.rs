//! Interactive markdown viewer demo

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use std::io;

use markdown_tui::options::RenderOptions;
use markdown_tui::stateful::{MarkdownState, StatefulMarkdownWidget};

const DEMO_MARKDOWN: &str = r#"# markdown-tui Demo

Render **GitHub Flavored Markdown** beautifully in your terminal.

## Features

- Full GFM support
- Syntax highlighting
- Unicode box-drawing tables
- Task lists: ☑/☐
- Math rendering

### Inline Styles

This is **bold**, *italic*, and ~~strikethrough~~. Inline `code` too.

## Code Block

```rust
fn main() {
    println!("Hello, world!");
}
```

## Table

| Feature | Status |
|---------|--------|
| Headings | Done |
| Lists | Done |
| Tables | Done |
| Code | Done |

## List Types

### Unordered
- First item
- Second item
  - Nested item
  - Another nested

### Ordered
1. First
2. Second
3. Third

### Task List
- [x] Implement parser
- [x] Add rendering
- [ ] Write tests
- [ ] Publish crate

## Blockquote

> This is a blockquote.
> It can span multiple lines.

> [!NOTE]
> This is a callout note.

> [!WARNING]
> Be careful with this!

---

## Math

```math
\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
```

*End of demo*
"#;

struct App {
    state: MarkdownState,
    source: String,
}

impl App {
    fn new() -> Self {
        let source = DEMO_MARKDOWN.to_string();
        let mut state = MarkdownState::new();
        let opts = RenderOptions {
            width: 80,
            ..Default::default()
        };
        state.set_content(&source, &opts);
        Self { state, source }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => {
                    app.state.scroll_down(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.state.scroll_up(1);
                }
                KeyCode::PageDown => {
                    app.state.scroll_down(10);
                }
                KeyCode::PageUp => {
                    app.state.scroll_up(10);
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    app.state.scroll = 0;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    let max = app.state.total_lines.saturating_sub(1) as u16;
                    app.state.scroll = max;
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(5),    // Content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Title bar
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " markdown-tui ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " Interactive Markdown Viewer",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(title, chunks[0]);

    // Content
    let content_area = chunks[1];
    let opts = RenderOptions {
        width: content_area.width as usize,
        ..Default::default()
    };
    app.state.set_content(&app.source, &opts);

    let md_widget = StatefulMarkdownWidget::new().style(Style::default().fg(Color::White));

    md_widget.render(content_area, f.buffer_mut(), &mut app.state);

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" Line {}/{} ", app.state.scroll + 1, app.state.total_lines),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        Span::styled(
            " j/k: scroll | PgUp/PgDn | q: quit ",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(status, chunks[2]);
}
