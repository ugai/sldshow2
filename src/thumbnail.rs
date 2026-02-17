//! Thumbnail generation and caching for gallery view.
//!
//! Generates 256x256 thumbnails lazily (on demand) using async rayon workers.
//! Uses an LRU cache with bounded memory to prevent unlimited growth.
//!
//! This module provides infrastructure for the future gallery view (issue #45).
//! Currently unused but ready for integration.

use camino::{Utf8Path, Utf8PathBuf};
use image::{GenericImageView, RgbaImage};
use log::{debug, warn};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::image_loader::apply_exif_rotation;

/// Fixed thumbnail dimensions (square, aspect-preserving)
const THUMBNAIL_SIZE: u32 = 256;

/// Maximum number of concurrent thumbnail generation tasks
#[allow(dead_code)]
const MAX_CONCURRENT_GENERATION: usize = 4;

#[allow(dead_code)]
pub struct ThumbnailManager {
    /// In-memory cache: index → thumbnail image
    cache: HashMap<usize, RgbaImage>,
    /// LRU queue for eviction (front = oldest, back = newest)
    lru_order: VecDeque<usize>,
    /// Maximum cache size (number of thumbnails)
    max_cache_size: usize,

    /// Async generation tracking
    loading_tasks: HashSet<usize>,
    /// Queue of requests waiting for a free concurrent slot
    pending_queue: VecDeque<(usize, Utf8PathBuf)>,
    tx: Sender<(usize, anyhow::Result<RgbaImage>)>,
    rx: Receiver<(usize, anyhow::Result<RgbaImage>)>,
}

#[allow(dead_code)]
impl ThumbnailManager {
    /// Create a new thumbnail manager with bounded cache size.
    pub fn new(max_cache_size: usize) -> Self {
        assert!(max_cache_size > 0, "max_cache_size must be greater than 0");
        let (tx, rx) = channel();
        Self {
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
            max_cache_size,
            loading_tasks: HashSet::new(),
            pending_queue: VecDeque::new(),
            tx,
            rx,
        }
    }

    /// Request thumbnail generation for a specific index.
    /// Returns immediately; call `update()` to process completed thumbnails.
    pub fn request_thumbnail(&mut self, index: usize, path: &Utf8Path) {
        // Already cached or loading or pending
        if self.cache.contains_key(&index)
            || self.loading_tasks.contains(&index)
            || self.pending_queue.iter().any(|(i, _)| *i == index)
        {
            return;
        }

        // Enforce concurrency limit — queue if at capacity
        if self.loading_tasks.len() >= MAX_CONCURRENT_GENERATION {
            self.pending_queue.push_back((index, path.to_owned()));
            return;
        }

        self.spawn_generation(index, path.to_owned());
    }

    /// Spawn a thumbnail generation task on a background thread.
    fn spawn_generation(&mut self, index: usize, path: Utf8PathBuf) {
        let tx = self.tx.clone();

        self.loading_tasks.insert(index);

        std::thread::spawn(move || {
            let result = generate_thumbnail(&path);
            if tx.send((index, result)).is_err() {
                warn!("Failed to send thumbnail {} (receiver dropped)", path);
            }
        });
    }

