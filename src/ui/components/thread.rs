// In src/ui/components/thread.rs
use std::{collections::{HashMap, HashSet, VecDeque}, sync::Arc};
use atrium_api::{app::bsky::feed::{
    defs::{ThreadViewPost, ThreadViewPostParentRefs, ThreadViewPostRepliesItem}, get_post_thread::OutputThreadRefs
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
    root_uri: String,
    selected_uri: String,
    parent_uri: Option<String>,
    visible_posts: HashSet<String>,
    indent_levels: HashMap<String, u16>,
}

impl ThreadRelationships {
    fn new(root_uri: String, selected_uri: String) -> Self {
        Self {
            root_uri,
            selected_uri,
            parent_uri: None,
            visible_posts: HashSet::new(),
            indent_levels: HashMap::new(),
        }
    }

    fn is_visible(&self, uri: &str) -> bool {
        self.visible_posts.contains(uri)
    }

    fn get_indent_level(&self, uri: &str) -> u16 {
        self.indent_levels.get(uri).copied().unwrap_or(0)
    }

    fn is_root_post(&self, uri: &str) -> bool {
        uri == self.root_uri
    }

    fn is_selected_post(&self, uri: &str) -> bool {
        uri == self.selected_uri
    }

    fn update(&mut self, post_uri: &str, parent_uri: &str, should_show: bool, indent_level: u16) {
        if should_show {
            self.visible_posts.insert(post_uri.to_string());
            self.indent_levels.insert(post_uri.to_string(), indent_level);
            self.parent_uri = Some(parent_uri.to_string());
        } else {
            self.visible_posts.remove(post_uri);
            self.indent_levels.remove(post_uri);
        }
    }

}

pub struct Thread {
    pub posts: VecDeque<ThreadViewPost>,
    pub rendered_posts: Vec<Post>,
    pub post_heights: HashMap<String, u16>,
    pub status_line: Option<String>,
    pub selected_uri: String,  // URI of the focused post
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
            selected_uri: String::new(),
            image_manager,
            base: PostListBase::new(),
            cached_relationships: None,
        };

        info!("About to process thread data");
        thread.process_thread_data(thread_data);
        thread.cached_relationships = Some(thread.get_relationships().clone());
        info!("After processing, selected_uri: {}", thread.selected_uri);
        thread
    }

    pub fn selected_index(&self) -> usize {
        return self.base.selected_index;
    }

    fn get_relationships(&mut self) -> &ThreadRelationships {
        if self.cached_relationships.is_none() {
            let root_uri = self.get_root_uri_from_thread()
                .unwrap_or_else(|| self.selected_uri.clone());

            let mut relationships = ThreadRelationships::new(root_uri.clone(), self.selected_uri.clone());

            let mut current_parent_uri = root_uri.clone();

            // First, find the selected post and its parent
            let mut selected_post_parent_uri = None;
            for post in &self.posts {
                if post.post.uri == self.selected_uri {
                    if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
                        selected_post_parent_uri = Some(parent_uri.clone());
                        relationships.update(&post.post.uri, &parent_uri, true, 0); // Set the selected post to 0 indent
                    } else {
                        relationships.update(&post.post.uri, &root_uri, true, 0); // Set the root post to 0 indent
                    }
                    break;
                }
            }

            // Then, process the rest of the posts
            for post in &self.posts {
                if post.post.uri != self.selected_uri {
                    if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
                        let (should_show, indent_level) = self.should_show_reply(&post.post.uri, &parent_uri, post);
                        if should_show {
                            if let Some(selected_post_parent_uri) = &selected_post_parent_uri {
                                if parent_uri == *selected_post_parent_uri {
                                    relationships.update(&post.post.uri, &parent_uri, true, 1);
                                } else {
                                    relationships.update(&post.post.uri, &parent_uri, true, relationships.get_indent_level(&parent_uri) + 1);
                                }
                            } else {
                                relationships.update(&post.post.uri, &parent_uri, true, relationships.get_indent_level(&parent_uri) + 1);
                            }
                            current_parent_uri = post.post.uri.clone();
                        } else {
                            relationships.update(&post.post.uri, &parent_uri, false, 0);
                        }
                    }
                }
            }

            self.cached_relationships = Some(relationships);
        }

        self.cached_relationships.as_ref().unwrap()
    }

     // Invalidate the cache when needed
     fn invalidate_relationships_cache(&mut self) {
        self.cached_relationships = None;
    }

    // fn build_relationships(&self, posts: &[ThreadViewPost]) -> ThreadRelationships {
    //     // First find the root of the thread
    //     let root_uri = self.get_root_uri_from_thread()
    //         .unwrap_or_else(|| self.selected_uri.clone());

    //     let mut relationships = ThreadRelationships::new(root_uri, self.selected_uri.clone());

    //     // Process each post to find relationships
    //     for post in posts {
    //         if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
    //             let (should_show, _) = self.should_show_reply(&post.post.uri, &parent_uri, post);
    //             if should_show {
    //                 relationships.add_reply(post.post.uri.clone(), parent_uri);
    //             }
    //         }
    //     }

    //     relationships
    // }

    // Helper to get the parent URI directly from the record field
    fn get_parent_uri_from_record(post: &ThreadViewPost) -> Option<String> {
        if let Unknown::Object(record) = &post.post.record {
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

    fn process_thread_data(&mut self, thread_data: OutputThreadRefs) -> Result<()> {
        info!("Starting process_thread_data");
        match thread_data {
            OutputThreadRefs::AppBskyFeedDefsThreadViewPost(post) => {
                // Debug the entire post structure
                info!("DEBUG RAW POST:");
                info!("URI: {}", post.post.uri);
                
                // Check for top-level parent field
                if let Some(parent) = &post.parent {
                    info!("Top-level parent exists:");
                    match parent {
                        atrium_api::types::Union::Refs(parent_refs) => {
                            match parent_refs {
                                ThreadViewPostParentRefs::ThreadViewPost(parent_post) => {
                                    info!("Parent is post: {}", parent_post.post.uri);
                                }
                                _ => info!("Parent is other variant")
                            }
                        }
                        _ => info!("Parent is Unknown type")
                    }
                } else {
                    info!("No top-level parent");
                }

                // Check the post.data field
                info!("DEBUG data field:");
                info!("Parent in data: {}", post.data.parent.is_some());
                
                // If we have replies, debug first reply's structure
                if let Some(replies) = &post.replies {
                    if let Some(first_reply) = replies.first() {
                        info!("DEBUG first reply structure:");
                        match first_reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                match reply_refs {
                                    ThreadViewPostRepliesItem::ThreadViewPost(reply_post) => {
                                        info!("Reply URI: {}", reply_post.post.uri);
                                        info!("Reply has top-level parent: {}", reply_post.parent.is_some());
                                        info!("Reply has data.parent: {}", reply_post.data.parent.is_some());
                                    }
                                    _ => info!("Reply is not ThreadViewPost")
                                }
                            }
                            _ => info!("Reply is not Refs type")
                        }
                    }
                }

                // Continue with normal processing
                self.selected_uri = post.post.uri.to_string();
                info!("Set selected_uri to: {}", self.selected_uri);

                // Process parent if exists
                if let Some(parent) = &post.parent {
                    info!("Found parent, processing parent thread");
                    match parent {
                        atrium_api::types::Union::Refs(parent_refs) => {
                            self.process_parent_thread(parent_refs)?;
                        },
                        _ => info!("Parent is Unknown type")
                    }
                }

                // Add the main post
                info!("Adding main post");
                self.add_post(*post.clone());

                // Process replies
                if let Some(replies) = &post.replies {
                    info!("Found {} replies", replies.len());
                    for reply in replies {
                        match reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                info!("Processing reply");
                                self.process_reply_thread(reply_refs)?;
                            },
                            _ => info!("Reply is Unknown type")
                        }
                    }
                }

                info!("Finished processing thread data");
                Ok(())
            }
            _ => {
                info!("Thread data was not AppBskyFeedDefsThreadViewPost");
                Ok(())
            }
        }
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
                info!("REPLY RAW DATA:");
                info!("URI: {}", post.post.uri);
                info!("Has data field: true");  // We know this exists from the type
                info!("Data has post field: true");  // We know this exists from the type
                info!("Data has parent field: {}", post.data.parent.is_some());
                if let Some(parent) = &post.data.parent {
                    // info!("Parent field content type: {:?}", std::any::type_name::<_>());
                } else {
                    info!("Parent field is None");
                }
                
                // Let's also look at where this post came from
                if let Some(parent_post) = &post.parent {
                    info!("Post has direct parent field");
                    match parent_post {
                        atrium_api::types::Union::Refs(parent_refs) => {
                            info!("Direct parent is refs type");
                            match parent_refs {
                                ThreadViewPostParentRefs::ThreadViewPost(parent) => {
                                    info!("Parent URI: {}", parent.post.uri);
                                }
                                _ => info!("Parent is other variant")
                            }
                        }
                        _ => info!("Parent is unknown type")
                    }
                } else {
                    info!("No direct parent field");
                }
                
                self.add_post(*post.clone());
                
                // Debug the same post AFTER we stored it
                if let Some(stored_post) = self.posts.iter().find(|p| p.post.uri == post.post.uri) {
                    info!("Checking stored version of post");
                    if let Some(parent) = &stored_post.data.parent {
                        info!("Stored post has parent");
                    } else {
                        info!("Stored post lost its parent");
                    }
                }

                // Process replies as before
                if let Some(replies) = &post.replies {
                    info!("Reply has {} nested replies", replies.len());
                    for reply in replies {
                        match reply {
                            atrium_api::types::Union::Refs(reply_refs) => {
                                self.process_reply_thread(reply_refs)?;
                            },
                            _ => info!("Nested reply is Unknown type")
                        }
                    }
                }
            }
            _ => info!("Reply is not ThreadViewPost type")
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

        // Invalidate cache when posts change
        self.invalidate_relationships_cache();
    }

    fn get_descendant_uris(&self) -> std::collections::HashSet<String> {
        let mut descendants = std::collections::HashSet::new();
        
        // First find our selected post
        for post in &self.posts {
            if post.post.uri == self.selected_uri {
                continue;
            }
            
            // Check if this post is a descendant
            let mut current_uri = post.post.uri.as_str();
            while let Some(current_post) = self.posts.iter().find(|p| p.post.uri == current_uri) {
                if let Some(parent) = &current_post.parent {
                    if let atrium_api::types::Union::Refs(parent_refs) = parent {
                        match parent_refs {
                            ThreadViewPostParentRefs::ThreadViewPost(parent_post) => {
                                if parent_post.post.uri == self.selected_uri {
                                    descendants.insert(post.post.uri.to_string());
                                    break;
                                }
                                current_uri = &parent_post.post.uri;
                                continue;
                            }
                            _ => break
                        }
                    }
                }
                break;
            }
        }
        descendants
    }

    // Helper to get the parent URI of a post
    fn get_parent_uri(post: &ThreadViewPost) -> Option<String> {
        info!("Examining post data: {}", post.post.uri);
        // Access through post.data
        match &post.data.parent {
            Some(parent_union) => {
                info!("Found parent in post.data.parent");
                match parent_union {
                    atrium_api::types::Union::Refs(parent_refs) => {
                        info!("Parent is Refs type");
                        match parent_refs {
                            ThreadViewPostParentRefs::ThreadViewPost(parent_post) => {
                                let uri = parent_post.post.uri.to_string();
                                info!("Found parent post URI: {}", uri);
                                Some(uri)
                            },
                            other => {
                                info!("Parent ref is different variant");
                                None
                            }
                        }
                    },
                    atrium_api::types::Union::Unknown(unknown) => {
                        info!("Parent is Unknown type: {}", unknown.r#type);
                        None
                    }
                }
            },
            None => {
                info!("No parent in post.data.parent");
                None
            }
        }
    }
     // Get both parent post replies and selected post replies
    //  fn get_thread_relationships(&self) -> (String, HashSet<String>, HashSet<String>) {
    //     let mut parent_replies = HashSet::new();
    //     let mut selected_replies = HashSet::new();
    //     let root_uri = self.get_root_uri_from_thread().unwrap_or_default();

    //     info!("Starting relationship detection...");
        
    //     // Process all posts to find relationships
    //     for post in &self.posts {
    //         if let Some(parent_uri) = Self::get_parent_uri_from_record(post) {
    //             let (should_show, category) = self.should_show_reply(&post.post.uri, &parent_uri, post);
                
    //             if should_show {
    //                 info!("Adding post {} as {}", post.post.uri, category);
    //                 match category {
    //                     "root_reply" => { parent_replies.insert(post.post.uri.to_string()); }
    //                     "selected_reply" => { selected_replies.insert(post.post.uri.to_string()); }
    //                     // Add new categories here as needed
    //                     _ => {}
    //                 }
    //             }
    //         }
    //     }

    //     (root_uri, parent_replies, selected_replies)
    // }

    fn get_root_uri_from_record(post: &ThreadViewPost) -> Option<String> {
        if let Unknown::Object(record) = &post.post.record {
            if let Some(reply) = record.get("reply") {
                if let ipld_core::ipld::Ipld::Map(reply_obj) = &**reply {
                    // Look for the "root" field instead of "parent"
                    if let Some(root) = reply_obj.get("root") {
                        if let ipld_core::ipld::Ipld::Map(root_obj) = root {
                            if let Some(uri) = root_obj.get("uri") {
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

    // Get the root URI of the thread, either from the selected post's record or the selected post itself
    fn get_root_uri_from_thread(&self) -> Option<String> {
        self.posts.iter()
            .find(|p| p.post.uri == self.selected_uri)
            .and_then(|selected_post| {
                Self::get_root_uri_from_record(selected_post)
                    .or_else(|| Some(selected_post.post.uri.clone()))
            })
    }

    // This function determines whether a reply should be shown in the current thread view
    fn should_show_reply(&self, reply_uri: &str, parent_uri: &str, reply_post: &ThreadViewPost) -> (bool, u16) {
        let is_root_reply = parent_uri == self.get_root_uri_from_thread().unwrap_or_default();
        let is_reply_to_selected = parent_uri == self.selected_uri;
        let is_from_selected_author = if let Some(selected_post) = self.posts.iter().find(|p| p.post.uri == self.selected_uri) {
            reply_post.post.author.did == selected_post.post.author.did
        } else {
            false
        };

        // Return both whether to show the reply and the indent level
        match () {
            _ if is_reply_to_selected => (true, 1),
            _ if is_root_reply => (true, 0),
            // Add more conditions here as needed:
            // _ if is_from_selected_author => (true, 2),
            _ => (false, 0),
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
        
        // Use cached relationships
        let relationships = &self.cached_relationships;
        
        let mut current_y = area.y;
        
        for (i, post) in self.rendered_posts.iter_mut()
            .enumerate()
            .skip(self.base.scroll_offset.clone())
        {
            let post_height = self.post_heights
                .get(&post.get_uri())
                .copied()
                .unwrap_or(6);
            
            let remaining_height = area.height.saturating_sub(current_y - area.y);
            if remaining_height == 0 {
                break;
            }
            
            let x_offset = if relationships.is_none() {
                info!("couldn't get relationships for render");
                10
            } else {
                relationships.as_ref().unwrap().get_indent_level(&post.get_uri())
            };
            
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
