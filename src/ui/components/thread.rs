// In src/ui/components/thread.rs
use std::{collections::{HashMap, VecDeque}, sync::Arc};
use atrium_api::app::bsky::feed::{
    defs::{ThreadViewPost, ThreadViewPostParentRefs, ThreadViewPostRepliesItem}, get_post_thread::OutputThreadRefs
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Widget, StatefulWidget}
};

use super::{
    images::ImageManager,
    post::Post,
    post_list::{PostList, PostListBase}
};

use anyhow::Result;

pub struct Thread {
    pub posts: VecDeque<ThreadViewPost>,
    pub rendered_posts: Vec<Post>,
    pub post_heights: HashMap<String, u16>,
    pub status_line: Option<String>,
    pub selected_uri: String,  // URI of the focused post
    image_manager: Arc<ImageManager>,
    base: PostListBase,
}

impl Thread {
    pub fn new(thread_data: OutputThreadRefs, image_manager: Arc<ImageManager>) -> Self {
        let mut thread = Self {
            posts: VecDeque::new(),
            rendered_posts: Vec::new(),
            post_heights: HashMap::new(),
            status_line: Some("".to_string()),
            selected_uri: String::new(),
            image_manager,
            base: PostListBase::new(),
        };

        thread.process_thread_data(thread_data);
        thread
    }

    fn process_thread_data(&mut self, thread_data: OutputThreadRefs) -> Result<()> {
        match thread_data {
            OutputThreadRefs::AppBskyFeedDefsThreadViewPost(post) => {
                self.selected_uri = post.post.uri.to_string();
                
                if let Some(parent) = &post.parent {
                    match parent {
                        atrium_api::types::Union::Refs(parent_refs) => {
                            self.process_parent_thread(parent_refs)?;
                        },
                        atrium_api::types::Union::Unknown(unknown_data) => {
                            return Err(anyhow::anyhow!(
                                "Unknown parent data type: {}, data: {:?}",
                                unknown_data.r#type,
                                unknown_data.data
                            ));
                        }
                    }
                }

                self.add_post(*post.clone());

                if let Some(replies) = &post.replies {
                    for reply in replies {
                        match reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                self.process_reply_thread(reply_refs)?;
                            },
                            atrium_api::types::Union::Unknown(unknown_data) => {
                                return Err(anyhow::anyhow!(
                                    "Unknown reply data type: {}, data: {:?}",
                                    unknown_data.r#type,
                                    unknown_data.data
                                ));
                            }
                        }
                    }
                }
            }
            OutputThreadRefs::AppBskyFeedDefsNotFoundPost(_) => {
                self.status_line = Some("Post not found".to_string());
            }
            OutputThreadRefs::AppBskyFeedDefsBlockedPost(_) => {
                self.status_line = Some("Post is blocked".to_string());
            }
        }
        Ok(())
    }
    
    fn process_parent_thread(&mut self, parent_refs: &ThreadViewPostParentRefs) -> Result<()> {
        match parent_refs {
            ThreadViewPostParentRefs::ThreadViewPost(post) => {
                if let Some(parent_parent) = &post.parent {
                    match parent_parent {
                        atrium_api::types::Union::Refs(parent_parent_refs) => {
                            self.process_parent_thread(parent_parent_refs)?;
                        },
                        atrium_api::types::Union::Unknown(unknown_data) => {
                            return Err(anyhow::anyhow!(
                                "Unknown parent's parent data type: {}, data: {:?}",
                                unknown_data.r#type,
                                unknown_data.data
                            ));
                        }
                    }
                }
                self.add_post(*post.clone());
            }
            ThreadViewPostParentRefs::NotFoundPost(_) => {
                // Optionally add a placeholder for not found posts
                self.status_line = Some("Parent post not found".to_string());
            }
            ThreadViewPostParentRefs::BlockedPost(_) => {
                // Optionally add a placeholder for blocked posts
                self.status_line = Some("Parent post is blocked".to_string());
            }
        }
        Ok(())
    }

    fn process_reply_thread(&mut self, reply_refs: &ThreadViewPostRepliesItem) -> Result<()> {
        match reply_refs {
            ThreadViewPostRepliesItem::ThreadViewPost(post) => {
                self.add_post(*post.clone());
                if let Some(replies) = &post.replies {
                    for reply in replies {
                        match reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                self.process_reply_thread(reply_refs)?;
                            },
                            atrium_api::types::Union::Unknown(unknown_data) => {
                                return Err(anyhow::anyhow!(
                                    "Unknown nested reply data type: {}, data: {:?}",
                                    unknown_data.r#type,
                                    unknown_data.data
                                ));
                            }
                        }
                    }
                }
            }
            ThreadViewPostRepliesItem::NotFoundPost(_) => {
                self.status_line = Some("Reply not found".to_string());
            }
            ThreadViewPostRepliesItem::BlockedPost(_) => {
                self.status_line = Some("Reply is blocked".to_string());
            }
        }
        Ok(())
    }

    

    fn add_post(&mut self, post: ThreadViewPost) {
        let uri = post.post.uri.to_string();
        self.rendered_posts.push(Post::new(post.post.clone(), self.image_manager.clone()));
        self.posts.push_back(post);
        
        if uri == self.selected_uri {
            self.base.selected_index = self.posts.len() - 1;
        }
    }
}

impl PostList for Thread {
    fn get_total_height_before_scroll(&self) -> u16 {
        self.posts
            .iter()
            .take(self.base.scroll_offset)
            .filter_map(|post| self.post_heights.get(&post.post.uri.to_string()))
            .sum()
    }

    fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.base.scroll_offset;

        for (i, post) in self.posts.iter().enumerate().skip(self.base.scroll_offset) {
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

    fn ensure_post_heights(&mut self) {
        let posts_to_calculate: Vec<_> = self.posts
            .iter()
            .filter(|post| !self.post_heights.contains_key(&post.post.uri.to_string()))
            .cloned()
            .collect();

        for post in posts_to_calculate {
            let height = PostListBase::calculate_post_height(&post.post);
            self.post_heights.insert(post.post.uri.to_string(), height);
        }
    }

    fn scroll_down(&mut self) {
        self.base.handle_scroll_down(
            &self.posts,
            |post| self.post_heights
                .get(&post.post.uri.to_string())
                .copied()
                .unwrap_or(6)
        );
    }

    fn scroll_up(&mut self) {
        self.base.handle_scroll_up();
    }
}

impl Widget for &mut Thread {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.base.last_known_height = area.height;
        self.ensure_post_heights();
        
        let mut current_y = area.y;
        
        for (i, post) in self.rendered_posts.iter_mut()
            .enumerate()
            .skip(self.base.scroll_offset) 
        {
            let post_height = self.post_heights
                .get(&post.get_uri())
                .copied()
                .unwrap_or(6);
            
            let remaining_height = area.height.saturating_sub(current_y - area.y);
            if remaining_height == 0 {
                break;
            }
            
            // Calculate indentation - no indent for selected post
            let x_offset = if post.get_uri() != self.selected_uri { 4 } else { 0 };
            
            let post_area = Rect {
                x: area.x + x_offset,
                y: current_y,
                width: area.width.saturating_sub(x_offset),
                height: remaining_height.min(post_height),
            };
            
            post.render(
                post_area,
                buf,
                &mut super::post::PostState {
                    selected: i == self.base.selected_index,
                },
            );
            
            current_y = current_y.saturating_add(post_height);
        }
    }
}
