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
    let api = API::new().await;
    let mut app = App::new(api);
    app.loading = true;
    terminal.draw(|f| draw(f, &mut app))?;
    if let Some(_session) = app.api.agent.get_session().await {
    } else {
        let identifier = std::env::var("BSKY_IDENTIFIER")?;
        let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
        app.login(identifier, password).await?;
    }
    app.load_initial_posts().await;
    app.loading = false;

    loop {
        terminal.draw(|f| draw(f, &mut app))?;
    
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                break;
            }
            app.handle_input(key.code);
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
