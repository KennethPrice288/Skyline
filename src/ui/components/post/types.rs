use atrium_api::app::bsky::feed::defs::PostViewData;
use ratatui::{buffer::Buffer, layout::Rect};
use std::sync::Arc;

use crate::ui::components::images::ImageManager;

pub struct PostState {
    pub selected: bool,
}

pub trait PostComponent {
    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &PostState);
    fn height(&self, area: Rect) -> u16;
}

#[derive(Clone)]
pub struct PostContext {
    pub image_manager: Arc<ImageManager>,
    pub indent_level: u16,
}
