use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use crate::ui::components::post::render_post;
use crate::ui::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Render posts
    let posts_chunk = chunks[0];
    let post_height = 6;
    let visible_posts = (posts_chunk.height / post_height) as usize;

    for (i, post) in app.posts.iter()
        .skip(app.selected_index.saturating_sub(visible_posts/2))
        .take(visible_posts)
        .enumerate() 
    {
        let post_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(post_height)])
            .split(posts_chunk)[0];
        
        render_post(f, post, post_area, i == app.selected_index);
    }

    // Render status line
    let status = if app.loading {
        "Loading..."
    } else if let Some(err) = &app.error {
        err
    } else {
        "Press q to quit, j/k to navigate"
    };
    
    f.render_widget(
        ratatui::widgets::Paragraph::new(status),
        chunks[1]
    );
}
