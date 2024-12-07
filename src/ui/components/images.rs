use anyhow::Result;
use atrium_api::app::bsky::embed::images::ViewImage;
use image::DynamicImage;
use image::load_from_memory;
use log::info;
use lru::LruCache;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{self, Style};
use ratatui::widgets::{Block, Borders, Widget};
use ratatui_image::{protocol, Image};
use reqwest;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Hash, PartialEq, Eq)]
pub struct SixelCacheKey {
    url: String,
    width: u16,
    height: u16,
}

impl SixelCacheKey {
    fn new(url: String, area: Rect) -> Self {
        Self {
            url,
            width: area.width,
            height: area.height,
        }
    }
}

pub struct SixelCache {
    cache: LruCache<SixelCacheKey, protocol::sixel::Sixel>,
}

impl SixelCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(50.try_into().unwrap()),
        }
    }

    pub fn get(
        &mut self,
        cache_key: &SixelCacheKey,
    ) -> Option<&ratatui_image::protocol::sixel::Sixel> {
        self.cache.get(cache_key)
    }

    pub fn contains(&self, cache_key: &SixelCacheKey) -> bool {
        self.cache.peek(cache_key).is_some()
    }

    pub fn insert(
        &mut self,
        cache_key: SixelCacheKey,
        data: ratatui_image::protocol::sixel::Sixel,
    ) {
        self.cache.put(cache_key, data);
    }
}

pub type SharedSixelCache = Arc<RwLock<SixelCache>>;

// Global image cache
pub struct ImageCache {
    cache: LruCache<String, Vec<u8>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(200.try_into().unwrap()),
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

// Cache for decoded images
pub struct DecodedImageCache {
    cache: LruCache<String, DynamicImage>,
}

impl DecodedImageCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(100.try_into().unwrap()),
        }
    }

    pub fn get(&mut self, url: &str) -> Option<&DynamicImage> {
        self.cache.get(url)
    }

    pub fn insert(&mut self, url: String, image: DynamicImage) {
        self.cache.put(url, image);
    }
}

// Thread-safe wrapper
pub type SharedDecodedImageCache = Arc<RwLock<DecodedImageCache>>;

// Image downloader/manager
pub struct ImageManager {
    client: reqwest::Client,
    pub raw_cache: SharedImageCache,
    pub decoded_cache: SharedDecodedImageCache,
    pub sixel_cache: SharedSixelCache,
    picker: ratatui_image::picker::Picker,
}

impl ImageManager {
    pub fn new() -> Self {
        let mut picker = ratatui_image::picker::Picker::from_query_stdio()
            .unwrap_or_else(|_| ratatui_image::picker::Picker::from_fontsize((16, 32)));

        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Sixel);
        picker.set_background_color(Some(image::Rgb::<u8>([0, 0, 0])));

        Self {
            client: reqwest::Client::new(),
            raw_cache: Arc::new(RwLock::new(ImageCache::new())),
            decoded_cache: Arc::new(RwLock::new(DecodedImageCache::new())),
            sixel_cache: Arc::new(RwLock::new(SixelCache::new())),
            picker,
        }
    }

    // get_image for downloading
    pub async fn get_image(&self, url: &str) -> Result<Vec<u8>> {
        {
            let mut cache = self.raw_cache.write().await;
            if let Some(data) = cache.get(url) {
                return Ok(data.clone());
            }
        }

        let response = self.client.get(url).send().await?;
        let image_data = response.bytes().await?.to_vec();

        self.raw_cache
            .write()
            .await
            .insert(url.to_string(), image_data.clone());

        Ok(image_data)
    }

    pub fn get_or_create_sixel(&self, url: &str, area: Rect) -> Option<protocol::sixel::Sixel> {
        let key = SixelCacheKey::new(url.to_string(), area);

        // Try cache first
        if let Ok(mut cache) = self.sixel_cache.try_write() {
            if let Some(sixel) = cache.get(&key).cloned() {
                return Some(sixel);
            }
        }

        // Check if we have a decoded image
        if let Ok(mut cache) = self.decoded_cache.try_write() {
            if let Some(decoded) = cache.get(url).cloned() {
                let sixel_cache = self.sixel_cache.clone();
                let font_size = self.picker.font_size();

                tokio::spawn(async move {
                    // Create a new picker with same settings
                    let mut picker = ratatui_image::picker::Picker::from_fontsize(font_size);
                    picker.set_protocol_type(ratatui_image::picker::ProtocolType::Sixel);
                    picker.set_background_color(Some(image::Rgb::<u8>([0, 0, 0])));

                    match picker.new_protocol(decoded, area, ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Triangle))) {
                        Ok(protocol) => {
                            if let protocol::Protocol::Sixel(sixel) = protocol {
                                if let Ok(mut cache) = sixel_cache.try_write() {
                                    cache.insert(key, sixel);
                                }
                            }
                        }
                        Err(e) => info!("Failed to create protocol: {:?}", e),
                    }
                });
            }
        }

        None
    }

    pub async fn get_decoded_image(&self, url: &str) -> Result<Option<DynamicImage>> {
        // Check decoded cache first
        if let Some(decoded) = self.decoded_cache.write().await.get(url) {
            return Ok(Some(decoded.clone()));
        }

        // If not in decoded cache, try to load and decode
        if let Ok(raw_data) = self.get_image(url).await {
            if let Ok(decoded) = load_from_memory(&raw_data) {
                info!("Successfully decoded image for {}", url);
                self.decoded_cache
                    .write()
                    .await
                    .insert(url.to_string(), decoded.clone());
                return Ok(Some(decoded));
            }
        }

        info!("Failed to load/decode image for {}", url);
        Ok(None)
    }
}

