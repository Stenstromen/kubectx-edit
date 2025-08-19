use crate::app::App;
use crossterm::event::{self, Event, KeyCode};
use std::io;

pub fn handle_input(app: &mut App) -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('a') => app.add_new_kubeconfig()?,
                KeyCode::Char('d') => app.delete_selected(),
                KeyCode::Char('e') => app.edit_selected()?,
                KeyCode::Enter => {
                    app.select();
                    app.edit_selected()?;
                }
                KeyCode::Down => app.next(),
                KeyCode::Up => app.previous(),
                _ => {}
            }
        }
    }
    Ok(false)
}
