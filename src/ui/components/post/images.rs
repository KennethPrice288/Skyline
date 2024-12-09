use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use atrium_api::app::bsky::embed::images::ViewImage;

use super::types::{PostComponent, PostContext, PostState};

pub struct PostImages {
    images: Vec<ViewImage>,
    context: PostContext,
    cached_sixels: Vec<Option<ratatui_image::protocol::sixel::Sixel>>,
}

impl PostImages {
    pub fn new(images: Vec<ViewImage>, context: PostContext) -> Self {
        // Start background loading of images
        let image_manager = context.image_manager.clone();
        for image in &images {
            let image_manager = image_manager.clone();
            let thumb_url = image.thumb.clone();
            
            tokio::spawn(async move {
                if let Ok(Some(_)) = image_manager.get_decoded_image(&thumb_url).await {
                    log::info!("Pre-loaded post image: {}", thumb_url);
                }
            });
        }

        let images_len = images.len();

        Self {
            images,
            context,
            cached_sixels: vec![None; images_len],
        }
    }

    fn render_single_image(
        image: &ViewImage,
        sixel: Option<&ratatui_image::protocol::sixel::Sixel>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),  // Image
                Constraint::Percentage(50),  // Alt text
            ])
            .split(area);

        let image_area = layout[0];
        let alt_text_area = layout[1];

        // Render image or loading indicator
        if let Some(sixel) = sixel {
            let protocol = ratatui_image::protocol::Protocol::Sixel(sixel.clone());
            ratatui_image::Image::new(&protocol).render(image_area, buf);
        } else {
            buf.set_string(
                image_area.x,
                image_area.y,
                "Loading image...",
                Style::default().fg(Color::DarkGray),
            );
        }

        // Render alt text
        let alt_text = if image.alt.is_empty() {
            "No alt text provided"
        } else {
            &image.alt
        };

        let alt_content = vec![
            Line::from(Span::styled("📷", Style::default().fg(Color::Gray))),
            Line::from(Span::styled(alt_text, Style::default().fg(Color::Gray))),
        ];

        Paragraph::new(alt_content)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(alt_text_area, buf);
    }

    fn update_cached_sixels(&mut self, area: Rect) {
        for (i, image) in self.images.iter().enumerate() {
            if self.cached_sixels[i].is_none() {
                if let Some(sixel) = self.context.image_manager
                    .get_or_create_sixel(&image.thumb, area) {
                    self.cached_sixels[i] = Some(sixel);
                }
            }
        }
    }
}

impl PostComponent for PostImages {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &PostState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Images");

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Update sixels first
        self.update_cached_sixels(inner_area);

        // Then get references to the data we need
        if let Some(first_image) = self.images.first() {
            if let Some(first_sixel) = self.cached_sixels.first() {
                Self::render_single_image(first_image, first_sixel.as_ref(), inner_area, buf);
            }
        }
    }

    fn height(&self, _area: Rect) -> u16 {
        if self.images.is_empty() {
            0
        } else {
            15  // Fixed height for image area
        }
    }
}
