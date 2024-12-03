use anyhow::Result;
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use ratatui::prelude::StatefulWidget;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};
use std::collections::{HashMap, VecDeque};

use crate::client::api::{ApiError, API};


pub struct Feed {
    pub posts: VecDeque<FeedViewPost>,
    pub cursor: Option<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub post_heights: HashMap<String, u16>,
    pub last_known_height: u16,
    pub status_line: Option<String>,
}

#[derive(Default)]
pub struct RenderStats {
    pub total_height: u16,
    pub visible_posts: usize,
    pub area_height: u16,
}

impl Feed {

    pub fn new() -> Self {
        Self {
            posts: VecDeque::new(),
            cursor: None,
            selected_index: 0,
            scroll_offset: 0,
            post_heights: HashMap::new(),
            last_known_height: 0,
            status_line: Some("".to_string()),
        }
    }

    // Helper to calculate total height of posts before scroll offset
    pub fn get_total_height_before_scroll(&self) -> u16 {
        self.posts
            .iter()
            .take(self.scroll_offset)
            .filter_map(|post| self.post_heights.get(&post.post.uri.to_string()))
            .sum()
    }

    pub fn get_render_stats(&self, area_height: u16) -> RenderStats {
        let mut total_height = 0;
        let mut visible_posts = 0;

        for post in self.posts.iter().skip(self.scroll_offset) {
            let height = self.post_heights
                .get(&post.post.uri.to_string())
                .copied()
                .unwrap_or(6);

            total_height += height;
            visible_posts += 1;

            if total_height >= area_height {
                break;
            }
        }

        RenderStats {
            total_height,
            visible_posts,
            area_height,
        }
    }

    pub fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.scroll_offset;

        for (i, post) in self.posts.iter().enumerate().skip(self.scroll_offset) {
            let height = self.post_heights
                .get(&post.post.uri.to_string())
                .copied()
                .unwrap_or(6);

            if total_height + height > area_height {
                break;
            }

            total_height += height;
            last_visible = i;
        }

        last_visible
    }

   // Calculate the internal height needed for post content and borders
   fn calculate_post_height(post: &FeedViewPost) -> u16 {
    let mut height = 0;
    
    // Block borders (top and bottom)
    height += 2;
    
    // Header line
    height += 1;
    
    // Stats line
    height += 1;
    
    // Content height (roughly one line per 50 chars)
    if let Some(text) = Feed::get_post_text(post) {
        height += ((text.len() as f32 / 50.0).ceil() as u16).max(1);
    }
    
    // Add height for images if present
    if post.post.embed.is_some() {
        height += 10; // Placeholder height for images
    }
    
    height
}

    // Helper to get post text - made static to avoid borrow issues
    fn get_post_text(post: &FeedViewPost) -> Option<String> {
        use atrium_api::types::Unknown;
        use ipld_core::ipld::Ipld;
        
        match &post.post.record {
            Unknown::Object(map) => match map.get("text") {
                Some(data_model) => match &**data_model {
                    Ipld::String(text) => Some(text.clone()),
                    _ => None,
                },
                None => None,
            },
            _ => None,
        }
    }

    // Get cached height or calculate new height
    pub fn ensure_post_heights(&mut self) {
        let posts_to_calculate: Vec<_> = self.posts
            .iter()
            .filter(|post| !self.post_heights.contains_key(&post.post.uri.to_string()))
            .cloned()
            .collect();

        for post in posts_to_calculate {
            let height = Feed::calculate_post_height(&post);
            self.post_heights.insert(post.post.uri.to_string(), height);
        }
    }

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
    
    pub fn scroll_down(&mut self) {
        if self.selected_index >= self.posts.len() - 1 {
            return;
        }
        
        // Before moving selection, verify the new selection would be visible
        let mut y_position = 0;
        let next_index = self.selected_index + 1;
        
        // Find where the next post would be positioned
        for (i, post) in self.posts.iter().enumerate().skip(self.scroll_offset) {
            if i == next_index {
                let height = self.post_heights
                    .get(&post.post.uri.to_string())
                    .copied()
                    .unwrap_or(6);
                    
                // Scroll if either:
                // 1. The post starts beyond visible area, or
                // 2. The post starts inside but would extend beyond visible area
                if y_position >= self.last_known_height || 
                   (y_position + height) > self.last_known_height {
                    // Keep scrolling until this post fits
                    while y_position >= self.last_known_height.saturating_sub(height) {
                        if self.scroll_offset >= self.posts.len() - 1 {
                            break;
                        }
                        if let Some(first_post) = self.posts.get(self.scroll_offset) {
                            let first_height = self.post_heights
                                .get(&first_post.post.uri.to_string())
                                .copied()
                                .unwrap_or(6);
                            y_position -= first_height;
                            self.scroll_offset += 1;
                        }
                    }
                }
                break;
            }
            
            let height = self.post_heights
                .get(&post.post.uri.to_string())
                .copied()
                .unwrap_or(6);
            y_position += height;
        }
        
        // Now safe to move selection
        self.selected_index = next_index;
    }

    pub fn scroll_up(&mut self) {
        if self.selected_index == 0 {
            return;
        }
        
        self.selected_index -= 1;
        
        // If we've scrolled above viewport, scroll up
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }
}

impl Widget for &mut Feed {
        fn render(self, area: Rect, buf: &mut Buffer) {
            self.last_known_height = area.height;
            self.ensure_post_heights();
            
            let mut current_y = area.y;
            let mut debug_info = Vec::new();
            
            for (i, post) in self.posts.iter().enumerate().skip(self.scroll_offset) {
                let height = self.post_heights
                    .get(&post.post.uri.to_string())
                    .copied()
                    .unwrap_or(6);
                    
                // Debug: track ALL posts from scroll_offset until after selected
                if i <= self.selected_index + 1 {
                    debug_info.push(format!("{}:{}/{}", i, current_y - area.y, height));
                }
                    
                let post_area = Rect {
                    x: area.x,
                    y: current_y,
                    width: area.width,
                    height,
                };
                
                let post_component = super::post::Post::new(post.clone());
                post_component.render(
                    post_area,
                    buf,
                    &mut super::post::PostState {
                        selected: self.selected_index == i,
                    },
                );
                
                current_y += height;
            }
        }
    }
