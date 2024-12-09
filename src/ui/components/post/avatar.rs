use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style}, widgets::Widget};

use super::types::{PostComponent, PostContext, PostState};

pub struct PostAvatar {
    url: String,
    context: PostContext,
}

impl PostAvatar {
    pub fn new(url: String, context: PostContext) -> Self {
        // Initialize avatar loading in background
        let image_manager = context.image_manager.clone();
        let url_clone = url.clone();
        
        tokio::spawn(async move {
            if let Ok(Some(_)) = image_manager.get_decoded_image(&url_clone).await {
                log::info!("Pre-loaded avatar image");
            }
        });

        Self { url, context }
    }
}

impl PostComponent for PostAvatar {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &PostState) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        // Try to get cached Sixel
        if let Some(sixel) = self.context.image_manager.get_or_create_sixel(&self.url, area) {
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

    fn height(&self, _area: Rect) -> u16 {
        3 // Fixed avatar height
    }
}
