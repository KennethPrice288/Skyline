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

    app.update_status();

    match app.view_stack.current_view() {
        super::views::View::Timeline(feed) => {
            f.render_widget(feed, chunks[0])
        },
        super::views::View::Thread(thread) => {
            f.render_widget(thread, chunks[0])
        },
        super::views::View::AuthorFeed(author_feed) => {
            f.render_widget(author_feed, chunks[0])
        },
        super::views::View::Notifications(notification_view) => {
            f.render_widget(notification_view, chunks[0])
        },
    }

    f.render_widget(Paragraph::new(app.status_line.clone()), chunks[1]);
}
