mod app;
mod config;
mod health;
mod input;
mod types;
mod ui;

use app::App;
use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    match config::load_config() {
        Ok((config, kubeconfig_path)) => {
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
        }
        Err(e) => {
            // Clean up terminal state before showing error
            disable_raw_mode()?;
            stdout.execute(LeaveAlternateScreen)?;
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    Ok(())
}
