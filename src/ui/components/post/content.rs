use atrium_api::{app::bsky::feed::defs::PostViewData, types::Unknown};
use ipld_core::ipld::Ipld;
use ratatui::{buffer::Buffer, layout::Rect, widgets::{Paragraph, Widget, Wrap}};

use super::types::{PostComponent, PostContext, PostState};

pub struct PostContent {
    text: String,
    context: PostContext,
}

impl PostContent {
    pub fn new(post: &PostViewData, context: PostContext) -> Self {
        let text = Self::extract_text_content(post);
        Self { text, context }
    }

    fn extract_text_content(post: &PostViewData) -> String {
        match &post.record {
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
        }
    }

    fn calculate_height(&self, width: u16) -> u16 {
        // Account for borders and padding (2 chars on each side)
        let usable_width = width.saturating_sub(4);
        
        // Calculate how many characters fit per line
        let chars_per_line = if usable_width > 0 {
            usable_width as usize
        } else {
            1
        };
        
        let wrapped_lines = textwrap::fill(&self.text, chars_per_line)
            .lines()
            .count();
        
        wrapped_lines as u16
    }
}

impl PostComponent for PostContent {
    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &PostState) {
        let paragraph = Paragraph::new(self.text.clone())
            .wrap(Wrap { trim: true });
        paragraph.render(area, buf);
    }

    fn height(&self, area: Rect) -> u16 {
        self.calculate_height(area.width)
    }
}
