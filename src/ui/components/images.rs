use std::sync::Arc;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{self, Style};
use ratatui::widgets::{Block, Borders, Widget};
use tokio::sync::RwLock;
use atrium_api::app::bsky::embed::images::ViewImage;
use anyhow::Result;
use reqwest;
use lru::LruCache;
use ratatui_image::{Image, protocol};
use image::{load_from_memory, imageops::resize, imageops::FilterType};

const FONT_SIZE: (u16, u16) = (8, 16);

// Global image cache
pub struct ImageCache {
    cache: LruCache<String, Vec<u8>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(200.try_into().unwrap())
        }
    }

    pub fn get(&mut self, url: &str) -> Option<&Vec<u8>> {
        self.cache.get(url)
    }

    pub fn contains(&self, url: &str) -> bool {
        // peek() checks if key exists without updating LRU order
        self.cache.peek(url).is_some()
    }

    pub fn insert(&mut self, url: String, data: Vec<u8>) {
        self.cache.put(url, data);
    }
}
// Thread-safe wrapper for the cache
pub type SharedImageCache = Arc<RwLock<ImageCache>>;

// Image downloader/manager
pub struct ImageManager {
    client: reqwest::Client,
    pub cache: SharedImageCache,
}

impl ImageManager {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(ImageCache::new())),
        }
    }

    pub async fn get_image(&self, url: &str) -> Result<Vec<u8>> {
        {
            let mut cache = self.cache.write().await;
            if let Some(data) = cache.get(url) {
                return Ok(data.clone());
            }
        }

        // Download if not in cache
        let response = self.client.get(url).send().await?;
        let image_data = response.bytes().await?.to_vec();

        // Store in cache
        self.cache.write().await.insert(url.to_string(), image_data.clone());

        Ok(image_data)
    }
}

pub struct PostImage {
    pub image_data: ViewImage,
    pub show_alt_text: bool,
    pub image_manager: Arc<ImageManager>,
    cached_image: Option<Vec<u8>>,
}

impl PostImage {
    pub fn new(image_data: ViewImage, image_manager: Arc<ImageManager>) -> Self {
        let cached_image = {
            if let Ok(mut cache) = image_manager.cache.try_write() {
                cache.get(&image_data.thumb).cloned()
            } else {
                None
            }
        };

        Self {
            image_data,
            show_alt_text: false,
            image_manager,
            cached_image,
        }
    }

    pub fn update_cache(&mut self, data: Vec<u8>) {
        self.cached_image = Some(data);
    }

    pub fn get_alt_text(&self) -> Option<&str> {
        if self.image_data.alt.is_empty() {
            None
        } else {
            Some(&self.image_data.alt)
        }
    }
}

impl Widget for &PostImage {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Skip if no space
        if area.height == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Image");

        let inner_area = block.inner(area);
        block.render(area, buf);

       // Always render alt text first if we have room
       if inner_area.y < area.bottom() {
        buf.set_string(
            inner_area.x,
            inner_area.y,
            format!("📷 {}", self.get_alt_text().unwrap_or(
                "No alt text provided"
            )),
            Style::default().fg(style::Color::Gray)
        );
    }

    // Status line or image rendering
    if let Some(image_data) = &self.cached_image {
        if inner_area.height > 2 {
            let image_area = Rect {
                x: inner_area.x,
                y: inner_area.y + 2,
                width: inner_area.width,
                height: inner_area.height.saturating_sub(2),
            };

            match load_from_memory(image_data) {
                Ok(decoded_image) => {
                    let source = protocol::ImageSource::new(
                        decoded_image,
                        (16, 32),
                    );
                    
                    if let Ok(sixel) = protocol::sixel::Sixel::from_source(
                        &source,
                        (16, 32),
                        ratatui_image::Resize::Fit(None),
                        None,
                        false,  // transparent
                        image_area,
                    ) {
                        let protocol = protocol::Protocol::Sixel(sixel);
                        Image::new(&protocol).render(image_area, buf);
                    }
                }
                Err(_) => {
                    // Only show error on actual decode failure
                    if inner_area.y + 1 < area.bottom() {
                        buf.set_string(
                            inner_area.x,
                            inner_area.y + 1,
                            "Failed to decode image",
                            Style::default().fg(style::Color::Red)
                        );
                    }
                }
            }
        }
        } else {
            // Only show loading when we don't have the image
            if inner_area.y + 1 < area.bottom() {
                buf.set_string(
                    inner_area.x,
                    inner_area.y + 1,
                    "Loading image...",
                    Style::default().fg(style::Color::DarkGray)
                );
            }
        }
    }
}
