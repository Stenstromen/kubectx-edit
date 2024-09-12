mod app;
mod ui;
mod input;
mod config;
mod types;

use app::App;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::{io, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let (config, kubeconfig_path) = config::load_config()?;
    let mut app = App::new(config, kubeconfig_path);

    loop {
        if app.needs_redraw {
            terminal.clear()?;
            app.needs_redraw = false;
        }

        terminal.draw(|f| ui::draw(f, &mut app))?;

        if input::handle_input(&mut app)? {
            break;
        }
    }

    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    Ok(())
}
