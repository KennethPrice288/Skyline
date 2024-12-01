use crate::client::api::{ApiError, API};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use ratatui::crossterm::event::KeyCode;
use secrecy::SecretString;
use std::collections::VecDeque;
use anyhow::Result;

use super::components::feed::Feed;

pub struct App {
    pub api: API,
    pub loading: bool,
    pub error: Option<String>,
    pub feed: Feed,
}

impl App {
    pub fn new(api: API) -> Self {
        Self {
            api,
            loading: false,
            error: None,
            feed: Feed::new(),
        }
    }
    
    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        self.api.login(identifier, password).await
    }

    pub async fn load_initial_posts(&mut self) {
        self.loading = true;
        self.feed.load_initial_posts(&mut self.api).await.unwrap();
        self.loading = false;
    }

    pub async fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') => {
                if self.feed.selected_index < self.feed.posts.len() - 1 {
                    self.feed.selected_index += 1;
                } else {
                    self.feed.scroll(&self.api).await;
                }
            },
            KeyCode::Char('k') => {
                if self.feed.selected_index > 0 {
                    self.feed.selected_index -= 1;
                }
            },
            _ => {}
        }
    }
    
}
