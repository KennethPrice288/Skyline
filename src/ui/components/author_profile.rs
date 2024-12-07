use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Constraint, Direction},
    style::{Style, Color},
    widgets::{Widget, Block, Borders, Paragraph},
    text::{Line, Span},
};
use atrium_api::app::bsky::actor::defs::ProfileViewDetailed;

pub struct AuthorProfile {
    profile: ProfileViewDetailed,
    height: u16,
}

impl AuthorProfile {
    pub fn new(profile: ProfileViewDetailed) -> Self {
        Self {
            profile,
            height: 8, // Fixed height for profile section
        }
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

impl Widget for &AuthorProfile {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Profile");

        let inner_area = block.inner(area);
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Name and handle
                Constraint::Length(2), // Stats
                Constraint::Min(2),    // Bio
            ])
            .split(inner_area);

        // Render name and handle
        let name_line = Line::from(vec![
            Span::styled(
                self.profile.display_name.clone().unwrap_or_default(),
                Style::default().fg(Color::White),
            ),
            Span::raw(" @"),
            Span::styled(
                &*self.profile.handle,
                Style::default().fg(Color::Gray),
            ),
        ]);
        
        // Render stats
        let stats_line = Line::from(vec![
            Span::raw(format!("📝 {} Posts", self.profile.posts_count.unwrap_or(8008))),
            Span::raw(" · "),
            Span::raw(format!("👥 {} Following", self.profile.follows_count.unwrap_or(8008))),
            Span::raw(" · "),
            Span::raw(format!("👥 {} Followers", self.profile.followers_count.unwrap_or(8008))),
        ]);

        // Render bio
        let bio = self.profile.description.clone().unwrap_or_default();
        let bio_widget = Paragraph::new(bio)
            .wrap(ratatui::widgets::Wrap { trim: true });

        block.render(area, buf);
        Paragraph::new(name_line).render(chunks[0], buf);
        Paragraph::new(stats_line).render(chunks[1], buf);
        bio_widget.render(chunks[2], buf);
    }
}
