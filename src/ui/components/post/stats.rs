use atrium_api::app::bsky::feed::defs::PostViewData;
use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style}, text::{Line, Span}, widgets::Widget};

use super::types::{PostComponent, PostContext, PostState};

pub struct PostStats {
    likes: u32,
    reposts: u32,
    replies: u32,
    has_liked: bool,
    has_reposted: bool,
    context: PostContext,
}

impl PostStats {
    pub fn new(post: &PostViewData, context: PostContext) -> Self {
        Self {
            likes: post.like_count.unwrap_or(0) as u32,
            reposts: post.repost_count.unwrap_or(0) as u32,
            replies: post.reply_count.unwrap_or(0) as u32,
            has_liked: Self::check_liked(post),
            has_reposted: Self::check_reposted(post),
            context,
        }
    }

    pub fn check_liked(post: &PostViewData) -> bool {
        post.viewer
            .as_ref()
            .and_then(|v| v.data.like.as_ref())
            .is_some()
    }

    pub fn check_reposted(post: &PostViewData) -> bool {
        post.viewer
            .as_ref()
            .and_then(|v| v.data.repost.as_ref())
            .is_some()
    }
    
    fn get_stats(&self) -> Line<'static> {
        let like_text = format!("{}", self.likes);
        let repost_text = format!("{}", self.reposts);
        let reply_text = format!("{}", self.replies);
    
        Line::from(vec![
            // Like section
            Span::styled(
                if self.has_liked { "❤️ " } else { "🤍 " },
                Style::default(),
            ),
            Span::styled(like_text, Style::default().fg(Color::White)),
            
            // Subtle divider
            Span::styled(" · ", Style::default().fg(Color::DarkGray)),
            
            // Repost section
            Span::styled(
                if self.has_reposted { "✨ " } else { "🔁 " },
                Style::default(),
            ),
            Span::styled(repost_text, Style::default().fg(Color::White)),
            
            // Subtle divider
            Span::styled(" · ", Style::default().fg(Color::DarkGray)),
            
            // Reply section
            Span::styled("💭 ", Style::default()),
            Span::styled(reply_text, Style::default().fg(Color::White)),
        ])
    }
}

impl PostComponent for PostStats {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &PostState) {
        let stats = self.get_stats();
        stats.render(area, buf);
    }

    fn height(&self, _area: Rect) -> u16 {
        1 // Fixed stats height
    }
}
