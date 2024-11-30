use ratatui::{Frame, layout::{Constraint, Direction, Layout}, widgets::Paragraph};
use crate::ui::components::feed::render_feed;
use crate::ui::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    render_feed(f, app);

    let status = if app.loading {
        "Loading..."
    } else if let Some(err) = &app.error {
        err
    } else {
        &format!("Press q to quit, j/k to navigate {} / {}", app.selected_index + 1, app.posts.len())
    };

    f.render_widget(
        Paragraph::new(status),
        chunks[1],
    );
}
