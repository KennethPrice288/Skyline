use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Constraint, Direction},
    style::{Style, Color},
    widgets::{Widget, Block, Borders, Paragraph},
    text::{Line, Span},
};
use atrium_api::app::bsky::actor::defs::ProfileViewDetailed;
use std::sync::Arc;
use super::images::ImageManager;

pub struct AuthorAvatar {
    pub url: String,
    pub image_manager: Arc<ImageManager>,
}

impl Widget for &AuthorAvatar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Only attempt to render if we have enough space
        if area.width < 2 || area.height < 2 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Avatar");
        
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Try to get cached Sixel
        if let Some(sixel) = self.image_manager.get_or_create_sixel(&self.url, inner_area) {
            let protocol = ratatui_image::protocol::Protocol::Sixel(sixel);
            ratatui_image::Image::new(&protocol).render(inner_area, buf);
        } else {
            // Loading indicator
            buf.set_string(
                inner_area.x,
                inner_area.y,
                "Loading...",
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}

pub struct AuthorProfile {
    profile: ProfileViewDetailed,
    height: u16,
    avatar: Option<AuthorAvatar>,
}

impl AuthorProfile {
    pub fn new(profile: ProfileViewDetailed, image_manager: Arc<ImageManager>) -> Self {
        let avatar = profile.avatar.as_ref().map(|url| AuthorAvatar {
            url: url.clone(),
            image_manager: image_manager.clone(),
        });

        // Start loading the avatar image in the background if we have a URL
        if let Some(avatar) = &avatar {
            let image_manager = image_manager.clone();
            let url = avatar.url.clone();
            
            tokio::spawn(async move {
                if let Ok(Some(_)) = image_manager.get_decoded_image(&url).await {
                    log::info!("Successfully pre-loaded avatar image");
                }
            });
        }

        Self {
            profile,
            height: 8, // Fixed height for profile section
            avatar,
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
        
        let horizontal_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(12), // Avatar width
                Constraint::Min(20),    // Profile info
            ])
            .split(inner_area);

        // Render avatar if available
        if let Some(avatar) = &self.avatar {
            let avatar_area = Rect {
                x: horizontal_layout[0].x,
                y: horizontal_layout[0].y,
                width: 12,
                height: 6,
            };
            avatar.render(avatar_area, buf);
        }

        // Profile info layout
        let info_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Name and handle
                Constraint::Length(2), // Stats
                Constraint::Min(2),    // Bio
            ])
            .split(horizontal_layout[1]);

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
        Paragraph::new(name_line).render(info_layout[0], buf);
        Paragraph::new(stats_line).render(info_layout[1], buf);
        bio_widget.render(info_layout[2], buf);
    }
}
