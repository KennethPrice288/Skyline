use crate::client::api::{ApiError, API};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use ratatui::crossterm::event::KeyCode;
use secrecy::SecretString;
use std::collections::VecDeque;
use anyhow::Result;

use super::components::feed::Feed;

pub struct App {
    pub api: API,
    pub posts: VecDeque<FeedViewPost>,
    pub cursor: Option<String>,
    pub visible_posts: usize,
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub feed: Feed,
}

impl App {
    pub fn new(api: API) -> Self {
        Self {
            api,
            posts: VecDeque::new(),
            cursor: None,
            visible_posts: 0,
            selected_index: 0,
            loading: false,
            error: None,
        }
    }
    
    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        self.api.login(identifier, password).await
    }

    pub async fn load_initial_posts(&mut self) {
        self.loading = true;
        
        self.loading = false;
    }

    pub async fn scroll(&mut self) {
        match self.api.get_timeline(self.cursor.clone()).await {
            Ok((posts, cursor)) => {
                self.posts.extend(posts);
                self.cursor = cursor;
            } 
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }

    pub async fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') => {
                if self.selected_index < self.posts.len() - 1 {
                    self.selected_index += 1;
                } else {
                    self.scroll().await;
                }
            },
            KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            },
            _ => {}
        }
    }
    
}
