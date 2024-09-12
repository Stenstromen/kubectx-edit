use std::io;
use crossterm::event::{self, Event, KeyCode};
use crate::app::App;

pub fn handle_input(app: &mut App) -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('a') => app.add_new_kubeconfig()?,
                KeyCode::Down => if app.show_menu {
                    app.menu_next();
                } else {
                    app.next();
                },
                KeyCode::Up => if app.show_menu {
                    app.menu_previous();
                } else {
                    app.previous();
                },
                KeyCode::Enter => {
                    if app.show_menu {
                        match app.menu_state.selected() {
                            Some(0) => {
                                app.edit_selected()?;
                                app.show_menu = false;
                            }
                            Some(1) => app.delete_selected(),
                            _ => {}
                        }
                    } else {
                        app.select();
                        app.toggle_menu();
                    }
                },
                KeyCode::Esc => if app.show_menu {
                    app.toggle_menu();
                },
                _ => {}
            }
        }
    }
    Ok(false)
}