pub struct PostImage {
    pub image_data: ViewImage,
    pub show_alt_text: bool,
    pub image_manager: Arc<ImageManager>,
    cached_decoded_image: Option<DynamicImage>,
}

impl PostImage {
    pub fn new(image_data: ViewImage, image_manager: Arc<ImageManager>) -> Self {
        let cached_decoded_image = {
            if let Ok(mut cache) = image_manager.decoded_cache.try_write() {
                cache.get(&image_data.thumb).cloned()
            } else {
                None
            }
        };

        if cached_decoded_image.is_none() {
            let image_manager_clone = image_manager.clone();
            let thumb = image_data.thumb.clone();

            tokio::spawn(async move {
                if let Ok(Some(decoded)) = image_manager_clone.get_decoded_image(&thumb).await {
                    if let Ok(mut cache) = image_manager_clone.decoded_cache.try_write() {
                        cache.insert(thumb, decoded);
                    }
                }
            });
        }

        Self {
            image_data,
            show_alt_text: false,
            image_manager,
            cached_decoded_image,
        }
    }

    pub fn update_cache(&mut self, image: DynamicImage) {
        self.cached_decoded_image = Some(image);
    }

    pub fn get_alt_text(&self) -> Option<&str> {
        if self.image_data.alt.is_empty() {
            None
        } else {
            Some(&self.image_data.alt)
        }
    }
}

use ratatui::layout::{Constraint, Direction, Layout};

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

impl Widget for &mut PostImage {
    fn render(self, area: Rect, buf: &mut Buffer) {
        log::info!("PostImage render called with area: {:?}", area);
        if area.height == 0 {
            log::info!("Area height is 0, returning early");
            return;
        }

        buf.set_style(area, Style::default());

        let block = Block::default().borders(Borders::ALL).title("Image");

        let inner_area = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Image
                Constraint::Percentage(50), // Alt text
            ])
            .split(inner_area);

        block.render(area, buf);

        let image_chunk = chunks[0];
        let alt_text_chunk = chunks[1];

        // Alt text using Paragraph widget for automatic wrapping
        let alt_text = self.get_alt_text().unwrap_or("No alt text provided");
        let alt_text_content = vec![
            Line::from(Span::styled("📷", Style::default().fg(style::Color::Gray))),
            Line::from(Span::styled(
                alt_text,
                Style::default().fg(style::Color::Gray),
            )),
        ];
        Paragraph::new(alt_text_content)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(alt_text_chunk, buf);

        // Try to get cached Sixel
        if let Some(sixel) = self
            .image_manager
            .get_or_create_sixel(&self.image_data.thumb, image_chunk)
        {

            let protocol = protocol::Protocol::Sixel(sixel);

            Image::new(&protocol).render(image_chunk, buf);
        } else {
            // Loading indicator
            buf.set_string(
                image_chunk.x,
                image_chunk.y,
                "Loading image...",
                Style::default().fg(style::Color::DarkGray),
            );
        }
    }
}
