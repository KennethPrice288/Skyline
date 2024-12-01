use crate::client::api::{ApiError, API};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use ratatui::crossterm::{event::KeyCode, terminal::EnterAlternateScreen};
use secrecy::SecretString;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use anyhow::Result;

use super::components::feed::Feed;

use ratatui::{Terminal, backend::Backend};
use ratatui::crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen},
    execute,
};
use std::io::{self, Stdout, Write};
use tokio::signal;

use crate::ui::draw;

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
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
        )?;
        terminal.show_cursor()?;
        Ok(())
    }
    
}
