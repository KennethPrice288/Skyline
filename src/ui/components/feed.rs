
use std::{collections::{HashMap, VecDeque}, sync::Arc};

use atrium_api::app::bsky::feed::defs::{PostView, PostViewData};
use ratatui::{buffer::Buffer, layout::Rect, widgets::{Block, Borders, StatefulWidget, Widget}};

use crate::{client::api::API, ui};
use anyhow::Result;
use super::{images::ImageManager, post::types::PostContext, post_list::{PostList, PostListBase}};

pub struct Feed {
    pub posts: VecDeque<PostView>,
    pub rendered_posts: Vec<super::post::Post>,
    pub cursor: Option<String>,
    pub post_heights: HashMap<String, u16>,
    pub status_line: Option<String>,
    pub image_manager: Arc<ImageManager>,
    base: PostListBase,
}

impl Feed {
    pub fn new(image_manager: Arc<ImageManager>) -> Self {
        Self {
            posts: VecDeque::new(),
            rendered_posts: Vec::new(),
            cursor: None,
            post_heights: HashMap::new(),
            status_line: Some("".to_string()),
            image_manager,
            base: PostListBase::new(),
        }
    }

    // Use delegated getters/setters for base fields
    pub fn selected_index(&self) -> usize {
        self.base.selected_index
    }

    pub fn post_heights(&self) -> &HashMap<String, u16> {
        &self.post_heights
    }


    pub async fn load_initial_posts(&mut self, api: &mut API) -> Result<()> {
        let timeline_result = api.get_timeline(None).await;
        Ok(match timeline_result {
            Ok((posts, cursor)) => {
                for feed_post in posts {
                    self.rendered_posts.push(super::post::Post::new(
                        feed_post.post.clone(),
                        PostContext {
                            image_manager: self.image_manager.clone(),
                            indent_level: 0,
                        }
                    ));
                    // Extract the PostView from FeedViewPost
                    self.posts.push_back(feed_post.post.clone());
                }
                self.cursor = cursor;
            }
            Err(e) => {
                return Err(e);
            }
        })
    }

    pub async fn scroll(&mut self, api: &API) {
                match api.get_timeline(self.cursor.clone()).await {
                    Ok((feed_posts, cursor)) => {
                        for feed_post in feed_posts {
                            self.rendered_posts.push(super::post::Post::new(
                                feed_post.post.clone(),
                                PostContext {
                                    image_manager: self.image_manager.clone(),
                                    indent_level: 0,
                                },
                            ));
                            self.posts.push_back(feed_post.post.clone());
                        }
                        self.cursor = cursor;
                    }
                    Err(e) => {
                        println!("{:?}", e);
                    }
                }
            }
    
            pub async fn reload_feed(&mut self, api: &mut API) -> Result<()> {
                // Store the URI of the currently selected post if we have one
                let current_uri = self.posts
                    .get(self.base.selected_index)
                    .map(|post| post.data.uri.clone());
        
                if let Some(anchor_uri) = current_uri {
                    // Clear existing posts but remember our position
                    let selected_index = self.base.selected_index;
                    self.posts.clear();
                    self.rendered_posts.clear();
                    
                    // Get the timeline centered around our current post
                    let params = atrium_api::app::bsky::feed::get_timeline::ParametersData {
                        algorithm: None,
                        // We want posts before our current position
                        cursor: None, // We'll need to implement a way to get the cursor for a specific post
                        limit: Some(atrium_api::types::LimitedNonZeroU8::MAX),
                    };
        
                    match api.agent.api.app.bsky.feed.get_timeline(params.into()).await {
                        Ok(response) => {
                            // Find the index of our anchor post in the new response
                            let anchor_index = response.feed.iter()
                                .position(|post| post.post.data.uri == anchor_uri);
        
                            if let Some(_index) = anchor_index {
                                // Add all posts to our feed
                                for feed_post in response.feed.clone() {
                                    self.rendered_posts.push(super::post::Post::new(
                                        feed_post.post.clone(),
                                        PostContext {
                                            image_manager: self.image_manager.clone(),
                                            indent_level: 0,
                                        },
                                    ));
                                    self.posts.push_back(feed_post.post.clone());
                                }
        
                                // Restore our selected position
                                self.base.selected_index = selected_index;
                                self.cursor = response.cursor.clone();
        
                                // Pre-fetch the next page if we're close to the end
                                if self.needs_more_content() {
                                    let _ = self.scroll(api).await;
                                }
                            } else {
                                // If we couldn't find our anchor post, fall back to load_initial_posts
                                self.load_initial_posts(api).await?;
                            }
                        }
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    // If we don't have a current post, just do a fresh load
                    self.load_initial_posts(api).await?;
                }
        
                Ok(())
            }

}

impl PostList for Feed {
    fn get_total_height_before_scroll(&self) -> u16 {
        self.posts
            .iter()
            .take(self.base.scroll_offset)
            .filter_map(|post| self.post_heights.get(&post.data.uri.to_string()))
            .sum()
    }

    fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.base.scroll_offset;

        for (i, post) in self.posts.iter().enumerate().skip(self.base.scroll_offset) {
            let height = self.post_heights
                .get(&post.data.uri.to_string())
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

    fn ensure_post_heights(&mut self, area: Rect) {
        let posts_to_calculate: Vec<_> = self.posts
            .iter()
            .filter(|post| !self.post_heights.contains_key(&post.data.uri.to_string()))
            .cloned()
            .collect();

        for post in posts_to_calculate {
            let height = PostListBase::calculate_post_height(&post, area.width);
            self.post_heights.insert(post.data.uri.to_string(), height);
        }
    }

    fn scroll_down(&mut self) {
        self.base.handle_scroll_down(
            &self.posts,
            |post| self.post_heights
                .get(&post.data.uri.to_string())
                .copied()
                .unwrap_or(6)
        );
    }

    fn scroll_up(&mut self) {
        self.base.handle_scroll_up();
    }

    fn needs_more_content(&self) -> bool {
        self.selected_index() > self.posts.len().saturating_sub(5)
    }

    fn selected_index(&self) -> usize {
        self.base.selected_index
    }

    fn get_post(&self, index: usize) -> Option<PostViewData> {
        self.posts.get(index).map(|post| post.data.clone())
    }

}

impl Widget for &mut Feed {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
        .borders(Borders::ALL)
        .title("🌃 Timeline");
        let inner_area = block.inner(area);
        // info!("Feed render area: {:?}", area);
        self.base.last_known_height = inner_area.height;
        self.ensure_post_heights(inner_area);

        let mut current_y = inner_area.y;
        block.render(area, buf);
        // Use the pre-created post components
        for (i, post) in self
            .rendered_posts
            .iter_mut()
            .enumerate()
            .skip(self.base.scroll_offset)
        {
            let post_height = self.post_heights.get(post.get_uri()).copied().unwrap_or(6);

            let remaining_height = inner_area.height.saturating_sub(current_y);
            if remaining_height == 0 {
                break;
            }

            let post_area = Rect {
                x: inner_area.x,
                y: current_y,
                width: inner_area.width,
                height: remaining_height.min(post_height),
            };

            // info!("Post {} area: {:?} (clipped from original height: {})",
            //   i, post_area, post_height);

            post.render(
                post_area,
                buf,
                &mut ui::components::post::types::PostState {
                    selected: self.base.selected_index == i,
                },
            );

            current_y = current_y.saturating_add(post_height);
        }
    }
}
