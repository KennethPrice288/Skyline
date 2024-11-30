use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use atrium_api::types::Unknown;

pub fn render_post(f: &mut Frame, post: &FeedViewPost, area: Rect, selected: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(if selected { Color::Yellow } else { Color::White }));

    let author = &post.post.author;
    
    // Debug the record content
    let post_text = match &post.post.record {
        Unknown::Object(map) => {
            format!("Keys: {:?}\nValues: {:?}", 
                map.keys().collect::<Vec<_>>(),
                map.values().collect::<Vec<_>>()
            )
        },
        Unknown::Null => "(Null content)".to_string(),
        Unknown::Other(data) => format!("Other: {:?}", data),
    };

    let author_handle = author.handle.to_string();
    let author_display_name = author.display_name.clone().unwrap_or(author_handle.clone());
    let header = Line::from(vec![
        Span::styled(
            &author_display_name,
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" @"),
        Span::raw(&author_handle),
    ]);

    let content = Line::from(post_text);

    let stats = Line::from(vec![
        Span::raw("♥ "),
        Span::raw(post.post.like_count.unwrap_or(999).to_string()),
        Span::raw(" ⟲ "),
        Span::raw(post.post.repost_count.unwrap_or(999).to_string()),
    ]);

    let text = vec![
        header,
        Line::raw(""),
        content,
        Line::raw(""),
        stats,
    ];

    f.render_widget(
        Paragraph::new(text)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}
