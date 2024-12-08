// In src/ui/components/notifications.rs
use std::{collections::{HashMap, VecDeque}, sync::Arc};
use atrium_api::{app::bsky::{feed::defs::PostViewData, notification::list_notifications::NotificationData}, types::LimitedNonZeroU8};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};
use crate::{client::api::API, ui::views::{View, ViewStack}};
use anyhow::Result;

use super::{images::ImageManager, post_list::{PostList, PostListBase}};

pub struct NotificationView {
    pub notifications: VecDeque<NotificationData>,
    pub notification_heights: HashMap<String, u16>,
    pub image_manager: Arc<ImageManager>,
    base: PostListBase,
}

impl NotificationView {
    pub fn new(image_manager: Arc<ImageManager>) -> Self {
        Self {
            notifications: VecDeque::new(),
            notification_heights: HashMap::new(),
            image_manager,
            base: PostListBase::new(),
        }
    }

    pub async fn load_notifications(&mut self, api: &mut crate::client::api::API) -> anyhow::Result<()> {
        let params = atrium_api::app::bsky::notification::list_notifications::Parameters {
            data: atrium_api::app::bsky::notification::list_notifications::ParametersData {
                cursor: None,
                limit: Some(LimitedNonZeroU8::MAX),
                seen_at: None,
                priority: None,
            },
            extra_data: ipld_core::ipld::Ipld::Null,
        };

        match api.agent.api.app.bsky.notification.list_notifications(params).await {
            Ok(response) => {
                self.notifications.clear();
                for notification in &response.notifications {
                    self.notifications.push_back(notification.data.clone());
                }
                self.base.selected_index = 0;
                self.base.scroll_offset = 0;
                Ok(())
            }
            Err(e) => Err(e.into())
        }
    }

    fn get_notification_color(&self, reason: &str) -> Color {
        match reason {
            "like" => Color::Red,
            "repost" => Color::Green,
            "follow" => Color::Blue,
            "reply" => Color::Yellow,
            "mention" => Color::Cyan,
            "quote" => Color::Magenta,
            _ => Color::White,
        }
    }

    fn get_notification_icon(&self, reason: &str) -> &str {
        match reason {
            "like" => "❤️",
            "repost" => "🔁",
            "follow" => "👤",
            "reply" => "💬",
            "mention" => "@",
            "quote" => "💭",
            _ => "📨",
        }
    }

    fn format_notification(&self, notification: &NotificationData) -> String {
        let icon = self.get_notification_icon(&notification.reason);
        let action = match notification.reason.as_str() {
            "like" => "liked your post",
            "repost" => "reposted your post",
            "follow" => "followed you",
            "reply" => "replied to your post",
            "mention" => "mentioned you",
            "quote" => "quoted your post",
            _ => "interacted with you",
        };
        
        format!(
            "{} @{} {}",
            icon,
            notification.author.handle.to_string(),
            action
        )
    }
    pub fn get_notification(&self) -> NotificationData {
        let selected_idx = self.selected_index();
        return self.notifications[selected_idx].clone();
    }

    pub async fn handle_new_notification(
        &mut self,
        _uri: String,
        api: &API,
    ) -> Result<()> {
        // Use existing API call to get fresh notifications
        let params = atrium_api::app::bsky::notification::list_notifications::Parameters {
            data: atrium_api::app::bsky::notification::list_notifications::ParametersData {
                cursor: None,
                limit: Some(LimitedNonZeroU8::MIN),  // Just get latest
                seen_at: None,
                priority: None,
            },
            extra_data: ipld_core::ipld::Ipld::Null,
        };

        match api.agent.api.app.bsky.notification.list_notifications(params).await {
            Ok(response) => {
                if let Some(new_notification) = response.notifications.first() {
                    // Only add if it's actually new
                    if !self.notifications.iter().any(|n| n.uri == new_notification.data.uri) {
                        self.notifications.push_front(new_notification.data.clone());
                        self.notification_heights.insert(new_notification.data.uri.clone(), 3);
                    }
                }
                return Ok(())
            }
            Err(e) =>return Err(e.into())
        }
    }
}

impl PostList for NotificationView {
    fn get_total_height_before_scroll(&self) -> u16 {
        self.notifications
            .iter()
            .take(self.base.scroll_offset)
            .filter_map(|notif| self.notification_heights.get(&notif.uri))
            .sum()
    }

