use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ralph_tui::TerminalWidget;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tui_term::widget::PseudoTerminal;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut widget = TerminalWidget::new();
    widget.process(b"Hello from TerminalWidget\nLine 2\nLine 3\n");

    loop {
        terminal.draw(|f| {
            let pseudo_term = PseudoTerminal::new(widget.parser().screen());
            f.render_widget(pseudo_term, f.area());
        })?;

        if let Event::Key(key) = event::read()?
            && key.code == KeyCode::Char('q')
        {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
