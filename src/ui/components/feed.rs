use ratatui::{
    backend::Backend, buffer::Buffer, crossterm::event::KeyCode, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Style}, text::{Span, Text}, widgets::{Block, Borders, Widget}, Frame
};
use ratatui::prelude::StatefulWidget;
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use std::collections::VecDeque;
use anyhow::Result;

use crate::client::api::{ApiError, API};

use super::post::PostState;

pub struct Feed {
    pub posts: VecDeque<FeedViewPost>,
    pub cursor: Option<String>,
    pub visible_posts: usize,
    pub selected_index: usize,
}

impl Feed {
    pub fn new() -> Self {
        Self {
            posts: VecDeque::new(),
            cursor: None,
            visible_posts: 0,
            selected_index: 0,
        }
    }
    
    fn get_highlighted_index(&self) -> usize {
        if self.selected_index > self.visible_posts - 2 {
            if self.selected_index != self.posts.len() - 2 {
                self.visible_posts - 2
            } else {
                self.visible_posts
            }
        } else {
            self.selected_index
        }
    }
    
    // Add methods to handle loading, scrolling, and updating the feed
    pub async fn load_initial_posts(&mut self, api: &mut API) -> Result<()> {
        let timeline_result = api.get_timeline(None).await;
        Ok(match timeline_result {
            Ok((posts, cursor)) => {
                self.posts.extend(posts);
                self.cursor = cursor;
            }
            Err(e) => {
                // Try to determine if this is an authentication error
                if let Some(api_error) = e.downcast_ref::<ApiError>() {
                    match api_error {
                        ApiError::SessionExpired | ApiError::NotAuthenticated => {
                            // Try to refresh and retry
                            match api.refresh_session().await {
                                Ok(_) => {
                                    // Retry getting timeline
                                    match api.get_timeline(None).await {
                                        Ok((posts, cursor)) => {
                                            self.posts.extend(posts);
                                            self.cursor = cursor;
                                        }
                                        Err(e) => {
                                            return Err(e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    return Err(e)
                                }
                            }
                        }
                        _ => {
                            return Err(e)
                        }
                    }
                } else {
                    return Err(e)
                }
            }
        })
    }

    pub async fn scroll(&mut self, api: &API) {
        // Implementation to load more posts
    }

    pub fn handle_input(&mut self, key: KeyCode) {
        // Implementation to handle user input and update selected index
    }
}

impl Widget for Feed {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area);

        let posts_chunk = chunks[0];
        let post_height = 6;
        self.visible_posts = (posts_chunk.height / post_height) as usize;

        let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(post_height))
            .take(self.visible_posts)
            .collect();

        let post_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(posts_chunk);

        for (i, (post, area)) in self.posts.iter()
            .skip(self.selected_index.saturating_sub(self.visible_posts - 2))
            .take(self.visible_posts)
            .zip(post_areas.iter())
            .enumerate()
        {
            let highlight_index = self.get_highlighted_index();
            let post_component = super::post::Post::new(post.clone());

            post_component.render(*area, buf, &mut PostState {
                selected: highlight_index == i,
                liked: false,
                reposted: false,
            });
        }
    }
}
