// In src/ui/views/mod.rs
use std::sync::Arc;
use anyhow::Result;
use atrium_api::app::bsky::feed::defs::PostViewData;
use atrium_api::types::string::AtIdentifier;
use atrium_api::types::LimitedU16;

use crate::client::api::API;
use crate::ui::components::author_profile::AuthorProfile;
use crate::ui::components::post::types::PostContext;
use crate::ui::components::post::Post;
use crate::ui::components::{feed::Feed, images::ImageManager, thread::Thread};

use super::components::author_feed::AuthorFeed;
use super::components::notifications::NotificationView;
use super::components::post_list::PostList;

pub enum View {
    Timeline(Feed),
    Thread(Thread),
    AuthorFeed(AuthorFeed),
    Notifications(NotificationView),
}

impl View {
    pub fn update_post(&mut self, updated_post: atrium_api::app::bsky::feed::defs::PostView) {
        let uri = updated_post.data.uri.clone();
        match self {
            View::Timeline(feed) => {
                if let Some(index) = feed.posts.iter().position(|p| p.data.uri == uri) {
                    log::info!("Updating timeline post at index {}", index);
                    feed.posts[index] = updated_post.clone();
                    // Recreate the rendered post with existing context
                    if let Some(rendered) = feed.rendered_posts.get_mut(index) {
                        feed.rendered_posts[index] = Post::new(
                            updated_post,
                            PostContext {
                                image_manager: feed.image_manager.clone(),
                                indent_level: 0,  // Timeline posts have no indent
                            }
                        );
                    }
                }
            }
            View::Thread(thread) => {
                if let Some(index) = thread.posts.iter().position(|p| p.uri == uri) {
                    log::info!("Updating thread post at index {}", index);
                    thread.posts[index] = updated_post.data.clone();
                    // Get indent level from relationships
                    let indent_level = thread.cached_relationships
                        .as_ref()
                        .map(|rels| rels.get_indent_level(&uri))
                        .unwrap_or(0);
                    
                    thread.rendered_posts[index] = Post::new(
                        updated_post,
                        PostContext {
                            image_manager: thread.image_manager.clone(),
                            indent_level,
                        }
                    );
                }
            }
            View::AuthorFeed(author_feed) => {
                if let Some(index) = author_feed.posts.iter().position(|p| p.data.uri == uri) {
                    log::info!("Updating author_feed post at index {}", index);
                    author_feed.posts[index] = updated_post.clone();
                    author_feed.rendered_posts[index] = Post::new(
                        updated_post,
                        PostContext {
                            image_manager: author_feed.image_manager.clone(),
                            indent_level: 0,  // Author feed posts have no indent
                        }
                    );
                }
            },
            View::Notifications(_notification_view) => {},
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
            View::AuthorFeed(author_feed) => {
                author_feed.posts.iter()
                .map(|post| post.data.uri.to_string())
                .collect()
            },
            View::Notifications(_notification_view) => {Vec::new()},
        }
    }
    
    pub fn scroll_down(&mut self) {
        match self {
            View::Timeline(feed) => feed.scroll_down(),
            View::Thread(thread) => thread.scroll_down(),
            View::AuthorFeed(author_feed) => author_feed.scroll_down(),
            View::Notifications(notification_view) => notification_view.scroll_down(),
        }
    }

    pub fn scroll_up(&mut self) {
        match self {
            View::Timeline(feed) => feed.scroll_up(),
            View::Thread(thread) => thread.scroll_up(),
            View::AuthorFeed(author_feed) => author_feed.scroll_up(),
            View::Notifications(notification_view) => notification_view.scroll_up(),
        }
    }

    pub fn get_selected_post(&self) -> Option<PostViewData> {
        match self {
            View::Timeline(feed) => feed.get_selected_post(),
            View::Thread(thread) => thread.get_selected_post(),
            View::AuthorFeed(author_feed) => author_feed.get_selected_post(),
            View::Notifications(_notification_view) => {None},
        }
    }

    pub fn can_view_thread(&self, uri: &str) -> bool {
        match self {
            View::Thread(thread) => uri != thread.anchor_uri,
            _ => true
        }
    }

    pub fn remove_post(&mut self, uri: &str) {
        match self {
            View::Timeline(feed) => {
                if let Some(index) = feed.posts.iter().position(|p| p.data.uri == uri) {
                    feed.posts.remove(index);
                    feed.rendered_posts.remove(index);
                }
            }
            View::Thread(thread) => {
                if let Some(index) = thread.posts.iter().position(|p| p.uri == uri) {
                    thread.posts.remove(index);
                    thread.rendered_posts.remove(index);
                    thread.update_relationships();
                }
            }
            View::AuthorFeed(author_feed) => {
                if let Some(index) = author_feed.posts.iter().position(|p| p.data.uri == uri) {
                    author_feed.posts.remove(index);
                    author_feed.rendered_posts.remove(index);
                }
            }
            View::Notifications(_) => {},
        }
    }
}

pub struct ViewStack {
    pub views: Vec<View>,
    pub image_manager: Arc<ImageManager>,
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
        log::info!("Attempting to create thread view for URI: {}", uri);
        
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

    pub async fn push_author_feed_view(&mut self, actor: AtIdentifier, api: &API) -> Result<()> {
        log::info!("Attempting to create author feed view from AtIdentifier: {:?}", actor);
        let get_author_feed_params = atrium_api::app::bsky::feed::get_author_feed::Parameters {
            data: atrium_api::app::bsky::feed::get_author_feed::ParametersData{
                actor: actor.clone(),
                cursor: None,
                filter: None, // TODO: Examine this field better
                include_pins: None,
                limit: None,
            },
            extra_data: ipld_core::ipld::Ipld::Null,
        };

        match api.agent.api.app.bsky.feed.get_author_feed(get_author_feed_params).await {
            Ok(response) => {
                let author_feed_data = response.feed.iter().map(|p| p.post.clone()).collect();
                let author_profile_data = api.agent.api.app.bsky.actor.get_profile(
                    atrium_api::app::bsky::actor::get_profile::ParametersData {
                        actor
                    }.into()
                ).await?;
                let author_profile = AuthorProfile::new(author_profile_data, self.image_manager.clone());
                let author_feed_view = AuthorFeed::new(author_profile, author_feed_data, self.image_manager.clone());
                self.views.push(View::AuthorFeed(author_feed_view));
            }
            Err(e) => {return Err(e.into())}
        }
        Ok(())
    }
    

    pub fn pop_view(&mut self) -> Option<View> {
        if self.views.len() > 1 {
            self.views.pop()
        } else {
            None // Don't pop the last view
        }
    }
}
