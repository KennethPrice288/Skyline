// In main.rs
use anyhow::Result;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use std::io;
use std::panic;

use skyline::client::api::API;
use skyline::ui::App;

use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;

pub fn setup_logging() -> std::io::Result<()> {
    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("skyline.log")?,
    )
    .expect("Failed to initialize logger");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging()?;

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
