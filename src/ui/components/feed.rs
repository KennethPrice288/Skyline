use ratatui::{Frame, layout::{Constraint, Direction, Layout}, widgets::Paragraph};
use crate::ui::components::post::render_post;
use crate::ui::App;

pub fn render_feed(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1)])
        .split(f.area());

    let posts_chunk = chunks[0];
    let post_height = 6;
    app.visible_posts = (posts_chunk.height / post_height) as usize;

    let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(post_height))
        .take(app.visible_posts)
        .collect();

    let post_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(posts_chunk);

    for (i, (post, area)) in app.posts.iter()
        .skip(app.selected_index.saturating_sub(app.visible_posts - 2))
        .take(app.visible_posts)
        .zip(post_areas.iter())
        .enumerate()
    {
        //Highlighted index is the same as selected index
        let mut highlight_index = app.selected_index;
        //Until we scroll down, then it should be the second to last post
        if app.selected_index > app.visible_posts - 2 {
            if app.selected_index != app.posts.len() - 2 {
                highlight_index = app.visible_posts - 2;
                //But if we're at the bottom then it can be the last one
            } else {
                highlight_index = app.visible_posts;
            }
        }
        render_post(f, post, *area, i == highlight_index);
    }
}
