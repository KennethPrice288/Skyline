use ratatui::{Frame, layout::{Constraint, Direction, Layout}, widgets::Paragraph};
use crate::ui::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    f.render_widget(&app.feed, chunks[0]);

    let status = if app.loading {
        "Loading..."
    } else if let Some(err) = &app.error {
        err
    } else {
        &format!("Press q to quit, j/k to navigate, r to refresh {} / {}", app.feed.selected_index + 1, app.feed.posts.len())
    };

    f.render_widget(
        Paragraph::new(status),
        chunks[1],
    );
}
