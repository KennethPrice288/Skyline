// In main.rs
use std::panic;
use std::io;
use ratatui::crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use ratatui::crossterm::execute;
use anyhow::Result;

use skyline::client::api::API;
use skyline::ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook for cleanup
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Clean up terminal
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        // Call the original panic handler
        original_hook(panic_info);
    }));

    // Create and run app
    let api = API::new().await;
    let app = App::new(api);

    if let Err(err) = app.run().await {
        // Clean up terminal before handling the error
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}
