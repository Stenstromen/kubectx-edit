use ratatui::{
    backend::Backend,
    layout::{Layout, Constraint, Direction, Rect, Alignment},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Clear, Paragraph},
    Frame,
};
use crate::app::App;

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let clusters_count = app.config.clusters.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(clusters_count),
                Constraint::Length(1),
            ].as_ref()
        )
        .split(f.size());

    let items: Vec<ListItem> = app.config.clusters
        .iter()
        .map(|cluster| ListItem::new(cluster.name.clone()))
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(Color::LightGreen));

    f.render_stateful_widget(list, chunks[2], &mut app.cluster_list_state);

    let help_message = Paragraph::new("Enter to Edit, A to Add, Q to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(help_message, chunks[3]);

    if app.show_menu {
        let menu_items = vec!["Edit", "Delete"];
        let menu = List::new(menu_items.into_iter().map(ListItem::new).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::LightGreen));

        let area = centered_rect(30, 30, f.size());
        f.render_widget(Clear, area);
        f.render_stateful_widget(menu, area, &mut app.menu_state);
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
