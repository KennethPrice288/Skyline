// In src/ui/components/thread.rs
use std::{collections::{HashMap, HashSet, VecDeque}, sync::Arc};
use atrium_api::{app::bsky::feed::{
    defs::{PostViewData, ThreadViewPostParentRefs, ThreadViewPostRepliesItem}, get_post_thread::OutputThreadRefs
}, types::Unknown};
use log::info;
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

#[derive(Debug, Clone)]
pub struct ThreadRelationships {
    visible_posts: HashSet<String>,
    indent_levels: HashMap<String, u16>,
    post_to_parent: HashMap<String, String>,
}

impl ThreadRelationships {
    fn new() -> Self {
        Self {
            visible_posts: HashSet::new(),
            indent_levels: HashMap::new(),
            post_to_parent: HashMap::new(),
        }
    }

    fn get_indent_level(&self, uri: &str) -> u16 {
        self.indent_levels.get(uri).copied().unwrap_or(0)
    }

    fn mark_visible(&mut self, post_uri: &str, parent_uri: Option<&str>, indent_level: u16) {
        self.visible_posts.insert(post_uri.to_string());
        self.indent_levels.insert(post_uri.to_string(), indent_level);
        if let Some(parent) = parent_uri {
            self.post_to_parent.insert(post_uri.to_string(), parent.to_string());
        }
    }

    fn is_visible(&self, uri: &str) -> bool {
        self.visible_posts.contains(uri)
    }
}
pub struct Thread {
    // pub posts: VecDeque<ThreadViewPost>,
    pub posts: VecDeque<PostViewData>,
    pub rendered_posts: Vec<Post>,
    pub post_heights: HashMap<String, u16>,
    pub status_line: Option<String>,
    pub anchor_uri: String,  // URI of the focused post
    image_manager: Arc<ImageManager>,
    base: PostListBase,
    cached_relationships: Option<ThreadRelationships>,
}


impl Thread {
    pub fn new(thread_data: OutputThreadRefs, image_manager: Arc<ImageManager>) -> Self {
        info!("Creating new thread");
        let mut thread = Self {
            posts: VecDeque::new(),
            rendered_posts: Vec::new(),
            post_heights: HashMap::new(),
            status_line: Some("".to_string()),
            anchor_uri: String::new(),
            image_manager,
            base: PostListBase::new(),
            cached_relationships: None,
        };

        info!("About to process thread data");
        let _ = thread.process_thread_data(thread_data);
        thread.update_relationships();
        info!("After processing, anchor_uri: {}", thread.anchor_uri);
        thread
    }

    fn update_relationships(&mut self) {
        let mut relationships = ThreadRelationships::new();
        
        // First pass: build parent relationships and mark anchor post
        let mut parent_chain = Vec::new();
        let mut current_uri = self.anchor_uri.clone();
        
        // Build chain from anchor post to root
        while let Some(post) = self.find_post_by_uri(&current_uri) {
            parent_chain.push(post.uri.clone());
            if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
                current_uri = parent_uri;
            } else {
                break;
            }
        }

        // Mark posts in parent chain as visible with increasing indentation
        for (depth, uri) in parent_chain.iter().rev().enumerate() {
            let indent = (parent_chain.len() - depth - 1) as u16;
            if let Some(post) = self.find_post_by_uri(uri) {
                let parent_uri = Self::get_parent_uri_from_record(post);
                relationships.mark_visible(uri, parent_uri.as_deref(), indent);
            }
        }

