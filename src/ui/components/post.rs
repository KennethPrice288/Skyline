use ipld_core::ipld::Ipld;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use atrium_api::types::Unknown;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::{FixedOffset, Local};

fn log_debug(msg: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug.log") 
    {
        let _ = writeln!(file, "{}", msg);
    }
}

pub fn render_post(f: &mut Frame, post: &FeedViewPost, area: Rect, selected: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(if selected { Color::Yellow } else { Color::White }));
    let author = &post.post.author;
    
    // Debug the record content
    let post_text = match &post.post.record {
        Unknown::Object(map) => {
            match map.get("text") {
                Some(data_model) => {
                    match &**data_model {
                        Ipld::String(text) => text.clone(),
                        Ipld::Null => "(Null content)".to_string(),
                        other => format!("(Unexpected format: {:?})", other)
                    }
                }
                None => "(No text content)".to_string()
            }
        },
        Unknown::Null => "(Null content)".to_string(),
        Unknown::Other(data) => format!("Other: {:?}", data),
    };

    let author_handle = author.handle.to_string();
    let author_display_name = author.display_name.clone().unwrap_or(author_handle.clone());

    let mut time_posted = &post.post.indexed_at;
    let fixed_offset: &chrono::DateTime<FixedOffset> = time_posted.as_ref();
    let local_time: chrono::DateTime<Local> = fixed_offset.with_timezone(&Local);

    let formatted_time = local_time.format("%Y-%m-%d %-I:%M %p").to_string();

    let header = Line::from(vec![
        Span::styled(
            &author_display_name,
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" @"),
        Span::raw(&author_handle),
        Span::raw(" posted at "),
        Span::raw(formatted_time),
    ]);
    let content = Line::from(post_text);

    let stats = {
        let like_text = format!("{}", post.post.like_count.unwrap_or(999));
        let repost_text = format!("{}", post.post.repost_count.unwrap_or(999));
    
        Line::from(vec![
            Span::styled("♥ ", Style::default().fg(Color::Red)),
            Span::styled(like_text, Style::default().fg(Color::White)),
            Span::styled(" ⟲ ", Style::default().fg(Color::Blue)),
            Span::styled(repost_text, Style::default().fg(Color::White)),
        ])
    };
    

    let block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(if selected { Color::Yellow } else { Color::White }));

    let inner_area = block.inner(area);

    let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Min(2),  // Main content 
        Constraint::Length(1), // Stats line
    ])
    .split(inner_area);

    // Render main content
    f.render_widget(
    Paragraph::new(vec![
        header,
        Line::raw(""),
        content,
    ])
    .alignment(Alignment::Left)
    .wrap(ratatui::widgets::Wrap { trim: true }),
    chunks[0],
    );

    // Render stats
    f.render_widget(
    Paragraph::new(stats)
        .alignment(Alignment::Left),
    chunks[1],
    );

    f.render_widget(block, area);



}
