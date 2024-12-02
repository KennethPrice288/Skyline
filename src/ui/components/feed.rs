use anyhow::Result;
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use ratatui::prelude::StatefulWidget;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use std::collections::VecDeque;

use crate::client::api::{ApiError, API};

use super::post::PostState;

const POST_HEIGHT: u16 = 6;

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
                                Err(e) => return Err(e),
                            }
                        }
                        _ => return Err(e),
                    }
                } else {
                    return Err(e);
                }
            }
        })
    }

    pub async fn scroll(&mut self, api: &API) {
        match api.get_timeline(self.cursor.clone()).await {
            Ok((posts, cursor)) => {
                self.posts.extend(posts);
                self.cursor = cursor;
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }
}

impl Widget for &Feed {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area);

        let posts_chunk = chunks[0];
        let post_height = POST_HEIGHT;
        let visible_posts = (posts_chunk.height / post_height) as usize;

        let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(post_height))
            .take(visible_posts)
            .collect();

        let post_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(posts_chunk);

        //If the start index is the second to last position in visible posts or greater
        //Then the start index should be scrolled down
        let start_index = if self.selected_index >= visible_posts.saturating_sub(2) {
            self.selected_index
                .saturating_sub(visible_posts.saturating_sub(2))
        } else {
            0 // Keep at top until we need to scroll
        };

        for (i, (post, area)) in self
            .posts
            .iter()
            .skip(start_index)
            .take(visible_posts)
            .zip(post_areas.iter())
            .enumerate()
        {
            let post_component = super::post::Post::new(post.clone());

            post_component.render(
                *area,
                buf,
                &mut PostState {
                    selected: self.selected_index == (start_index + i),
                    liked: false,
                    reposted: false,
                },
            );
        }
    }
}
