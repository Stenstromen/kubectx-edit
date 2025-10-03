use crate::app::App;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let clusters_count = app.config.clusters.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(clusters_count + 2),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    let items: Vec<ListItem> = app
        .config
        .clusters
        .iter()
        .map(|cluster| ListItem::new(cluster.name.clone()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::LightGreen).fg(Color::Black));

    f.render_stateful_widget(list, chunks[2], &mut app.cluster_list_state);

    let help_message = Paragraph::new(
        "Enter/E to Edit, A to Add, R to Rotate Credentials, D to Delete, Q to quit",
    )
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center);
    f.render_widget(help_message, chunks[3]);
}