    fn get_last_visible_index(&self, area_height: u16) -> usize {
        let mut total_height = 0;
        let mut last_visible = self.base.scroll_offset;

        for (i, notification) in self.notifications.iter().enumerate().skip(self.base.scroll_offset) {
            let height = self.notification_heights
                .get(&notification.uri)
                .copied()
                .unwrap_or(3);

            if total_height + height > area_height {
                break;
            }

            total_height += height;
            last_visible = i;
        }

        last_visible
    }

    fn ensure_post_heights(&mut self, _area: Rect) {
        let notifications_to_calculate: Vec<_> = self.notifications
            .iter()
            .filter(|notif| !self.notification_heights.contains_key(&notif.uri))
            .cloned()
            .collect();

        for notification in notifications_to_calculate {
            // Each notification takes 3 lines: content, status, and padding
            self.notification_heights.insert(notification.uri, 3);
        }
    }

    fn scroll_down(&mut self) {
        if self.base.selected_index >= self.notifications.len().saturating_sub(1) {
            return;
        }

        let next_index = self.base.selected_index + 1;
        let mut y_position = 0;

        // Calculate if we need to adjust scroll_offset
        for (i, notification) in self.notifications.iter().enumerate().skip(self.base.scroll_offset) {
            if i == next_index {
                let height = self.notification_heights
                    .get(&notification.uri)
                    .copied()
                    .unwrap_or(3);
                
                // If the next selection would be off screen, increment scroll offset
                if y_position + height > self.base.last_known_height {
                    self.base.scroll_offset += 1;
                }
                break;
            }
            
            y_position += self.notification_heights
                .get(&notification.uri)
                .copied()
                .unwrap_or(3);
        }

        self.base.selected_index = next_index;
    }

    fn scroll_up(&mut self) {
        if self.base.selected_index == 0 {
            return;
        }

        self.base.selected_index -= 1;
        
        // Adjust scroll offset if we're scrolling above current view
        if self.base.selected_index < self.base.scroll_offset {
            self.base.scroll_offset = self.base.selected_index;
        }
    }


    fn needs_more_content(&self) -> bool {
        self.selected_index() > self.notifications.len().saturating_sub(5)
    }

    fn selected_index(&self) -> usize {
        self.base.selected_index
    }

    // This allows us to get the author from a notification when 'a' is pressed
    fn get_post(&self, _index: usize) -> Option<PostViewData> {
        // Since we need to return a PostViewData but have NotificationData,
        // we'll return None to indicate this is a notification view
        // The author information will be handled separately
        None
    }
}

impl Widget for &mut NotificationView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("🌆 Notifications");
        
        let inner_area = block.inner(area);
        block.render(area, buf);

        self.base.last_known_height = area.height;
        let mut current_y = inner_area.y;

        for (i, notification) in self.notifications
            .iter()
            .enumerate()
            .skip(self.base.scroll_offset)
        {
            let height = self.notification_heights
                .get(&notification.uri)
                .copied()
                .unwrap_or(3);

            let remaining_height = inner_area.height.saturating_sub(current_y - inner_area.y);
            if remaining_height == 0 {
                break;
            }

            let notification_area = Rect {
                x: inner_area.x,
                y: current_y,
                width: inner_area.width,
                height: remaining_height.min(height),
            };

            // Create selection background
            if i == self.base.selected_index {
                // Fill the entire notification area with a highlight
                for y in notification_area.y..notification_area.y + height {
                    buf.set_style(
                        Rect {
                            x: notification_area.x,
                            y,
                            width: notification_area.width,
                            height: 1,
                        },
                        Style::default().bg(Color::DarkGray)
                    );
                }
            }

            // Render notification content
            let formatted = self.format_notification(notification);
            let content_style = Style::default()
                .fg(if i == self.base.selected_index {
                    Color::White
                } else {
                    self.get_notification_color(&notification.reason)
                })
                .bg(if i == self.base.selected_index {
                    Color::DarkGray
                } else {
                    Color::Reset
                });

            // Main notification text
            buf.set_string(
                notification_area.x + 1, // Add padding
                notification_area.y,
                formatted,
                content_style
            );

            // Add unread indicator
            if !notification.is_read {
                buf.set_string(
                    notification_area.x + 1,
                    notification_area.y + 1,
                    "● New",
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(if i == self.base.selected_index {
                            Color::DarkGray
                        } else {
                            Color::Reset
                        })
                );
            }

            current_y = current_y.saturating_add(height);
        }
    }
}
// Update ViewStack implementation to include notifications view state
impl ViewStack {
    pub fn push_notifications_view(&mut self) {
        let notifications = NotificationView::new(Arc::clone(&self.image_manager));
        let notifications_view = View::Notifications(notifications);
        self.views.push(notifications_view);
    }
}
