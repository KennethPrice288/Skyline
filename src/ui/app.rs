use crate::client::api::API;
use anyhow::Result;
use atrium_api::{app::bsky::feed::defs::PostView, types::string::AtIdentifier};
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
    notification_check_interval: Duration,
    last_notification_check: Instant,
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
            notification_check_interval: Duration::from_secs(60),
            last_notification_check: Instant::now(),
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
        if let Some(post) = self.view_stack.current_view().get_selected_post() {
            let uri = post.uri.as_str();
            if post.viewer
                .as_ref()
                .and_then(|v| v.data.like.as_ref())
                .is_some() {
                let _ = self.api.unlike_post(&post).await;
            } else {
                let cid = &post.cid;
                let _ = self.api.like_post(uri, cid).await;
            }
            
            self.spawn_get_post_task(200, uri.to_string()).await;
        }
    }

    async fn handle_repost(&mut self) {
        if let Some(post) = self.view_stack.current_view().get_selected_post() {
            let uri = post.uri.as_str();
            if post.viewer
                .as_ref()
                .and_then(|v| v.data.repost.as_ref())
                .is_some() {
                let _ = self.api.unrepost(&post).await;
            } else {
                let cid = &post.cid;
                let _ = self.api.repost(uri, cid).await;
            }
            
            self.spawn_get_post_task(200, uri.to_string()).await;
        } else {
            log::info!("couldnt get selected post for repost");
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

    async fn check_notifications(&mut self) {
        if self.last_notification_check.elapsed() >= self.notification_check_interval {
            if let View::Notifications(notifications) = self.view_stack.current_view() {
                notifications.load_notifications(&mut self.api).await.ok();
            }
            self.last_notification_check = Instant::now();
        }
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
            },
            KeyCode::Char('v') => {
                if let Some(post) = self.view_stack.current_view().get_selected_post() {
                    let uri = post.uri.to_string();
                    if self.view_stack.current_view().can_view_thread(&uri) {
                        if let Err(e) = self.view_stack.push_thread_view(uri, &self.api).await {
                            self.error = Some(format!("Failed to load thread: {}", e));
                        }
                    }
                }
            },
            KeyCode::Char('l') => {
               self.handle_like_post().await;
            },
            KeyCode::Char('r') => {
                self.handle_repost().await;
            },
            KeyCode::Char('a') => {
                if let View::Notifications(notifications) = self.view_stack.current_view() {
                    let selected_author_did = &notifications.get_notification().author.did;
                    let actor = AtIdentifier::Did(selected_author_did.clone());
                    match self.view_stack.push_author_feed_view(actor, &self.api).await {
                        Ok(_) => {},
                        Err(e) => {
                            log::info!("Error pushing author feed view: {:?}", e);
                            self.error = Some(format!("Failed to load author feed: {}", e));
                        }
                    }
                } else if let Some(post) = self.view_stack.current_view().get_selected_post() {
                    let selected_author_did = post.author.did.clone();
                    
                    // Check if we're already viewing this author's feed
                    let is_same_author = match self.view_stack.current_view() {
                        View::AuthorFeed(author_feed) => {
                            // Get the DID from the current author feed's profile
                            author_feed.profile.profile.did == selected_author_did
                        },
                        _ => false
                    };
            
                    if !is_same_author {
                        let actor = AtIdentifier::Did(selected_author_did);
                        match self.view_stack.push_author_feed_view(actor, &self.api).await {
                            Ok(_) => {},
                            Err(e) => {
                                log::info!("Error pushing author feed view: {:?}", e);
                                self.error = Some(format!("Failed to load author feed: {}", e));
                            }
                        }
                    }
                }
            },
            KeyCode::Char('n') => {
                // Push notifications view and load initial data
                self.view_stack.push_notifications_view();
                if let View::Notifications(notifications) = self.view_stack.current_view() {
                    self.loading = true;
                    notifications.load_notifications(&mut self.api).await.ok();
                    self.loading = false;
                    // Mark notifications as seen
                    // self.api.mark_notifications_seen().await.ok();
                }
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
                self.check_notifications().await;
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
                View::AuthorFeed(author_feed) => {(author_feed.selected_index() + 1, author_feed.posts.len())},
                View::Notifications(notification_view) => {(notification_view.selected_index() + 1, notification_view.notifications.len())},
            };
            
            format!(
                "Press q to quit, j/k to navigate, l to like/unlike, v to view  a thread, a to view a profile, and ESC to back out of one {} / {}",
                selected,
                total
            )
        };
    }
}
