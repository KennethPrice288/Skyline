
use std::{collections::{HashMap, VecDeque}, sync::Arc};

use atrium_api::app::bsky::feed::defs::PostView;
use ratatui::{buffer::Buffer, layout::Rect, widgets::{Widget, StatefulWidget}};

use crate::client::api::{ApiError, API};
use anyhow::Result;
use super::{images::ImageManager, post_list::{PostList, PostListBase}};

pub struct Feed {
    pub posts: VecDeque<PostView>,
    pub rendered_posts: Vec<super::post::Post>,
    pub cursor: Option<String>,
    pub post_heights: HashMap<String, u16>,
    pub status_line: Option<String>,
    image_manager: Arc<ImageManager>,
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
                        self.image_manager.clone(),
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
                                self.image_manager.clone(),
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

    fn ensure_post_heights(&mut self) {
        let posts_to_calculate: Vec<_> = self.posts
            .iter()
            .filter(|post| !self.post_heights.contains_key(&post.data.uri.to_string()))
            .cloned()
            .collect();

        for post in posts_to_calculate {
            let height = PostListBase::calculate_post_height(&post);
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

}

impl Widget for &mut Feed {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // info!("Feed render area: {:?}", area);
        self.base.last_known_height = area.height;
        self.ensure_post_heights();

        let mut current_y = area.y;

        // Use the pre-created post components
        for (i, post) in self
            .rendered_posts
            .iter_mut()
            .enumerate()
            .skip(self.base.scroll_offset)
        {
            let post_height = self.post_heights.get(&post.get_uri()).copied().unwrap_or(6);

            let remaining_height = area.height.saturating_sub(current_y);
            if remaining_height == 0 {
                break;
            }

            let post_area = Rect {
                x: area.x,
                y: current_y,
                width: area.width,
                height: remaining_height.min(post_height),
            };

            // info!("Post {} area: {:?} (clipped from original height: {})",
            //   i, post_area, post_height);

            post.render(
                post_area,
                buf,
                &mut super::post::PostState {
                    selected: self.base.selected_index == i,
                },
            );

            current_y = current_y.saturating_add(post_height);
        }
    }
}