    /// Process completed thumbnail generation tasks.
    /// Call this from the main event loop.
    pub fn update(&mut self) {
        while let Ok((index, result)) = self.rx.try_recv() {
            self.loading_tasks.remove(&index);

            match result {
                Ok(thumbnail) => {
                    // Remove from lru_order if already present (re-generation case)
                    self.lru_order.retain(|&i| i != index);

                    // Evict LRU entry if cache is full
                    if self.cache.len() >= self.max_cache_size {
                        if let Some(evict_index) = self.lru_order.pop_front() {
                            self.cache.remove(&evict_index);
                            debug!("Evicted thumbnail {} from cache", evict_index);
                        }
                    }

                    self.cache.insert(index, thumbnail);
                    self.lru_order.push_back(index);
                    debug!("Cached thumbnail {}", index);
                    debug_assert_eq!(self.cache.len(), self.lru_order.len());
                }
                Err(e) => {
                    warn!("Failed to generate thumbnail {}: {}", index, e);
                }
            }
        }

        // Drain pending queue to start new tasks up to the concurrency limit
        while self.loading_tasks.len() < MAX_CONCURRENT_GENERATION {
            match self.pending_queue.pop_front() {
                Some((index, path)) => {
                    // Skip if already cached or already loading in the meantime
                    if self.cache.contains_key(&index) || self.loading_tasks.contains(&index) {
                        continue;
                    }
                    self.spawn_generation(index, path);
                }
                None => break,
            }
        }
    }

    /// Retrieve a cached thumbnail. Returns None if not yet generated.
    /// Marks the entry as recently used (LRU).
    pub fn get_thumbnail(&mut self, index: usize) -> Option<&RgbaImage> {
        if self.cache.contains_key(&index) {
            // Move to back of LRU queue (most recently used)
            self.lru_order.retain(|&i| i != index);
            self.lru_order.push_back(index);
            debug_assert_eq!(self.cache.len(), self.lru_order.len());
        }
        self.cache.get(&index)
    }

    /// Clear all cached thumbnails and cancel pending tasks.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_order.clear();
        self.loading_tasks.clear();
        self.pending_queue.clear();
        // Recreate channel so old threads' tx handles are orphaned;
        // their sends will silently fail without leaking loading_tasks.
        let (tx, rx) = channel();
        self.tx = tx;
        self.rx = rx;
    }

    /// Returns the number of cached thumbnails.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Returns the number of thumbnails currently being generated.
    pub fn pending_count(&self) -> usize {
        self.loading_tasks.len()
    }

    /// Return a list of all currently cached thumbnail indices.
    pub fn get_cached_indices(&self) -> Vec<usize> {
        self.cache.keys().cloned().collect()
    }
}

/// Generate a 256x256 thumbnail from an image file.
/// Preserves aspect ratio with letterboxing.
#[allow(dead_code)]
fn generate_thumbnail(path: &Utf8Path) -> anyhow::Result<RgbaImage> {
    let img = image::open(path.as_std_path())
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;

    // Apply EXIF rotation (shared with image_loader)
    let img = apply_exif_rotation(img, path);

    // Resize to fit within 256x256, preserving aspect ratio
    let (orig_w, orig_h) = img.dimensions();
    let scale = (THUMBNAIL_SIZE as f32 / orig_w as f32).min(THUMBNAIL_SIZE as f32 / orig_h as f32);

    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);

    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

    // Letterbox to exact 256x256 with centered content
    let mut thumbnail = RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let offset_x = (THUMBNAIL_SIZE - new_w) / 2;
    let offset_y = (THUMBNAIL_SIZE - new_h) / 2;

    image::imageops::overlay(
        &mut thumbnail,
        &resized.to_rgba8(),
        offset_x.into(),
        offset_y.into(),
    );

    Ok(thumbnail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_manager_creation() {
        let manager = ThumbnailManager::new(100);
        assert_eq!(manager.cache_size(), 0);
        assert_eq!(manager.pending_count(), 0);
    }

    #[test]
    fn test_thumbnail_manager_clear() {
        let mut manager = ThumbnailManager::new(100);
        manager.cache.insert(0, RgbaImage::new(256, 256));
        manager.lru_order.push_back(0);
        manager.loading_tasks.insert(1);

        manager.clear();

        assert_eq!(manager.cache_size(), 0);
        assert_eq!(manager.pending_count(), 0);
    }

    #[test]
    #[should_panic(expected = "max_cache_size must be greater than 0")]
    fn test_thumbnail_manager_zero_cache_size() {
        ThumbnailManager::new(0);
    }
}
