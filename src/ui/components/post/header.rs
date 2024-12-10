use chrono::{FixedOffset, Local};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use atrium_api::{app::bsky::feed::defs::PostViewData, types::{Unknown, string::Datetime}};

use super::types::{PostComponent, PostContext, PostState};

pub struct PostHeader {
    author_display_name: String,
    author_handle: String,
    timestamp: Datetime,
    is_reply: bool,
    following_status: FollowingStatus,
    context: PostContext,
}

#[derive(Clone, Copy)]
enum FollowingStatus {
    Following,
    NotFollowing,
    Self_,
}

impl PostHeader {
    pub fn new(post: &PostViewData, context: PostContext) -> Self {
        let author = &post.author;
        Self {
            author_display_name: author.display_name.clone().unwrap_or_else(|| author.handle.to_string()),
            author_handle: author.handle.to_string(),
            // Convert the API's Datetime to chrono's DateTime
            timestamp: post.indexed_at.clone(),
            is_reply: Self::check_is_reply(post),
            following_status: Self::determine_following_status(post),
            context,
        }
    }

    fn check_is_reply(post: &PostViewData) -> bool {
        if let Unknown::Object(record) = &post.record {
            record.get("reply").is_some()
        } else {
            false
        }
    }

    fn determine_following_status(post: &PostViewData) -> FollowingStatus {
        if let Some(viewer) = &post.author.viewer {
            if viewer.data.following.is_some() {
                FollowingStatus::Following
            } else {
                FollowingStatus::NotFollowing
            }
        } else {
            FollowingStatus::NotFollowing
        }
    }

    fn format_timestamp(&self) -> String {
        let time_posted = &self.timestamp;
        let fixed_offset: &chrono::DateTime<FixedOffset> = time_posted.as_ref();
        let local_time: chrono::DateTime<Local> = fixed_offset.with_timezone(&Local);
    
        local_time.format("%Y-%m-%d %-I:%M %p").to_string()
    }

    fn following_status_style(&self) -> (String, Style) {
        match self.following_status {
            FollowingStatus::Following => (
                "Following".to_string(),
                Style::default().fg(Color::Green),
            ),
            FollowingStatus::NotFollowing => (
                // "Not Following".to_string(),
                "".to_string(),
                Style::default(),
                // Style::default().fg(Color::Gray),
            ),
            FollowingStatus::Self_ => (
                "You".to_string(),
                Style::default().fg(Color::Yellow),
            ),
        }
    }

    fn build_header_spans(&self) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        
        // Author info
        spans.push(Span::styled(
            self.author_display_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" @".to_string()));
        spans.push(Span::raw(self.author_handle.clone()));

        // Reply indicator
        if self.is_reply {
            spans.push(Span::styled(" · ".to_string(), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("✉️".to_string(), Style::default()));
        }

        // Timestamp
        spans.push(Span::styled(" · ".to_string(), Style::default().fg(Color::DarkGray)));
        spans.push(Span::raw(self.format_timestamp()));

        // Following status
        let (following_status, following_style) = self.following_status_style();
        if !following_status.is_empty() {
            spans.push(Span::styled(" · ".to_string(), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(following_status, following_style));
        }

        spans
    }
}

impl PostComponent for PostHeader {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &PostState) {
        let header_spans = self.build_header_spans();
        let header_line = Line::from(header_spans);
        
        let paragraph = Paragraph::new(header_line)
            .wrap(ratatui::widgets::Wrap { trim: true });

        paragraph.render(area, buf);
    }

    fn height(&self, _area: Rect) -> u16 {
        1  // Header is always single line
    }
}
