// In src/ui/views/mod.rs
use std::sync::Arc;
use anyhow::Result;
use log::info;
use atrium_api::types::LimitedU16;

use crate::client::api::API;
use crate::ui::components::{feed::Feed, images::ImageManager, thread::Thread};

use super::components::post_list::PostList;

pub enum View {
    Timeline(Feed),
    Thread(Thread),
}

impl View {
    pub fn update_post(&mut self, updated_post: atrium_api::app::bsky::feed::defs::PostView) {
        let uri = updated_post.data.uri.clone();
        match self {
            View::Timeline(feed) => {
                if let Some(index) = feed.posts.iter().position(|p| p.data.uri == uri) {
                    log::info!("Updating timeline post at index {}", index);
                    feed.posts[index] = updated_post.clone();
                    if let Some(rendered) = feed.rendered_posts.get_mut(index) {
                        rendered.post = updated_post;
                    }
                }
            }
            View::Thread(thread) => {
                if let Some(index) = thread.posts.iter().position(|p| p.uri == uri) {
                    log::info!("Updating thread post at index {}", index);
                    thread.posts[index] = updated_post.data.clone();
                    if let Some(rendered) = thread.rendered_posts.get_mut(index) {
                        rendered.post = updated_post;
                    }
                }
            }
        }
    }

    pub fn get_all_post_uris(&self) -> Vec<String> {
        match self {
            View::Timeline(feed) => {
                feed.posts.iter()
                    .map(|post| post.data.uri.to_string())
                    .collect()
            },
            View::Thread(thread) => {
                thread.posts.iter()
                    .map(|post| post.uri.to_string())
                    .collect()
            }
        }
    }
    
    pub fn scroll_down(&mut self) {
        match self {
            View::Timeline(feed) => feed.scroll_down(),
            View::Thread(thread) => thread.scroll_down(),
        }
    }

    pub fn scroll_up(&mut self) {
        match self {
            View::Timeline(feed) => feed.scroll_up(),
            View::Thread(thread) => thread.scroll_up(),
        }
    }
}

pub struct ViewStack {
    views: Vec<View>,
    image_manager: Arc<ImageManager>,
}

impl ViewStack {
    pub fn new(image_manager: Arc<ImageManager>) -> Self {
        let initial_feed = Feed::new(Arc::clone(&image_manager));
        Self {
            views: vec![View::Timeline(initial_feed)],
            image_manager,
        }
    }

    pub fn current_view(&mut self) -> &mut View {
        self.views.last_mut().unwrap()
    }

    pub async fn push_thread_view(&mut self, uri: String, api: &API) -> Result<()> {
        info!("Attempting to create thread view for URI: {}", uri);
        
        let params = atrium_api::app::bsky::feed::get_post_thread::Parameters {
            data: atrium_api::app::bsky::feed::get_post_thread::ParametersData {
                uri: uri.into(),
                depth: Some(LimitedU16::MAX),
                parent_height: Some(LimitedU16::MAX),
            },
            extra_data: ipld_core::ipld::Ipld::Null,
        };
        
        match api.agent.api.app.bsky.feed.get_post_thread(params).await {
            Ok(response) => {
                // Try to see the raw response data
                info!("Raw response: {:?}", response);
                
                let thread_refs = match response.data.thread {
                    atrium_api::types::Union::Refs(refs) => refs,
                    atrium_api::types::Union::Unknown(unknown) => {
                        return Err(anyhow::anyhow!(
                            "Unknown thread data type: {}, data: {:?}", 
                            unknown.r#type, 
                            unknown.data
                        ))
                    }
                };
    
                let thread_view = Thread::new(thread_refs, Arc::clone(&self.image_manager));
                self.views.push(View::Thread(thread_view));
                Ok(())
            }
            Err(e) => Err(e.into())
        }
    }

    pub fn pop_view(&mut self) -> Option<View> {
        if self.views.len() > 1 {
            self.views.pop()
        } else {
            None // Don't pop the last view
        }
    }
}
