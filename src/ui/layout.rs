use crate::ui::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    app.update_status(chunks[0].height);

    f.render_widget(&mut app.feed, chunks[0]);

    f.render_widget(Paragraph::new(app.status_line.clone()), chunks[1]);
}
