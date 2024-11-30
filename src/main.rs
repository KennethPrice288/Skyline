use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, crossterm, Terminal};
use secrecy::SecretString;
use std::io;
use skyline::{client::api::API, ui::App};
use skyline::ui::draw;  // Changed this line
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let identifier = std::env::var("BSKY_IDENTIFIER")?;
    let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
    let mut api = API::new().await;
    
    println!("Before login: authenticated = {}", api.is_authenticated());
    api.login(identifier, password).await?;
    println!("After login: authenticated = {}", api.is_authenticated());
    
    let mut app = App::new(api);
    println!("Before loading posts: authenticated = {}", app.api.is_authenticated());
    app.load_initial_posts().await;
    println!("After loading posts: error = {:?}", app.error);
    println!("authenticated = {}", app.api.is_authenticated());

    loop {
        terminal.draw(|f| draw(f, &app))?;  // Changed this line

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('j') => {
                    app.selected_index = app.selected_index.saturating_add(1);
                }
                KeyCode::Char('k') => {
                    app.selected_index = app.selected_index.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
