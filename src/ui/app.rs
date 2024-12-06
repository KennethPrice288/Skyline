use crate::client::api::API;
use anyhow::Result;
use atrium_api::app::bsky::feed::defs::PostView;
use ratatui::crossterm::{event::KeyCode, terminal::EnterAlternateScreen};
use secrecy::SecretString;
use tokio::sync::mpsc;
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
    post_update_sender: mpsc::Sender<PostView>,
    post_update_receiver: mpsc::Receiver<PostView>,
}

impl App {
    pub fn new(api: API) -> Self {
        let image_manager = Arc::new(ImageManager::new());
        let (sender, receiver) = mpsc::channel(10);
        Self {
            api,
            loading: false,
            error: None,
            view_stack: ViewStack::new(Arc::clone(&image_manager)),
            status_line: "".to_string(),
            image_manager,
            post_update_sender: sender,
            post_update_receiver: receiver,
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

    async fn spawn_get_post_task(&self, delay: u64, update_uri: String) {
        let api = self.api.clone();
                let sender = self.post_update_sender.clone();
                
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    if let Ok(updated_post) = api.get_post(&update_uri).await {
                        sender.send(updated_post).await.ok();
                    }
                });
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
            self.spawn_get_post_task(200, update_uri).await;
        }
    }

    async fn handle_repost(&mut self) {
        let update_uri = match self.view_stack.current_view() {
            View::Timeline(feed) => {
                let selected_idx = feed.selected_index();
                if let Some(post) = feed.posts.get(selected_idx) {
                    let uri = post.data.uri.as_str();
                    if post.viewer
                    .as_ref()
                    .and_then(|v| v.data.repost.as_ref())
                    .is_some() {
                        let _ = self.api.unrepost(post).await;
                    } else {
                        let cid = &post.cid;
                        let _ = self.api.repost(uri, cid).await;
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
                    .and_then(|v| v.data.repost.as_ref())
                    .is_some() {
                        let _ = self.api.unrepost(post).await;
                    } else {
                        let cid = &post.cid;
                        let _ = self.api.repost(uri, cid).await;
                    } 
                    uri.to_string()
                } else {
                        "".to_string()
                    }
                },
            };

            if !update_uri.is_empty() {
                self.spawn_get_post_task(200, update_uri).await;
            }
    }

    pub async fn refresh_current_view(&mut self) -> Result<()> {
        let uris = self.view_stack.current_view().get_all_post_uris();
        log::info!("Refreshing view with {} total URIs", uris.len());
        
        // Create a vector to hold our futures
        let mut fetch_futures = Vec::new();
        
        // Create futures for each chunk
        for chunk in uris.chunks(25) {
            let chunk_vec = chunk.to_vec();
            let api = &self.api;
            
            // Create future for this chunk
            let future = async move {
                let params = atrium_api::app::bsky::feed::get_posts::ParametersData {
                    uris: chunk_vec,
                }.into();
                api.agent.api.app.bsky.feed.get_posts(params).await
            };
            
            fetch_futures.push(future);
        }
        
        // Execute all futures concurrently
        let results = futures::future::join_all(fetch_futures).await;
        
        // Process results
        for result in results {
            match result {
                Ok(response) => {
                    log::info!("Received {} posts from chunk", response.data.posts.len());
                    for post in response.data.posts {
                        self.view_stack.current_view().update_post(post);
                    }
                }
                Err(e) => {
                    log::error!("Chunk error: {:?}", e);
                    return Err(e.into());
                }
            }
        }
        
        Ok(())
    }

    pub async fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') => {
                self.view_stack.current_view().scroll_down();
                // Check if we need to load more content
                if let View::Timeline(feed) = self.view_stack.current_view() {
                    if feed.needs_more_content() {
                        self.loading = true;
                        feed.scroll(&self.api).await;
                        self.loading = false;
                    }
                }
            }
            KeyCode::Char('k') => {
                self.view_stack.current_view().scroll_up();
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
            KeyCode::Char('r') => {
                self.handle_repost().await;
            },
            KeyCode::Esc => {
                self.view_stack.pop_view();
                match self.refresh_current_view().await {
                    Ok(_) => log::info!("Successfully refreshed view"),
                    Err(e) => {
                        log::error!("Failed to refresh view: {:?}", e);
                        self.error = Some(format!("Failed to refresh view: {}", e));
                    }
                }
            },
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
            // Check for post updates
            while let Ok(updated_post) = self.post_update_receiver.try_recv() {
                self.view_stack.current_view().update_post(updated_post);
            }

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
