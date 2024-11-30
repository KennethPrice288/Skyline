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

    // Create constraints for all visible posts
    let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(post_height))
        .take(visible_posts)
        .collect();

    // Split the chunk into multiple areas
    let post_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(posts_chunk);

    for (i, (post, area)) in app.posts.iter()
        .skip(app.selected_index.saturating_sub(visible_posts/2))
        .take(visible_posts)
        .zip(post_areas.iter())
        .enumerate() 
    {    
        render_post(f, post, *area, i == app.selected_index);
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
