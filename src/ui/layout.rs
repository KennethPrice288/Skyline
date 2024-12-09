use crate::ui::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, StatefulWidget},
    Frame,
};

use super::{components::{command_input::CommandInputState, post_composer::PostComposerState}, views::View};

pub fn draw(f: &mut Frame, app: &mut App) {
    if !app.authenticated {
        // Show login view
        if let Some(login_view) = &app.login_view {
            let chunks = if app.command_mode {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Max(5),
                        Constraint::Length(1),
                    ])
                    .split(f.area())
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Length(1),
                    ])
                    .split(f.area())
            };

            f.render_widget(login_view, chunks[0]);
            
            // Use existing command mode logic
            if app.command_mode {
                let command_area = Block::default()
                    .borders(Borders::ALL)
                    .inner(chunks[1]);
                
                f.render_stateful_widget(
                    &app.command_input,
                    command_area,
                    &mut CommandInputState { is_active: true }
                );

                f.render_widget(Paragraph::new(app.status_line.clone()), chunks[2]);
            } else {
                f.render_widget(Paragraph::new(app.status_line.clone()), chunks[1]);
            }
        }
        return;
    }

    let chunks = if app.command_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),      // Main content (chunks[0])
                Constraint::Length(3),   // Command input (chunks[1])
                Constraint::Length(1),   // Status line (chunks[2])
            ])
            .split(f.area())
    } else if app.composing {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),
                Constraint::Min(10),
                Constraint::Length(1)
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(f.area())
    };

    // Main content rendering
    match app.view_stack.current_view() {
        View::Thread(thread) if app.composing => {
            // Your existing thread composing logic
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

                rendered_post.render(
                    post_area,
                    f.buffer_mut(),
                    &mut super::components::post::PostState {
                        selected: false,
                    },
                );
            }

            if let Some(composer) = &app.post_composer {
                let composer_area = Rect {
                    x: chunks[1].x + 2,
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
            if let Some(composer) = &app.post_composer {
                f.render_stateful_widget(
                    composer,
                    chunks[0],
                    &mut PostComposerState { is_active: true }
                );
            }
        },
        _ => {
            match app.view_stack.current_view() {
                View::Timeline(feed) => f.render_widget(feed, chunks[0]),
                View::Thread(thread) => f.render_widget(thread, chunks[0]),
                View::AuthorFeed(author_feed) => f.render_widget(author_feed, chunks[0]),
                View::Notifications(notification_view) => f.render_widget(notification_view, chunks[0]),
            }
        }
    }

    // Command input and status line rendering
    if app.command_mode {
        // Render debug borders around command input chunk
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Command Input Area");
        f.render_widget(block, chunks[1]);

        // Now render actual content inside the chunks
        let command_area = Block::default()
            .borders(Borders::NONE)
            .inner(chunks[1]);
        
        f.render_stateful_widget(
            &app.command_input,
            command_area,
            &mut CommandInputState { is_active: true }
        );

        let status_area = Block::default()
            .borders(Borders::NONE)
            .inner(chunks[2]);
        
        f.render_widget(
            Paragraph::new(app.status_line.clone()),
            status_area
        );
    } else {
        f.render_widget(Paragraph::new(app.status_line.clone()), chunks[chunks.len() - 1]);
    }
}
