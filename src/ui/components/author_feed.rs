use std::{collections::{HashMap, VecDeque}, sync::Arc};
use atrium_api::{app::bsky::feed::defs::{PostView, PostViewData}, types::Object};
use ratatui::{buffer::Buffer, layout::Rect, widgets::{StatefulWidget, Widget}};
use super::{author_profile::AuthorProfile, images::ImageManager, post::Post, post_list::{PostList, PostListBase}};

pub struct AuthorFeed {
    pub profile: AuthorProfile,
    pub posts: VecDeque<PostView>,
    pub rendered_posts: Vec<Post>,
    pub post_heights: HashMap<String, u16>,
    pub base: PostListBase,
    image_manager: Arc<ImageManager>,
}

impl AuthorFeed {
    pub fn new(profile: AuthorProfile, feed_data: Vec<Object<PostViewData>>, image_manager: Arc<ImageManager>) -> Self {
        log::info!("Creating new author feed");
        let mut author_feed = Self {
            profile: profile,
            posts: VecDeque::new(),
            rendered_posts: Vec::new(),
            post_heights: HashMap::new(),
            base: PostListBase::new(),
            image_manager: image_manager,
        };

        author_feed.process_feed_data(feed_data);

        return author_feed;
    }

    fn process_feed_data(&mut self, feed_data: Vec<Object<PostViewData>>) {
        for post in feed_data {
            self.add_post(post.data);
        }
    }

    fn add_post(&mut self, post: PostViewData) {
        self.rendered_posts.push(Post::new(post.clone().into(), self.image_manager.clone()));
        self.posts.push_back(post.into());
    }

}

impl PostList for AuthorFeed {
    fn get_total_height_before_scroll(&self) -> u16 {
        let profile_height = if self.base.scroll_offset == 0 {
            self.profile.height()
        } else {
            0
        };
    
        profile_height + self.posts
            .iter()
            .take(self.base.scroll_offset)
            .filter_map(|post| self.post_heights.get(&post.uri.to_string()))
            .sum::<u16>()
    }

    fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.base.scroll_offset;
    
        // If we're showing the profile, account for its height
        if self.base.scroll_offset == 0 {
            total_height += self.profile.height();
            if total_height > area_height {
                return 0;
            }
        }
    
        // Then check posts
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
            .filter(|post| !self.post_heights.contains_key(&post.uri.to_string()))
            .cloned()
            .collect();
    
        for post in posts_to_calculate {
            let has_images = super::post::Post::extract_images_from_post(&post.clone().into()).is_some();
            let height = PostListBase::calculate_post_height(&post.clone().into(), area.width);
            log::info!("Calculated height {} for post {}, has_images: {}", height, post.uri, has_images);
            self.post_heights.insert(post.uri.to_string(), height);
        }
    }
    
    fn scroll_down(&mut self) {
        if self.base.selected_index >= self.posts.len() - 1 {
            return;
        }

        let mut y_position = if self.base.scroll_offset == 0 { 
            self.profile.height() 
        } else { 
            0 
        };
        let next_index = self.base.selected_index + 1;

        for (i, post) in self.posts.iter().enumerate().skip(self.base.scroll_offset) {
            if i == next_index {
                let height = self.post_heights
                    .get(&post.data.uri.to_string())
                    .copied()
                    .unwrap_or(6);
                    
                if y_position >= self.base.last_known_height || 
                   (y_position + height) > self.base.last_known_height {
                    while y_position >= self.base.last_known_height.saturating_sub(height) {
                        if self.base.scroll_offset >= self.posts.len() - 1 {
                            break;
                        }
                        if let Some(first_post) = self.posts.get(self.base.scroll_offset) {
                            let first_height = self.post_heights
                                .get(&first_post.data.uri.to_string())
                                .copied()
                                .unwrap_or(6);
                            y_position -= first_height;
                            self.base.scroll_offset += 1;
                        }
                    }
                }
                break;
            }
            let height = self.post_heights
                .get(&post.data.uri.to_string())
                .copied()
                .unwrap_or(6);
            y_position += height;
        }
        
        self.base.selected_index = next_index;
    }
    
    fn scroll_up(&mut self) {
        // If we're at the first post and scrolled down, go back to profile
        if self.base.selected_index == 1 && self.base.scroll_offset > 0 {
            self.base.selected_index = 0;
            self.base.scroll_offset = 0;
            return;
        }

        // Otherwise use the common scroll up logic
        self.base.handle_scroll_up();
    }
    
    fn needs_more_content(&self) -> bool {
        // Account for profile in the index calculation
        let effective_index = if self.base.selected_index == 0 {
            0
        } else {
            self.base.selected_index - 1
        };

        effective_index > self.posts.len().saturating_sub(5)
    }

    fn selected_index(&self) -> usize {
        self.base.selected_index
    }

    fn get_post(&self, index: usize) -> Option<PostViewData> {
        self.posts.get(index).map(|post| post.data.clone())
    }
}


impl Widget for &mut AuthorFeed {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Similar to Feed's render, but handle profile at top if scroll_offset is 0
        let mut current_y = area.y;
        self.base.last_known_height = area.height;
        self.ensure_post_heights(area);

        if self.base.scroll_offset == 0 {
            let profile_area = Rect {
                x: area.x,
                y: current_y,
                width: area.width,
                height: self.profile.height(),
            };
            
            (&self.profile).render(profile_area, buf);
            current_y += self.profile.height();
        }

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
