use atrium_api::app::bsky::feed::defs::PostViewData;
use ratatui::{
    buffer::Buffer, 
    layout::{Rect, Layout, Direction, Constraint},
    widgets::{Block, Borders, Widget},
    style::{Color, Style},
};

use super::{
    header::PostHeader,
    content::PostContent,
    stats::PostStats,
    types::{PostComponent, PostContext, PostState}
};

pub struct QuotedPost {
    post: PostViewData,
    components: Vec<Box<dyn PostComponent>>,
    context: PostContext,
}

impl QuotedPost {
    pub fn new(post: PostViewData, context: PostContext) -> Self {
        let mut components: Vec<Box<dyn PostComponent>> = vec![];
        
        // Add header component
        components.push(Box::new(PostHeader::new(&post, context.clone())));
        
        // Add content component
        components.push(Box::new(PostContent::new(&post, context.clone())));
        
        // Add stats component
        components.push(Box::new(PostStats::new(&post, context.clone())));

        Self { 
            post,
            components,
            context,
        }
    }
}

impl PostComponent for QuotedPost {
    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &PostState) {
        // Create quoted post block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray))  // Dimmer border for quoted posts
            .title("Quoted Post");

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Create layout for components
        let component_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(1),  // Header
                Constraint::Min(1),     // Content
                Constraint::Length(1),  // Stats
            ])
            .split(inner_area);

        // Render each component in its designated area
        for (component, area) in self.components.iter_mut().zip(component_areas.iter()) {
            component.render(*area, buf, state);
        }
    }

    fn height(&self, area: Rect) -> u16 {
        // Account for block borders
        let inner_width = area.width.saturating_sub(2);
        let inner_area = Rect { width: inner_width, ..area };
        
        // Sum component heights
        let content_height = self.components.iter()
            .map(|c| c.height(inner_area))
            .sum::<u16>();

        // Add borders
        content_height + 2
    }
}
