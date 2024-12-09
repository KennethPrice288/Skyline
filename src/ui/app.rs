use crate::client::{api::API, update::{UpdateEvent, UpdateManager}};
use anyhow::Result;
use atrium_api::{app::bsky::feed::defs::PostView, types::string::{AtIdentifier, Handle}};
use ratatui::crossterm::{event::{KeyCode, KeyEvent, KeyModifiers}, terminal::EnterAlternateScreen};
use secrecy::SecretString;
use tokio::sync::mpsc;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use super::{components::{command_input::CommandInput, images::ImageManager, post_composer::PostComposer, post_list::PostList}, views::{View, ViewStack}};

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
    update_manager: UpdateManager,
    pub post_composer: Option<PostComposer>,
    pub composing: bool,
    pub command_input: CommandInput,
    pub command_mode: bool,
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
            notification_check_interval: Duration::from_secs(120),
            last_notification_check: Instant::now(),
            update_manager: UpdateManager::new(),
            post_composer: None,
            composing: false,
            command_input: CommandInput::new(),
            command_mode: false,
        }
    }
    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        self.api.login(identifier, password).await
    }

    pub async fn load_initial_posts(&mut self) {
        self.loading = true;
        self.update_status();
        if let View::Timeline(feed) = self.view_stack.current_view() {
            feed.load_initial_posts(&mut self.api).await.unwrap();
        }
        self.loading = false;
        self.update_status();
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

    async fn handle_get_profile(&mut self, handle: AtIdentifier) {
        let _ = self.view_stack.push_author_feed_view(handle, &self.api).await;
    }
    
    pub async fn refresh_current_view(&mut self) -> Result<()> {
        self.loading = true;
        
        match self.view_stack.current_view() {
            View::Timeline(feed) => {
                feed.reload_feed(&mut self.api).await?;
            }
            View::Thread(thread) => {
                let params = atrium_api::app::bsky::feed::get_post_thread::Parameters {
                    data: atrium_api::app::bsky::feed::get_post_thread::ParametersData {
                        uri: thread.anchor_uri.clone().into(),
                        depth: Some(atrium_api::types::LimitedU16::MAX),
                        parent_height: Some(atrium_api::types::LimitedU16::MAX),
                    },
                    extra_data: ipld_core::ipld::Ipld::Null,
                };
                
                if let Ok(response) = self.api.agent.api.app.bsky.feed.get_post_thread(params).await {
                    if let atrium_api::types::Union::Refs(thread_refs) = response.data.thread {
                        thread.posts.clear();
                        thread.rendered_posts.clear();
                        let _ = thread.process_thread_data(thread_refs);
                    }
                }
            }
            View::AuthorFeed(author_feed) => {
                let actor = AtIdentifier::Did(author_feed.profile.profile.did.clone());
                let params = atrium_api::app::bsky::feed::get_author_feed::Parameters {
                    data: atrium_api::app::bsky::feed::get_author_feed::ParametersData {
                        actor: actor.clone(),
                        cursor: None,
                        filter: None,
                        include_pins: None,
                        limit: None,
                    },
                    extra_data: ipld_core::ipld::Ipld::Null,
                };
    
                if let Ok(response) = self.api.agent.api.app.bsky.feed.get_author_feed(params).await {
                    author_feed.posts.clear();
                    author_feed.rendered_posts.clear();
                    for post in &response.feed {
                        author_feed.add_post(post.post.data.clone());
                    }
                }
            }
            View::Notifications(notifications) => {
                notifications.load_notifications(&mut self.api).await?;
            }
        }
    
        self.loading = false;
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

    async fn handle_follow(&mut self) {
        let did = match self.view_stack.current_view() {
            // When viewing notifications
            View::Notifications(notifications) => {
                let notification = notifications.get_notification();
                Some(notification.author.did.clone())
            },
            // When viewing regular posts (timeline, thread, author feed)
            _ => {
                self.view_stack.current_view()
                    .get_selected_post()
                    .map(|post| post.author.did.clone())
            }
        };
    
        if let Some(did) = did {
            // Get profile to check current follow status
            let params = atrium_api::app::bsky::actor::get_profile::ParametersData {
                actor: atrium_api::types::string::AtIdentifier::Did(did.clone())
            }.into();
            
            match self.api.agent.api.app.bsky.actor.get_profile(params).await {
                Ok(profile) => {
                    let is_following = profile.viewer
                        .as_ref()
                        .and_then(|v| v.following.as_ref())
                        .is_some();
    
                    if is_following {
                        let _ = self.api.unfollow_actor(&did).await;
                    } else {
                        let _ = self.api.follow_actor(did).await;
                    }
    
                    // Refresh the current view to show updated follow status
                    if let Err(e) = self.refresh_current_view().await {
                        self.error = Some(format!("Failed to refresh view: {}", e));
                    }
                }
                Err(e) => {
                    self.error = Some(format!("Failed to get profile: {}", e));
                }
            }
        }
    }
    

    pub async fn handle_input(&mut self, key: KeyEvent) {
        match (self.command_mode, self.composing) {
            (true, _) => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.command_mode = false;
                    self.command_input.clear();
                },
                (KeyCode::Enter, _) => {
                    if let Some(command) = self.command_input.submit_command() {
                        self.command_mode = false;
                        if let Err(e) = self.handle_command(&command.to_lowercase()).await {
                            self.error = Some(format!("Command error: {}", e));
                        }
                    }
                },
                (KeyCode::Tab, _) => {
                    self.command_input.handle_tab();
                },
                (KeyCode::Char(c), mods) => {
                    if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT {
                        self.command_input.insert_char(c);
                    }
                },
                (KeyCode::Backspace, _) => self.command_input.delete_char(),
                (KeyCode::Left, _) => self.command_input.move_cursor_left(),
                (KeyCode::Right, _) => self.command_input.move_cursor_right(),
                (KeyCode::Up, _) => self.command_input.history_up(),
                (KeyCode::Down, _) => self.command_input.history_down(),
                _ => {}
            },
    
            // Then compose mode
            (false, true) => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.composing = false;
                    self.post_composer = None;
                },
                (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                    if let Some(composer) = &self.post_composer {
                        let content = composer.get_content().to_string();
                        let reply_to = composer.reply_to.clone();
                        
                        match self.api.create_post(content, reply_to).await {
                            Ok(()) => {
                                self.status_line = "Post created successfully".to_string();
                                self.composing = false;
                                self.post_composer = None;
                                
                                // Refresh view based on context
                                match self.view_stack.current_view() {
                                    View::Timeline(feed) => {
                                        feed.load_initial_posts(&mut self.api).await.ok();
                                    },
                                    View::Thread(thread) => {
                                        let anchor_uri = thread.anchor_uri.clone();
                                        self.view_stack.push_thread_view(anchor_uri, &self.api).await.ok();
                                    },
                                    _ => {}
                                }
                            },
                            Err(e) => {
                                self.error = Some(format!("Failed to create post: {}", e));
                            }
                        }
                    }
                },
                (KeyCode::Char(c), mods) => {
                    if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT {
                        if let Some(composer) = &mut self.post_composer {
                            composer.insert_char(c);
                        }
                    }
                },
                (KeyCode::Backspace, _) => {
                    if let Some(composer) = &mut self.post_composer {
                        composer.delete_char();
                    }
                },
                (KeyCode::Left, _) => {
                    if let Some(composer) = &mut self.post_composer {
                        composer.move_cursor_left();
                    }
                },
                (KeyCode::Right, _) => {
                    if let Some(composer) = &mut self.post_composer {
                        composer.move_cursor_right();
                    }
                },
                _ => {}
            },
    
            // Finally visual mode
            (false, false) => match (key.code, key.modifiers) {
                // Enter command mode
                (KeyCode::Char(':'), KeyModifiers::NONE) => {
                    self.command_mode = true;
                },
                
                (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    self.view_stack.current_view().scroll_down();
                    if let View::Timeline(feed) = self.view_stack.current_view() {
                        if feed.needs_more_content() {
                            self.loading = true;
                            feed.scroll(&self.api).await;
                            self.loading = false;
                        }
                    }
                },
                (KeyCode::Char('k'), KeyModifiers::NONE) => self.view_stack.current_view().scroll_up(),
                (KeyCode::Char('l'), KeyModifiers::NONE) => self.handle_like_post().await,
                (KeyCode::Char('r'), KeyModifiers::NONE) => self.handle_repost().await,
                (KeyCode::Char('f'), KeyModifiers::NONE) => self.handle_follow().await,
                (KeyCode::Char('v'), KeyModifiers::NONE) => {
                    if let Some(post) = self.view_stack.current_view().get_selected_post() {
                        let uri = post.uri.to_string();
                        if self.view_stack.current_view().can_view_thread(&uri) {
                            if let Err(e) = self.view_stack.push_thread_view(uri, &self.api).await {
                                self.error = Some(format!("Failed to load thread: {}", e));
                            }
                        }
                    }
                },
                (KeyCode::Char('V'), KeyModifiers::SHIFT) => {
                    if let Some(post) = self.view_stack.current_view().get_selected_post() {
                        if let Some(quoted_post) = super::components::post::Post::extract_quoted_post_data(&post.into()) {
                            let quoted_uri = quoted_post.uri.to_string();
                            if self.view_stack.current_view().can_view_thread(&quoted_uri) {
                                if let Err(e) = self.view_stack.push_thread_view(quoted_uri, &self.api).await {
                                    self.error = Some(format!("Failed to load quoted thread: {}", e));
                                }
                            }
                        }
                    }
                },
                (KeyCode::Char('n'), KeyModifiers::NONE) => {
                    let currently_notifs_view = if let View::Notifications(_) = self.view_stack.current_view() {
                        true
                    } else {
                        false
                    };
                    if !currently_notifs_view {self.view_stack.push_notifications_view();}
                    if let View::Notifications(notifications) = self.view_stack.current_view() {
                        self.loading = true;
                        let _ = notifications.load_notifications(&mut self.api).await;
                        self.loading = false;
                    }
                },
                (KeyCode::Char('a'), KeyModifiers::NONE) => {
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
                        
                        let is_same_author = match self.view_stack.current_view() {
                            View::AuthorFeed(author_feed) => {
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
                (KeyCode::Char('A'), KeyModifiers::SHIFT) => {
                    if let Some(session) = self.api.agent.get_session().await {
                        // Get the logged-in user's DID
                        let did = &session.did;
                        let actor = AtIdentifier::Did(did.clone());
                        
                        match self.view_stack.push_author_feed_view(actor, &self.api).await {
                            Ok(_) => {},
                            Err(e) => {
                                log::info!("Error pushing logged-in user feed view: {:?}", e);
                                self.error = Some(format!("Failed to load your profile: {}", e));
                            }
                        }
                    }
                },
                (KeyCode::Esc, _) => {
                    self.view_stack.pop_view();
                }
                _ => {}
            }
        }
    
        self.update_status();
    }
    
    //Helper function to handle command parsing and execution
    async fn handle_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }
    
        match parts[0] {
            "reply" => {
                if let Some(post) = self.view_stack.current_view().get_selected_post() {
                    let uri = post.uri.to_string();
                    if self.view_stack.current_view().can_view_thread(&uri) {
                        self.view_stack.push_thread_view(uri, &self.api).await?;
                    }
                    
                    self.post_composer = Some(PostComposer::new(Some(post.uri.to_string())));
                    self.composing = true;
                }
            },
            "post" => {
                self.post_composer = Some(PostComposer::new(None));
                self.composing = true;
            },
            "refresh" => {
                self.refresh_current_view().await?;
            },
            "notifications" => {
                self.view_stack.push_notifications_view();
                if let View::Notifications(notifications) = self.view_stack.current_view() {
                    self.loading = true;
                    notifications.load_notifications(&mut self.api).await?;
                    self.loading = false;
                }
            },
            "timeline" => {
                while self.view_stack.views.len() > 1 {
                    self.view_stack.pop_view();
                }
            },
            "follow" => {
                self.handle_follow().await;
            },
            "like" => {
                self.handle_like_post().await;
            },
            "repost" => {
                self.handle_repost().await;
            },
            "profile" => {
                //if we have an arg, handle argument to go to specific profile
                if parts.len() > 1 {
                    let actor = AtIdentifier::Handle(
                        Handle::new(
                            parts[1].to_string()
                        ).unwrap());
                    self.handle_get_profile(actor).await;
                } 
                // otherwise go to profile belonging to highlighted post
                else {
                    if let Some(post) = self.view_stack.current_view().get_selected_post() {
                        let actor = &post.author.did;
                        self.handle_get_profile(AtIdentifier::Did(actor.clone())).await;
                    } else {
                        if let View::Notifications(notif_view) =  self.view_stack.current_view() {
                            let actor = &notif_view.get_notification().author.did;
                            self.handle_get_profile(AtIdentifier::Did(actor.clone())).await;
                        }
                    }
                }
            }
            "delete" => {
                if let Some(post) = self.view_stack.current_view().get_selected_post() {
                    // Only allow deletion if the post author's DID matches the current user's DID
                    if let Some(session) = self.api.agent.get_session().await {
                        if post.author.did == session.did {
                            match self.api.delete_post(&post.uri).await {
                                Ok(_) => {
                                    self.status_line = "Post deleted successfully".to_string();
                                    // Refresh the current view to reflect the deletion
                                    self.refresh_current_view().await.ok();
                                }
                                Err(e) => {
                                    self.error = Some(format!("Failed to delete post: {}", e));
                                }
                            }
                        } else {
                            self.status_line = "You can only delete your own posts".to_string();
                        }
                    }
                    let _ = self.refresh_current_view().await;
                }
            }
            _ => {
                self.status_line = format!("Unknown command: {}", command);
            }
        }
        Ok(())
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

        // Start update manager after authentication
        if let Some(session) = self.api.agent.get_session().await {
            self.update_manager.start(session.access_jwt.clone()).await?;
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
                        if key.code == KeyCode::Char('q') && !self.command_mode && !self.composing {
                            return Ok(());
                        }
                        self.handle_input(key).await;
                    }
                    Event::Mouse(_) => {}
                    Event::Resize(_, _) => {}
                    Event::FocusGained => {}
                    Event::FocusLost => {}
                    Event::Paste(_) => {}
                }
            }

            // Handle real-time updates
            while let Some(event) = self.update_manager.try_recv() {
                match event {
                    UpdateEvent::Notification { uri } => {
                        if let View::Notifications(notifications) = self.view_stack.current_view() {
                            notifications.handle_new_notification(uri, &self.api).await?;
                        }
                    }
                    UpdateEvent::ConnectionStatus(_status) => {
                        // Handle connection status...
                    }
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
                "🌆 Press q to quit, j/k to navigate, l to like/unlike, v to view a thread, a to view a profile, and ESC to back out of one {} / {}",
                selected,
                total
            )
        };
    }
}
