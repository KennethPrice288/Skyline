use std::sync::Arc;

use atrium_api::app::bsky::embed::images::ViewImage;
use atrium_api::app::bsky::embed::record_with_media::ViewMediaRefs;
use atrium_api::app::bsky::feed::defs::PostView;
use atrium_api::app::bsky::feed::defs::PostViewEmbedRefs;
use atrium_api::types::Unknown;
use chrono::{FixedOffset, Local};
use ipld_core::ipld::Ipld;
use log::info;
use ratatui::widgets::Paragraph;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, StatefulWidget, Widget},
};

use super::images::{ImageManager, PostImage};

pub struct PostState {
    pub selected: bool,
}

pub struct PostAvatar {
    url: String,
    image_manager: Arc<ImageManager>,
}

impl PostAvatar {
    fn new(url: String, image_manager: Arc<ImageManager>) -> Self {
        Self { url, image_manager }
    }
}

impl Widget for &PostAvatar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        // Try to get cached Sixel
        if let Some(sixel) = self.image_manager.get_or_create_sixel(&self.url, area) {
            let protocol = ratatui_image::protocol::Protocol::Sixel(sixel);
            ratatui_image::Image::new(&protocol).render(area, buf);
        } else {
            // Loading indicator - just a placeholder circle when loading
            buf.set_string(
                area.x,
                area.y,
                "○",
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}

pub struct Post {
    pub post: PostView,
    image_manager: Arc<ImageManager>,
    avatar: Option<PostAvatar>,
}

impl Post {

    pub fn new(post: PostView, image_manager: Arc<ImageManager>) -> Self {
        // Create avatar if URL exists
        let avatar = post.data.author.avatar.as_ref().map(|url| {
            // Start loading the avatar image in the background
            let image_manager_clone = image_manager.clone();
            let url_clone = url.clone();
            
            tokio::spawn(async move {
                if let Ok(Some(_)) = image_manager_clone.get_decoded_image(&url_clone).await {
                    info!("Successfully pre-loaded avatar image for post");
                }
            });

            PostAvatar::new(url.clone(), image_manager.clone())
        });

        // Start a background task to load post images if they exist
        if let Some(images) = Self::extract_images_from_post(&post) {
            info!("Found {} images in post", images.len());
            let image_manager_clone = image_manager.clone();

            tokio::spawn(async move {
                for image in images {
                    info!("Starting load for image {}", image.thumb);
                    match image_manager_clone.get_decoded_image(&image.thumb).await {
                        Ok(Some(_)) => info!("Successfully loaded image {}", image.thumb),
                        Ok(None) => info!("No image loaded for {}", image.thumb),
                        Err(e) => info!("Error loading image {}: {:?}", image.thumb, e),
                    }
                }
            });
        }

        Self {
            post,
            image_manager,
            avatar,
        }
    }

    pub fn get_uri(&self) -> String {
        return self.post.data.uri.clone();
        // return self.post.post.uri.clone();
    }

    fn has_liked(&self) -> bool {
        self.post.viewer
            .as_ref()
            .and_then(|v| v.data.like.as_ref())
            .is_some()
    }

    fn has_reposted(&self) -> bool {
        self.post.viewer
            .as_ref()
            .and_then(|v| v.data.repost.as_ref())
            .is_some()
    }

    fn get_stats(&self) -> Line<'static> {
        let like_text = format!("{}", self.post.data.like_count.unwrap_or(0));
        let repost_text = format!("{}", self.post.data.repost_count.unwrap_or(0));
        let reply_text = format!("{}", self.post.data.reply_count.unwrap_or(0));
    
        Line::from(vec![
            // Like section
            Span::styled(
                if self.has_liked() { "❤️ " } else { "🤍 " },
                Style::default(),
            ),
            Span::styled(like_text, Style::default().fg(Color::White)),
            
            // Subtle divider
            Span::styled(" · ", Style::default().fg(Color::DarkGray)),
            
            // Repost section
            Span::styled(
                if self.has_reposted() { "✨ " } else { "🔁 " },
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

    fn get_reply_info(&self) -> Option<String> {
    
        if let Unknown::Object(record) = &self.post.data.record {
            
            if let Some(reply) = record.get("reply") {
                
                let reply_ipld = &**reply;
                
                if let ipld_core::ipld::Ipld::Map(reply_map) = reply_ipld {
                    
                    if let Some(parent) = reply_map.get("parent") {
                        
                        if let ipld_core::ipld::Ipld::Map(parent_map) = parent {
                            
                            // Get URI and handle together if possible
                            let uri = parent_map.get("uri");
                            if let Some(uri) = uri {
                                if let ipld_core::ipld::Ipld::String(uri_str) = uri {
                                    return Some(uri_str.clone());
                                }
                            }
                        }
                    }
                }
            } else {
            }
        }
        None
    }

    fn get_header(&self) -> Paragraph<'static> {
        let author = &self.post.data.author;
        let author_handle = author.handle.to_string();
        let author_display_name = author.display_name.clone().unwrap_or(author_handle.clone());

        let time_posted = &self.post.data.indexed_at;
        let fixed_offset: &chrono::DateTime<FixedOffset> = time_posted.as_ref();
        let local_time: chrono::DateTime<Local> = fixed_offset.with_timezone(&Local);

        let formatted_time = local_time.format("%Y-%m-%d %-I:%M %p").to_string();

        let mut spans = vec![
            Span::styled(
                author_display_name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" @"),
            Span::raw(author_handle),
        ];
    
        // Add reply indicator if it's a reply
        if let Some(_uri) = self.get_reply_info() {
            spans.extend_from_slice(&[
                Span::styled(
                    " · ✉️",
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
        }
    
        spans.extend_from_slice(&[
            Span::styled(
                " · ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(formatted_time),
        ]);
    
        Paragraph::new(Line::from(spans)).wrap(ratatui::widgets::Wrap { trim: true })
    }

    pub fn extract_images_from_post(post: &PostView) -> Option<Vec<ViewImage>> {
        if let Some(embed) = &post.data.embed {
            match embed {
                atrium_api::types::Union::Refs(refs) => match refs {
                    PostViewEmbedRefs::AppBskyEmbedImagesView(images_view) => {
                        Some(images_view.images.clone())
                    }
                    PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(record_with_media) => {
                        match &record_with_media.media {
                            atrium_api::types::Union::Refs(media_refs) => match media_refs {
                                ViewMediaRefs::AppBskyEmbedImagesView(images_view) => {
                                    Some(images_view.images.clone())
                                }
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    _ => None,
                },
                atrium_api::types::Union::Unknown(_) => None,
            }
        } else {
            None
        }
    }

}

impl StatefulWidget for &mut Post {
    type State = PostState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Skip rendering entirely if no space
        if area.height == 0 {
            return;
        }

        let post_text = match &self.post.data.record {
            Unknown::Object(map) => match map.get("text") {
                Some(data_model) => match &**data_model {
                    Ipld::String(text) => text.clone(),
                    Ipld::Null => "(Null content)".to_string(),
                    other => format!("(Unexpected format: {:?})", other),
                },
                None => "(No text content)".to_string(),
            },
            Unknown::Null => "(Null content)".to_string(),
            Unknown::Other(data) => format!("Other: {:?}", data),
        };

        let header = self.get_header();
        let content = ratatui::widgets::Paragraph::new(post_text)
            .wrap(ratatui::widgets::Wrap { trim: false });

        let stats = self.get_stats();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if state.selected {
                Color::Blue
            } else {
                Color::White
            }));

        let inner_area = block.inner(area);

        let images = Post::extract_images_from_post(&self.post);

        if inner_area.height > 0 {
            let avatar_width = 3; // Space for small avatar
            
            // Split horizontally first to create avatar column
            let horizontal_split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(avatar_width),
                    Constraint::Min(20),
                ])
                .split(inner_area);

            // Avatar area
            if let Some(avatar) = &self.avatar {
                let avatar_area = Rect {
                    x: horizontal_split[0].x,
                    y: horizontal_split[0].y,
                    width: avatar_width,
                    height: 3, // Small square avatar
                };
                avatar.render(avatar_area, buf);
            }

            // Content area
            let content_constraints = if images.is_some() {
                vec![
                    Constraint::Length(1),  // Header
                    Constraint::Min(1),     // Content
                    Constraint::Length(15), // Images
                    Constraint::Length(1),  // Stats
                ]
            } else {
                vec![
                    Constraint::Length(1), // Header
                    Constraint::Min(1),    // Content
                    Constraint::Length(1), // Stats
                ]
            };

            let content_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(content_constraints)
                .split(horizontal_split[1]);

            if images.is_some() && !images.as_ref().unwrap().is_empty() {
                let image_area = content_chunks[2];
                if let Some(first_image_data) = images.unwrap().get(0) {
                    let mut first_image = PostImage::new(first_image_data.clone(), self.image_manager.clone());
                    first_image.render(image_area, buf);
                }
            }

            block.render(area, buf);
            header.render(content_chunks[0], buf);
            content.render(content_chunks[1], buf);
            stats.render(content_chunks[content_chunks.len() - 1], buf);
        }
    }
}
