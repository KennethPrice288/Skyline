// In src/ui/views/mod.rs
use std::sync::Arc;
use anyhow::Result;
use atrium_api::app::bsky::feed::get_post_thread::OutputThreadRefs;

use crate::client::api::API;
use crate::ui::components::{feed::Feed, images::ImageManager, thread::Thread};

pub enum View {
    Timeline(Feed),
    Thread(Thread),
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
        let params = atrium_api::app::bsky::feed::get_post_thread::Parameters {
            data: atrium_api::app::bsky::feed::get_post_thread::ParametersData {
                uri: uri.into(),
                depth: None,
                parent_height: None,
            },
            extra_data: ipld_core::ipld::Ipld::Null,
        };
    
        match api.agent.api.app.bsky.feed.get_post_thread(params).await {
            Ok(response) => {
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
