use crate::client::api::API;
use anyhow::Result;
use ratatui::crossterm::{event::KeyCode, terminal::EnterAlternateScreen};
use secrecy::SecretString;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use super::{components::{images::ImageManager, post_list::PostList}, views::{View, ViewStack}};

use ratatui::crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::io::{self, Write};

use crate::ui::draw;

pub struct App {
    pub api: API,
    pub loading: bool,
    pub error: Option<String>,
    pub view_stack: ViewStack,
    pub status_line: String,
    pub image_manager: Arc<ImageManager>,
}

impl App {
    pub fn new(api: API) -> Self {
        let image_manager = Arc::new(ImageManager::new());
        Self {
            api,
            loading: false,
            error: None,
            view_stack: ViewStack::new(Arc::clone(&image_manager)),
            status_line: "".to_string(),
            image_manager,
        }
    }
    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        self.api.login(identifier, password).await
    }

    pub async fn load_initial_posts(&mut self) {
        self.loading = true;
        if let View::Timeline(feed) = self.view_stack.current_view() {
            feed.load_initial_posts(&mut self.api).await.unwrap();
        }
        self.loading = false;
    }

    async fn update_post_data(&mut self, uri: &str) -> Result<()> {
        if let Ok(updated_post) = self.api.get_post(uri).await {
            self.view_stack.update_post(updated_post);
        }
        Ok(())
    }

    async fn handle_like_post(&mut self) {
        let update_uri = match self.view_stack.current_view() {
            View::Timeline(feed) => {
                let selected_idx = feed.selected_index();
                if let Some(post) = feed.posts.get(selected_idx) {
                    let uri = post.data.uri.as_str();
                    if post.viewer
                    .as_ref()
                    .and_then(|v| v.data.like.as_ref())
                    .is_some() {
                        let _ = self.api.unlike_post(post).await;
                    } else {
                        let cid = &post.data.cid;
                        let _ = self.api.like_post(uri, cid).await;
                    }
                    uri.to_string()
                } else {
                    "".to_string()
                }
            },
            View::Thread(thread) => {
                let selected_idx = thread.selected_index();
                if let Some(post) = thread.posts.get(selected_idx) {
                    let uri = post.uri.as_str();
                    if post.viewer
                    .as_ref()
                    .and_then(|v| v.data.like.as_ref())
                    .is_some() {
                        let _ = self.api.unlike_post(post).await;
                    } else {
                        let cid = &post.cid;
                        let _ = self.api.like_post(uri, cid).await;
                    } 
                    uri.to_string()
                } else {
                        "".to_string()
                    }
                },
            };
    
        if !update_uri.is_empty() {
            let _ = self.update_post_data(&update_uri).await;
        }
    }

    pub async fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') => {
                match self.view_stack.current_view() {
                    View::Timeline(feed) => {
                        feed.scroll_down();
                        // Only fetch more posts if we're near the end
                        if feed.selected_index() >= feed.posts.len().saturating_sub(5) {
                            feed.scroll(&self.api).await;
                        }
                    },
                    View::Thread(thread) => {
                        thread.scroll_down();
                    },
                }
            }
            KeyCode::Char('k') => {
                match self.view_stack.current_view() {
                    View::Timeline(feed) => {
                        feed.scroll_up();
                    },
                    View::Thread(thread) => {
                        thread.scroll_up();
                    },
                }
            }
            KeyCode::Char('v') => {
                match self.view_stack.current_view() {
                    View::Timeline(feed) => {
                        if let Some(selected_post) = feed.posts.get(feed.selected_index()) {
                            let uri = selected_post.data.uri.to_string();
                            if let Err(e) = self.view_stack.push_thread_view(uri, &self.api).await {
                                self.error = Some(format!("Failed to load thread: {}", e));
                            }
                        }
                    },
                    View::Thread(thread) => {
                        if let Some(selected_post) = thread.posts.get(thread.selected_index()) {
                            let uri = selected_post.uri.to_string();
                            //Cant select same post over again
                            if uri == thread.anchor_uri {
                                return;
                            }
                            if let Err(e) = self.view_stack.push_thread_view(uri, &self.api).await {
                                self.error = Some(format!("Failed to load thread: {}", e));
                            }
                        }
                    },
                }
            }
            KeyCode::Char('l') => {
               self.handle_like_post().await;
            },
            // KeyCode::Char('r') => {
            //     match self.view_stack.current_view() {
            //         View::Timeline(feed) => {
            //             let selected_idx = feed.selected_index();
            //             if let Some(post) = feed.posts.get(selected_idx) {
            //                 if let Err(e) = self.api.repost(&post.data.uri, &post.data.cid).await {
            //                     self.error = Some(format!("Failed to repost: {}", e));
            //                 } else {
            //                     // TODO: Add method to refresh post data
            //                 }
            //             }
            //         },
            //         View::Thread(thread) => {
            //             let selected_idx = thread.selected_index();
            //             if let Some(post) = thread.posts.get(selected_idx) {
            //                 if let Err(e) = self.api.repost(&post.uri, &post.cid).await {
            //                     self.error = Some(format!("Failed to repost: {}", e));
            //                 } else {
            //                     // TODO: Add method to refresh post data
            //                 }
            //             }
            //         },
            //     }
            // },
            KeyCode::Esc => {
                self.view_stack.pop_view();
            }
            _ => {}
        }
    }

    pub async fn run(mut self) -> Result<()> {
        // Terminal initialization
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Initialize app state
        self.loading = true;
        terminal.draw(|f| draw(f, &mut self))?;

        // Handle authentication
        if let Some(_session) = self.api.agent.get_session().await {
            // Already authenticated
        } else {
            let identifier = std::env::var("BSKY_IDENTIFIER")?;
            let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
            self.login(identifier, password).await?;
        }

        // Load initial data
        self.load_initial_posts().await;
        self.loading = false;

        // Main event loop
        let result = self.event_loop(&mut terminal).await;

        // Cleanup
        self.cleanup(&mut terminal)?;

        // Return any error that occurred
        result
    }

    async fn event_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let tick_rate = Duration::from_millis(250);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| draw(f, self))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.code == KeyCode::Char('q') {
                            return Ok(());
                        }
                        self.handle_input(key.code).await;
                    }
                    Event::Mouse(_) => {}
                    Event::Resize(_, _) => {}
                    Event::FocusGained => {}
                    Event::FocusLost => {}
                    Event::Paste(_) => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                // Handle time-based updates here if needed
                last_tick = Instant::now();
            }
        }
    }

    fn cleanup<B: Backend + Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
        terminal.show_cursor()?;
        Ok(())
    }

    pub fn update_status(&mut self) {
        self.status_line = if self.loading {
            "Loading...".to_string()
        } else if let Some(err) = &self.error {
            err.to_string()
        } else {
            let (selected, total) = match self.view_stack.current_view() {
                View::Timeline(feed) => (feed.selected_index() + 1, feed.posts.len()),
                View::Thread(thread) => (thread.selected_index() + 1, thread.posts.len()),
            };
            
            format!(
                "Press q to quit, j/k to navigate, l to like/unlike, v to view  a thread and ESC to back out of one {} / {}",
                selected,
                total
            )
        };
    }
}
