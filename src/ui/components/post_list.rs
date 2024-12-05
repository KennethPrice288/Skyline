// In src/ui/components/post_list.rs
use std::collections::VecDeque;
use atrium_api::app::bsky::feed::defs::PostView;

// A trait for components that manage a scrollable list of posts
pub trait PostList {
    fn get_total_height_before_scroll(&self) -> u16;
    fn get_last_visible_index(&self, area_height: u16) -> usize;
    fn ensure_post_heights(&mut self);
    fn scroll_down(&mut self);
    fn scroll_up(&mut self);
}

// Shared data structure that both Feed and Thread can use
pub struct PostListBase {
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub last_known_height: u16,
}

impl PostListBase {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            scroll_offset: 0,
            last_known_height: 0,
        }
    }

    // Helper to calculate post height - moved from Feed
    pub fn calculate_post_height(post: &PostView) -> u16 {
        let mut height = 0;
        
        // Base structure (borders)
        height += 2;  // Top and bottom borders
        
        // Fixed components
        height += 1;  // Header line
        height += 1;  // Stats line
        
        // Dynamic content height (text)
        if let Some(text) = Self::get_post_text(post) {
            height += ((text.len() as f32 / 50.0).ceil() as u16).max(1);
        }
        
        // Image section if present
        if post.data.embed.is_some() {
            height += 12;  // 10 for image area + 2 for borders
        }
        
        height
    }

    // Helper to get post text - moved from Feed
    pub fn get_post_text(post: &PostView) -> Option<String> {
        use atrium_api::types::Unknown;
        use ipld_core::ipld::Ipld;
        
        match &post.data.record {
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

    // Common scroll logic that both Feed and Thread can use
    pub fn handle_scroll_down<T>(
        &mut self,
        posts: &VecDeque<T>,
        get_height: impl Fn(&T) -> u16,
    ) {
        if self.selected_index >= posts.len() - 1 {
            return;
        }
        
        let mut y_position = 0;
        let next_index = self.selected_index + 1;
        
        for (i, post) in posts.iter().enumerate().skip(self.scroll_offset) {
            if i == next_index {
                let height = get_height(post);
                    
                if y_position >= self.last_known_height || 
                   (y_position + height) > self.last_known_height {
                    while y_position >= self.last_known_height.saturating_sub(height) {
                        if self.scroll_offset >= posts.len() - 1 {
                            break;
                        }
                        if let Some(first_post) = posts.get(self.scroll_offset) {
                            let first_height = get_height(first_post);
                            y_position -= first_height;
                            self.scroll_offset += 1;
                        }
                    }
                }
                break;
            }
            
            let height = get_height(post);
            y_position += height;
        }
        
        self.selected_index = next_index;
    }

    pub fn handle_scroll_up(&mut self) {
        if self.selected_index == 0 {
            return;
        }
        
        self.selected_index -= 1;
        
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }
}
