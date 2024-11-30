use crate::client::api::{ApiError, API};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use std::collections::VecDeque;

pub struct App {
    pub api: API,
    pub posts: VecDeque<FeedViewPost>,
    pub cursor: Option<String>,
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<String>,
}

impl App {
    pub fn new(api: API) -> Self {
        Self {
            api,
            posts: VecDeque::new(),
            cursor: None,
            selected_index: 0,
            loading: false,
            error: None,
        }
    }

    pub async fn load_initial_posts(&mut self) {
        self.loading = true;
        
        let timeline_result = self.api.get_timeline(None).await;
        match timeline_result {
            Ok((posts, cursor)) => {
                self.posts.extend(posts);
                self.cursor = cursor;
                self.error = None;
            }
            Err(e) => {
                // Try to determine if this is an authentication error
                if let Some(api_error) = e.downcast_ref::<ApiError>() {
                    match api_error {
                        ApiError::SessionExpired | ApiError::NotAuthenticated => {
                            // Try to refresh and retry
                            match self.api.refresh_session().await {
                                Ok(_) => {
                                    // Retry getting timeline
                                    match self.api.get_timeline(None).await {
                                        Ok((posts, cursor)) => {
                                            self.posts.extend(posts);
                                            self.cursor = cursor;
                                            self.error = None;
                                        }
                                        Err(e) => {
                                            self.error = Some(format!("Failed after refresh: {}", e));
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.error = Some(format!("Refresh failed: {}", e));
                                }
                            }
                        }
                        _ => {
                            self.error = Some(e.to_string());
                        }
                    }
                } else {
                    self.error = Some(e.to_string());
                }
            }
        }
        self.loading = false;
    }
    
}
