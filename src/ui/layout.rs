use crate::ui::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Paragraph, StatefulWidget},
    Frame,
};

use super::{components::post_composer::PostComposerState, views::View};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = if app.composing {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Original post
                Constraint::Min(10),    // Post composer
                Constraint::Length(1)   // Status line
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(f.area())
    };

    match app.view_stack.current_view() {
        View::Thread(thread) if app.composing => {
            // Render the anchor post at the top
            if let Some(_anchor_post) = thread.posts.iter()
                .find(|p| p.uri == thread.anchor_uri) 
            {
                let rendered_post = thread.rendered_posts.iter_mut()
                    .find(|p| p.get_uri() == thread.anchor_uri)
                    .unwrap();
                
                let post_area = Rect {
                    x: chunks[0].x,
                    y: chunks[0].y,
                    width: chunks[0].width,
                    height: chunks[0].height,
                };

                // Render the anchor post
                rendered_post.render(
                    post_area,
                    f.buffer_mut(),
                    &mut super::components::post::PostState {
                        selected: false,
                    },
                );
            }

            // Render the composer below with indentation
            if let Some(composer) = &app.post_composer {
                let composer_area = Rect {
                    x: chunks[1].x + 2, // Add 2 spaces for indentation
                    y: chunks[1].y,
                    width: chunks[1].width - 2,
                    height: chunks[1].height,
                };
                
                f.render_stateful_widget(
                    composer,
                    composer_area,
                    &mut PostComposerState { is_active: true }
                );
            }
        },
        _ if app.composing => {
            // For non-thread views, just show the composer
            if let Some(composer) = &app.post_composer {
                f.render_stateful_widget(
                    composer,
                    chunks[0],
                    &mut PostComposerState { is_active: true }
                );
            }
        },
        _ => {
            // Normal view rendering
            match app.view_stack.current_view() {
                View::Timeline(feed) => {
                    f.render_widget(feed, chunks[0])
                },
                View::Thread(thread) => {
                    f.render_widget(thread, chunks[0])
                },
                View::AuthorFeed(author_feed) => {
                    f.render_widget(author_feed, chunks[0])
                },
                View::Notifications(notification_view) => {
                    f.render_widget(notification_view, chunks[0])
                },
            }
        }
    }

    // Always render status line at bottom
    f.render_widget(Paragraph::new(app.status_line.clone()), chunks[chunks.len() - 1]);
}
