use crate::app::{App, HealthStatus};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const PKG_NAME: &str = env!("CARGO_PKG_NAME");
    
    let clusters_count = app.config.clusters.len() as u16;
    let status_height: u16 = if app.status_message.is_some() { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(clusters_count + 2),
                Constraint::Length(status_height),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    // Display package name and version above the selection box
    let title = Paragraph::new(format!("🚀 {} v{}", PKG_NAME, VERSION))
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[1]);

    let items: Vec<ListItem> = app
        .config
        .clusters
        .iter()
        .map(|cluster| {
            let icon = match app.health_status.get(&cluster.name) {
                Some(HealthStatus::Ok) => "✅",
                Some(HealthStatus::Failed) => "❌",
                None => "",
            };
            ListItem::new(format!("{}{}", icon, cluster.name))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::LightGreen).fg(Color::Black));

    f.render_stateful_widget(list, chunks[2], &mut app.cluster_list_state);

    if let Some(status) = &app.status_message {
        let status_line = Paragraph::new(status.clone())
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(status_line, chunks[3]);
    }

    let help_message = Paragraph::new(
        "Enter/E to Edit, A to Add, R to Rotate Credentials, H to Health Check, D to Delete, Q to quit",
    )
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center);
    f.render_widget(help_message, chunks[4]);
}
