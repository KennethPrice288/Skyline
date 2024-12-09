use atrium_api::app::bsky::{embed::{images::ViewImage, record::ViewRecordRefs, record_with_media::ViewMediaRefs}, feed::defs::{PostView, PostViewData, PostViewEmbedRefs}};
use avatar::PostAvatar;
use content::PostContent;
use header::PostHeader;
use images::PostImages;
use quoted_post::QuotedPost;
use ratatui::{buffer::Buffer, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Style}, widgets::{Block, Borders, StatefulWidget, Widget}};
use stats::PostStats;
use types::{PostComponent, PostContext, PostState};

pub mod avatar;
pub mod content;
pub mod header;
pub mod images;
pub mod quoted_post;
pub mod stats;
pub mod types;

pub struct Post {
    // components: Vec<Box<dyn PostComponent>>,
    header: Box<PostHeader>,
    avatar: Option<Box<PostAvatar>>,
    content: Box<dyn PostComponent>,
    quoted_post: Option<Box<QuotedPost>>,
    images: Option<Box<PostImages>>,
    stats: Box<dyn PostComponent>,
    context: PostContext,
    uri: String,
}

impl Post {
    pub fn new(post: PostView, context: PostContext) -> Self {
        let mut components: Vec<Box<dyn PostComponent>> = vec![];
        let mut avatar = None;
        let mut quoted_post = None;
        let mut images = None;
        
        // Add avatar if available
        if let Some(avatar_url) = &post.data.author.avatar {
            components.push(Box::new(PostAvatar::new(
                avatar_url.clone(),
                context.clone(),
            )));
            avatar = Some(Box::new(PostAvatar::new(
                avatar_url.clone(),
                context.clone(),
            )));
        }

        // Add other components
        let header = Box::new(PostHeader::new(&post.data, context.clone()));
        let content = Box::new(PostContent::new(&post.data, context.clone()));
        
        // Add quoted post if present
        if let Some(quoted) = Self::extract_quoted_post_data(&post) {
            quoted_post = Some(Box::new(QuotedPost::new(quoted, context.clone())));
        }

        // Add images if present
        if let Some(extracted_images) = Self::extract_images_from_post(&post) {
            images = Some(Box::new(PostImages::new(extracted_images, context.clone())));
        }

        let stats = Box::new(PostStats::new(&post.data, context.clone()));

        let uri = post.data.uri;

        Self {
            header,
            avatar,
            content,
            quoted_post,
            images,
            stats,
            context,
            uri,
        }
    }
    pub fn extract_quoted_post_data(post: &PostView) -> Option<PostViewData> {
        if let Some(embed) = &post.data.embed {
            match embed {
                atrium_api::types::Union::Refs(refs) => {
                    if let PostViewEmbedRefs::AppBskyEmbedRecordView(record_view) = refs {
                        return match &record_view.data.record {
                            atrium_api::types::Union::Refs(refs) => {
                                if let ViewRecordRefs::ViewRecord(view_record) = refs {
                                    return Some(
                                        PostViewData {
                                            author: view_record.author.clone(),
                                            cid: view_record.cid.clone(),
                                            embed: None,
                                            indexed_at: view_record.indexed_at.clone(),
                                            labels: view_record.labels.clone(),
                                            like_count: view_record.like_count,
                                            quote_count: view_record.quote_count,
                                            record: view_record.value.clone(),
                                            reply_count: view_record.reply_count,
                                            repost_count: view_record.repost_count,
                                            threadgate: None,
                                            uri: view_record.uri.clone(),
                                            viewer: None,
                                        }
                                    );
                                }
                                None
                            },
                            atrium_api::types::Union::Unknown(unknown_data) => {
                                log::warn!("Unknown data from extract_quoted_post_data: {:?}", unknown_data);
                                None
                            },
                        };
                    }
                }
                atrium_api::types::Union::Unknown(_) => {}
            }
        }
        None
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

    pub fn get_uri(&self) -> &String {
        return &self.uri;
    }
    pub fn has_avatar(&self) -> bool {
        return self.avatar.is_some();
    }
}

impl StatefulWidget for &mut Post {
    type State = PostState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(
                if state.selected { Color::Blue } else { Color::White }
            ));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let mut current_y = inner_area.y;
        let max_y = inner_area.y + inner_area.height;

        let horizontal_areas = if self.has_avatar() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                ])
                .split(Rect {
                    x: inner_area.x,
                    y: current_y,
                    width: inner_area.width,
                    height: 1,
                })
            } else {
                Layout::default()
                .direction(Direction::Horizontal)
                .split(Rect {
                    x: inner_area.x,
                    y: current_y,
                    width: inner_area.width,
                    height: 1
                })
            };
        let has_avatar = self.has_avatar();
        if has_avatar {
            self.avatar.as_mut().unwrap().render(horizontal_areas[0], buf, state);
            self.header.render(horizontal_areas[1], buf, state);
        } else {
            self.header.render(horizontal_areas[0], buf, state);
        }
        current_y += 1;

        let mut remaining_height = max_y.saturating_sub(current_y);
        if remaining_height == 0 {
            return;
        }
        let content_height = self.content.height(inner_area).min(remaining_height);
        let content_area = Rect {
            x: inner_area.x,
            y: current_y,
            width: inner_area.width,
            height: content_height,
        };
        self.content.render(content_area, buf, state);
        current_y += content_height;
        remaining_height = max_y.saturating_sub(current_y);
        if remaining_height == 0 {
            return;
        }

        if let Some(images) = &mut self.images {
            let image_height = images.height(inner_area).min(remaining_height);
            let image_area = Rect {
                x: inner_area.x,
                y: current_y,
                width: inner_area.width,
                height: image_height,
            };
            images.render(image_area, buf, state);
            current_y += image_height;
            remaining_height = max_y.saturating_sub(current_y);
            if remaining_height == 0 {
                return;
            }
        }

        if let Some(quoted_post) = &mut self.quoted_post {
            let quote_height = quoted_post.height(inner_area).min(remaining_height);
            let quote_area = Rect {
                x: inner_area.x,
                y: current_y,
                width: inner_area.width,
                height: quote_height,
            };
            quoted_post.render(quote_area, buf, state);
            current_y += quote_height;
            remaining_height = max_y.saturating_sub(current_y);
            if remaining_height == 0 {
                return;
            }
        }

        let stats_height = self.stats.height(inner_area).min(remaining_height);
        let stats_area = Rect {
            x: inner_area.x,
            y: current_y,
            width: inner_area.width,
            height: stats_height,
        };
        self.stats.render(stats_area, buf, state);
    }
}
