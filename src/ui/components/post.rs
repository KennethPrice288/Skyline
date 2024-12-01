use ipld_core::ipld::Ipld;
use ratatui::{
    buffer::Buffer, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Paragraph, StatefulWidget, Widget}, Frame
};
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use atrium_api::types::Unknown;
use chrono::{FixedOffset, Local};

pub struct PostState {
    pub selected: bool,
    pub liked: bool,
    pub reposted: bool,
}

pub struct Post {
    post: FeedViewPost,
}

impl Post {
    pub fn new(post: FeedViewPost) -> Self {
        Self { post }
    }
}

impl StatefulWidget for Post {
    type State = PostState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(if state.selected { Color::Blue } else { Color::White }));
        let author = &self.post.post.author;
        
        // Debug the record content
        let post_text = match &self.post.post.record {
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

        let time_posted = &self.post.post.indexed_at;
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
            Span::styled("
                 posted at ",
                Style::default().add_modifier(Modifier::BOLD)
            ),
            Span::raw(formatted_time),
        ]);
        let content = Line::from(post_text);

        let stats = {
            let like_text = format!("{}", self.post.post.like_count.unwrap_or(999));
            let repost_text = format!("{}", self.post.post.repost_count.unwrap_or(999));
        
            Line::from(vec![
                Span::styled("♥ ", Style::default().fg(Color::Red)),
                Span::styled(like_text, Style::default().fg(Color::White)),
                Span::styled(" ⟲ ", Style::default().fg(Color::Blue)),
                Span::styled(repost_text, Style::default().fg(Color::White)),
            ])
        };
        

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if state.selected { Color::Yellow } else { Color::White }));

        let inner_area = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Header
                Constraint::Min(1),     // Content
                Constraint::Length(1)   // Stats
            ])
            .split(inner_area);

        block.render(area, buf);
        header.render(chunks[0], buf);
        content.render(chunks[1], buf);
        stats.render(chunks[2], buf);

    }
}