        // Second pass: handle direct replies to anchor post
        if let Some(anchor_post) = self.find_post_by_uri(&self.anchor_uri) {
            let anchor_indent = relationships.get_indent_level(&self.anchor_uri);
            
            for post in &self.posts {
                if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
                    if parent_uri == anchor_post.uri {
                        // Only show direct replies to anchor post
                        relationships.mark_visible(&post.uri, Some(&parent_uri), anchor_indent + 1);
                    }
                }
            }
        }

        self.cached_relationships = Some(relationships);
    }

    fn find_post_by_uri(&self, uri: &str) -> Option<&PostViewData> {
        self.posts.iter().find(|p| p.uri == uri)
    }

    fn process_thread_data(&mut self, thread_data: OutputThreadRefs) -> Result<()> {
        match thread_data {
            OutputThreadRefs::AppBskyFeedDefsThreadViewPost(post) => {
                self.anchor_uri = post.post.uri.to_string();
                
                // Process parent chain first
                if let Some(parent) = &post.parent {
                    match parent {
                        atrium_api::types::Union::Refs(parent_refs) => {
                            self.process_parent_thread(parent_refs)?;
                        },
                        _ => {}
                    }
                }

                // Add anchor post
                self.add_post(post.post.data.clone());

                // Process direct replies only
                if let Some(replies) = &post.replies {
                    for reply in replies {
                        match reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                match reply_refs {
                                    ThreadViewPostRepliesItem::ThreadViewPost(reply_post) => {
                                        // Only add the direct reply, not its replies
                                        self.add_post(reply_post.post.data.clone());
                                    },
                                    _ => {}
                                }
                            },
                            _ => {}
                        }
                    }
                }

                Ok(())
            }
            _ => Ok(())
        }
    }

    pub fn selected_index(&self) -> usize {
        return self.base.selected_index;
    }

    // Helper to get the parent URI directly from the record field
    fn get_parent_uri_from_record(post: &PostViewData) -> Option<String> {
        if let Unknown::Object(record) = &post.record {
            // Try to get the "reply" field
            if let Some(reply) = record.get("reply") {
                let reply_ipld = &**reply;
                if let ipld_core::ipld::Ipld::Map(reply_map) = reply_ipld {
                    if let Some(parent) = reply_map.get("parent") {
                        if let ipld_core::ipld::Ipld::Map(parent_map) = parent {
                            if let Some(uri) = parent_map.get("uri") {
                                if let ipld_core::ipld::Ipld::String(uri_str) = uri {
                                    return Some(uri_str.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
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
                self.add_post(post.post.data.clone());
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
    
    fn add_post(&mut self, post: PostViewData) {
        let uri = post.uri.to_string();
        self.rendered_posts.push(Post::new(post.clone().into(), self.image_manager.clone()));
        self.posts.push_back(post);
        
        if uri == self.anchor_uri {
            self.base.selected_index = self.posts.len() - 1;
        }
    }
}

impl PostList for Thread {
    fn get_total_height_before_scroll(&self) -> u16 {
        self.posts
            .iter()
            .take(self.base.scroll_offset)
            .filter_map(|post| self.post_heights.get(&post.uri.to_string()))
            .sum()
    }

    fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.base.scroll_offset;

        for (i, post) in self.posts.iter().enumerate().skip(self.base.scroll_offset) {
            let height = self.post_heights
                .get(&post.uri.to_string())
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
            .filter(|post| !self.post_heights.contains_key(&post.uri.to_string()))
            .cloned()
            .collect();

        for post in posts_to_calculate {
            let height = PostListBase::calculate_post_height(&post.clone().into());
            self.post_heights.insert(post.uri.to_string(), height);
        }
    }

    fn scroll_down(&mut self) {
        self.base.handle_scroll_down(
            &self.posts,
            |post| self.post_heights
                .get(&post.uri.to_string())
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
        
        let relationships = self.cached_relationships.as_ref().unwrap();
        let mut current_y = area.y;
        
        for (i, post) in self.rendered_posts.iter_mut()
            .enumerate()
            .skip(self.base.scroll_offset)
            .filter(|(_, post)| relationships.is_visible(&post.get_uri()))
        {
            let post_height = self.post_heights
                .get(&post.get_uri())
                .copied()
                .unwrap_or(6);
            
            let remaining_height = area.height.saturating_sub(current_y - area.y);
            if remaining_height == 0 {
                break;
            }
            
            let indent_level = relationships.get_indent_level(&post.get_uri());
            let x_offset = indent_level * 2; // 2 spaces per indent level
            
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
