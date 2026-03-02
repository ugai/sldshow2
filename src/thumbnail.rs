//! Thumbnail generation and caching for gallery view.
//!
//! Generates 256x256 thumbnails lazily (on demand) using async rayon workers.
//! Uses an LRU cache with bounded memory to prevent unlimited growth.
//!
//! Used by the gallery flow to generate and cache thumbnails asynchronously for
//! the overlay gallery view.

use camino::{Utf8Path, Utf8PathBuf};
use image::{GenericImageView, RgbaImage};
use log::{debug, warn};
use lru::LruCache;
use std::collections::{HashSet, VecDeque};
use std::num::NonZeroUsize;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::image_loader::{apply_exif_rotation, fast_resize};

/// Fixed thumbnail dimensions (square, aspect-preserving)
const THUMBNAIL_SIZE: u32 = 256;

/// Maximum number of concurrent thumbnail generation tasks
#[allow(dead_code)]
const MAX_CONCURRENT_GENERATION: usize = 4;

#[allow(dead_code)]
pub struct ThumbnailManager {
    /// LRU cache: index → thumbnail image (O(1) access and eviction)
    cache: LruCache<usize, RgbaImage>,

    /// Async generation tracking
    loading_tasks: HashSet<usize>,
    /// Queue of requests waiting for a free concurrent slot
    pending_queue: VecDeque<(usize, Utf8PathBuf)>,
    tx: Sender<(usize, anyhow::Result<RgbaImage>)>,
    rx: Receiver<(usize, anyhow::Result<RgbaImage>)>,

    /// Indices of thumbnails that were newly inserted into the cache since the
    /// last call to `drain_newly_cached()`. Used by the overlay to invalidate
    /// stale egui texture handles after a thumbnail is re-generated.
    newly_cached: Vec<usize>,
}

#[allow(dead_code)]
impl ThumbnailManager {
    /// Create a new thumbnail manager with bounded cache size.
    pub fn new(max_cache_size: usize) -> Self {
        assert!(max_cache_size > 0, "max_cache_size must be greater than 0");
        let cap = NonZeroUsize::new(max_cache_size).expect("max_cache_size must be greater than 0");
        let (tx, rx) = channel();
        Self {
            cache: LruCache::new(cap),
            loading_tasks: HashSet::new(),
            pending_queue: VecDeque::new(),
            tx,
            rx,
            newly_cached: Vec::new(),
        }
    }

    /// Request thumbnail generation for a specific index.
    /// Returns immediately; call `update()` to process completed thumbnails.
    pub fn request_thumbnail(&mut self, index: usize, path: &Utf8Path) {
        // Already cached or loading or pending
        if self.cache.contains(&index)
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

    /// Spawn a thumbnail generation task on a rayon thread-pool worker.
    fn spawn_generation(&mut self, index: usize, path: Utf8PathBuf) {
        let tx = self.tx.clone();

        self.loading_tasks.insert(index);

        rayon::spawn(move || {
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
                    // put() inserts and promotes to MRU; evicts LRU entry automatically if full.
                    if let Some((evict_index, _)) = self.cache.push(index, thumbnail) {
                        debug!("Evicted thumbnail {} from cache", evict_index);
                    }
                    debug!("Cached thumbnail {}", index);
                    self.newly_cached.push(index);
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
                    if self.cache.contains(&index) || self.loading_tasks.contains(&index) {
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
        self.cache.get(&index)
    }

    /// Clear all cached thumbnails and cancel pending tasks.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.loading_tasks.clear();
        self.pending_queue.clear();
        self.newly_cached.clear();
        // Recreate channel so old threads' tx handles are orphaned;
        // their sends will silently fail without leaking loading_tasks.
        let (tx, rx) = channel();
        self.tx = tx;
        self.rx = rx;
    }

    /// Clear only the pending queue.
    /// Used to reset priorities when the requested set changes (e.g. rapid scrolling).
    pub fn clear_pending(&mut self) {
        self.pending_queue.clear();
    }

    /// Return and clear the list of indices whose thumbnails were newly inserted
    /// into the cache since the last call to this method.
    ///
    /// Call this each frame to invalidate stale egui texture handles so the gallery
    /// view displays the latest thumbnail data after a re-generation.
    pub fn drain_newly_cached(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.newly_cached)
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
        self.cache.iter().map(|(&k, _)| k).collect()
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

    let mut rgba_img = img.into_rgba8();
    let src_image = fast_image_resize::images::Image::from_slice_u8(
        orig_w,
        orig_h,
        rgba_img.as_mut(),
        fast_image_resize::PixelType::U8x4,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let resized = fast_resize(
        src_image,
        new_w,
        new_h,
        fast_image_resize::FilterType::Lanczos3,
    )?;

    // Letterbox to exact 256x256 with centered content
    let mut thumbnail = RgbaImage::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let offset_x = (THUMBNAIL_SIZE - new_w) / 2;
    let offset_y = (THUMBNAIL_SIZE - new_h) / 2;

    image::imageops::overlay(&mut thumbnail, &resized, offset_x.into(), offset_y.into());

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
        manager.cache.put(0, RgbaImage::new(256, 256));
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

    #[test]
    fn test_lru_eviction() {
        let mut manager = ThumbnailManager::new(2);
        manager.cache.put(0, RgbaImage::new(256, 256));
        manager.cache.put(1, RgbaImage::new(256, 256));
        // Access 0 to make it MRU; 1 becomes LRU
        manager.get_thumbnail(0);
        // Insert 2 — should evict 1 (LRU)
        manager.cache.put(2, RgbaImage::new(256, 256));
        assert_eq!(manager.cache_size(), 2);
        assert!(manager.get_thumbnail(0).is_some());
        assert!(manager.get_thumbnail(1).is_none());
        assert!(manager.get_thumbnail(2).is_some());
    }
}
