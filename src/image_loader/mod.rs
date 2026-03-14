//! Async image loading with GPU texture management and rolling cache.

mod decode;
pub(crate) mod exif;
mod scan;

pub use scan::scan_image_paths;

use camino::{Utf8Path, Utf8PathBuf};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};

pub(crate) use decode::fast_resize;
use decode::load_image_mips;
use exif::extract_exr_fps;
pub(crate) use exif::{apply_orientation, read_exif_orientation};

/// Maximum number of concurrent loading tasks
const MAX_CONCURRENT_TASKS: usize = 4;

pub struct LoadedTexture {
    #[allow(dead_code)] // Kept alive to prevent GPU texture deallocation
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    /// True when the texture contains HDR content (e.g. EXR in linear light).
    pub is_hdr_content: bool,
}

/// Mip-chain data returned from the loader thread to the GPU upload path.
pub enum MipData {
    /// Standard 8-bit sRGB pixels (one element per mip level).
    Sdr(Vec<image::RgbaImage>),
    /// Linear 32-bit float pixels for HDR EXR files (one element per mip level).
    Hdr(Vec<image::Rgba32FImage>),
}

pub struct TextureManager {
    pub paths: Vec<Utf8PathBuf>,
    pub current_index: usize,
    pub textures: HashMap<usize, LoadedTexture>,
    pub max_texture_size: (u32, u32),
    pub cache_extent: usize,
    /// When true, EXR files are loaded as linear Rgba16Float GPU textures.
    pub is_hdr: bool,

    // Original sort order for restoring when shuffle is turned off
    original_paths: Vec<Utf8PathBuf>,

    // Async loading (sends mip chain: Vec[0]=base, Vec[1]=LOD1, ...)
    loading_tasks: HashMap<usize, u64>,
    errors: HashMap<usize, String>,
    // Incremented on every replace_paths / set_shuffle_enabled call so that
    // results from threads spawned in a previous generation are discarded.
    epoch: u64,
    tx: Sender<(u64, usize, anyhow::Result<MipData>)>,
    rx: Receiver<(u64, usize, anyhow::Result<MipData>)>,
}

impl TextureManager {
    pub fn new(cache_extent: usize, max_texture_size: (u32, u32)) -> Self {
        let (tx, rx) = channel();
        Self {
            paths: Vec::new(),
            current_index: 0,
            textures: HashMap::new(),
            max_texture_size,
            cache_extent,
            is_hdr: false,
            original_paths: Vec::new(),
            loading_tasks: HashMap::new(),
            errors: HashMap::new(),
            epoch: 0,
            tx,
            rx,
        }
    }

    pub fn scan_paths(
        &mut self,
        input_paths: &[Utf8PathBuf],
        scan_subfolders: bool,
    ) -> crate::error::Result<()> {
        let sorted_paths = scan_image_paths(input_paths, scan_subfolders)?;
        self.original_paths = sorted_paths.clone();
        self.paths = sorted_paths;
        info!("Scanned {} images", self.paths.len());
        Ok(())
    }

    pub fn shuffle_paths(&mut self) {
        use rand::seq::SliceRandom;
        let mut rng = rand::rng();
        self.paths.shuffle(&mut rng);
    }

    /// Toggle shuffle on/off, reordering paths accordingly.
    /// Returns the new index that points to the same image as before.
    pub fn set_shuffle_enabled(&mut self, enabled: bool) -> usize {
        if self.paths.is_empty() {
            return 0;
        }

        let current_path = self.paths[self.current_index].clone();

        // Preserve current texture if loaded
        let current_texture = self.textures.remove(&self.current_index);

        if enabled {
            // Shuffle paths and remap current_index
            self.shuffle_paths();
        } else {
            // Restore original sorted order
            self.paths = self.original_paths.clone();
        }

        // Find the current image in the reordered list
        let new_index = self
            .paths
            .iter()
            .position(|p| p == &current_path)
            .unwrap_or(0);

        // Invalidate texture cache since indices changed, but keep current.
        // Bump epoch so any in-flight thread results from the old ordering
        // are discarded when they arrive in update().
        self.epoch = self.epoch.wrapping_add(1);
        self.textures.clear();
        self.loading_tasks.clear();
        self.errors.clear();
        while self.rx.try_recv().is_ok() {}

        // Restore preserved texture at new index
        if let Some(texture) = current_texture {
            self.textures.insert(new_index, texture);
        }

        self.current_index = new_index;
        new_index
    }

    pub fn next(&mut self, pause_at_last: bool) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        if self.current_index + 1 < self.paths.len() {
            self.current_index += 1;
            true
        } else if !pause_at_last {
            self.current_index = 0;
            true
        } else {
            false
        }
    }

    pub fn previous(&mut self, pause_at_last: bool) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        if self.current_index > 0 {
            self.current_index -= 1;
        } else if !pause_at_last {
            self.current_index = self.paths.len() - 1;
        } else {
            return false;
        }
        true
    }

    pub fn jump_to(&mut self, index: usize) {
        if index < self.paths.len() {
            self.current_index = index;
        }
    }

    /// Replace the entire image list, clearing all cached textures and pending loads.
    pub fn replace_paths(&mut self, new_paths: Vec<Utf8PathBuf>) {
        self.original_paths = new_paths.clone();
        self.paths = new_paths;
        // Bump epoch so any in-flight thread results from the previous path
        // list are discarded when they arrive in update().
        self.epoch = self.epoch.wrapping_add(1);
        self.textures.clear();
        self.loading_tasks.clear();
        self.errors.clear();
        self.current_index = 0;
        while self.rx.try_recv().is_ok() {}
    }

    /// Append paths to the existing image list, preserving the current index and loaded textures.
    pub fn append_paths(&mut self, new_paths: Vec<Utf8PathBuf>) {
        if self.paths.is_empty() {
            self.replace_paths(new_paths);
            return;
        }
        self.original_paths.extend(new_paths.clone());
        self.paths.extend(new_paths);
        // Do not bump epoch or clear existing textures because the existing indices remain valid.
        // The new images will naturally be fetched when navigated to.
    }

    /// Detect framerate from EXR metadata if available.
    /// Returns the FPS from the first EXR file found in the path list.
    pub fn detect_sequence_fps(&self) -> Option<f32> {
        for path in &self.paths {
            if path.extension().unwrap_or("").eq_ignore_ascii_case("exr") {
                if let Some(fps) = extract_exr_fps(path) {
                    return Some(fps);
                }
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn current_path(&self) -> Option<&Utf8Path> {
        self.paths.get(self.current_index).map(|p| p.as_path())
    }

    pub fn get_current_texture(&self) -> Option<&LoadedTexture> {
        self.textures.get(&self.current_index)
    }

    pub fn get_texture(&self, index: usize) -> Option<&LoadedTexture> {
        self.textures.get(&index)
    }

    pub fn get_error(&self, index: usize) -> Option<&String> {
        self.errors.get(&index)
    }

    /// Returns `true` while any texture loads are still in progress.
    pub fn is_loading(&self) -> bool {
        !self.loading_tasks.is_empty()
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.paths.is_empty() {
            return;
        }

        // 1. Process received images and upload to GPU
        while let Ok((msg_epoch, idx, result)) = self.rx.try_recv() {
            // Discard results from threads spawned before the last path reorder.
            // Only remove from loading_tasks if the epoch matches the spawned entry,
            // so that stale messages don't evict a newer task for the same slot.
            if msg_epoch != self.epoch {
                continue;
            }
            if self.loading_tasks.get(&idx) == Some(&msg_epoch) {
                self.loading_tasks.remove(&idx);
            }
            match result {
                Ok(mip_data) => {
                    match mip_data {
                        MipData::Sdr(mips) => {
                            let Some(_base) = mips.first() else {
                                error!("Image {} returned empty SDR mip chain", idx);
                                self.errors.insert(idx, "empty mip chain".to_string());
                                continue;
                            };
                            // SDR: 4 channels × 1 byte (u8), uploaded as-is
                            let mip_iter = mips.iter().map(|mip| {
                                (
                                    mip.width(),
                                    mip.height(),
                                    4 * mip.width(),
                                    mip.as_raw().clone(),
                                )
                            });
                            self.upload_mip_chain(
                                device,
                                queue,
                                idx,
                                wgpu::TextureFormat::Rgba8UnormSrgb,
                                "SDR",
                                false,
                                mip_iter,
                            );
                        }
                        MipData::Hdr(mips) => {
                            let Some(_base) = mips.first() else {
                                error!("Image {} returned empty HDR mip chain", idx);
                                self.errors.insert(idx, "empty mip chain".to_string());
                                continue;
                            };
                            // HDR: convert f32 → f16; 4 channels × 2 bytes per pixel
                            let mip_iter = mips.iter().map(|mip| {
                                let f16_bytes: Vec<u8> = mip
                                    .as_raw()
                                    .iter()
                                    .flat_map(|&f| half::f16::from_f32(f).to_ne_bytes())
                                    .collect();
                                (mip.width(), mip.height(), 8 * mip.width(), f16_bytes)
                            });
                            self.upload_mip_chain(
                                device,
                                queue,
                                idx,
                                wgpu::TextureFormat::Rgba16Float,
                                "HDR",
                                true,
                                mip_iter,
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load image {}: {}", idx, e);
                    self.errors.insert(idx, e.to_string());
                }
            }
        }

        // 2. Manage cache and start new tasks
        let mut needed_indices = HashSet::new();
        needed_indices.insert(self.current_index);

        let len = self.paths.len();
        let extent = self.cache_extent.min(len.saturating_sub(1));
        for i in 1..=extent {
            needed_indices.insert((self.current_index + i) % len);
            needed_indices.insert((self.current_index + len - i) % len);
        }

        self.textures.retain(|idx, _| needed_indices.contains(idx));
        self.errors.retain(|idx, _| needed_indices.contains(idx));
        self.loading_tasks
            .retain(|idx, _| needed_indices.contains(idx));

        for idx in needed_indices {
            if !self.textures.contains_key(&idx)
                && !self.errors.contains_key(&idx)
                && !self.loading_tasks.contains_key(&idx)
            {
                if self.loading_tasks.len() >= MAX_CONCURRENT_TASKS {
                    break;
                }

                if let Some(path) = self.paths.get(idx).cloned() {
                    let tx = self.tx.clone();
                    let max_size = self.max_texture_size;
                    let epoch = self.epoch;
                    let is_hdr = self.is_hdr;

                    self.loading_tasks.insert(idx, self.epoch);

                    rayon::spawn(move || {
                        let res =
                            std::panic::catch_unwind(|| load_image_mips(&path, max_size, is_hdr))
                                .unwrap_or_else(|payload| {
                                    let msg = if let Some(s) = payload.downcast_ref::<String>() {
                                        s.clone()
                                    } else if let Some(s) = payload.downcast_ref::<&str>() {
                                        (*s).to_owned()
                                    } else {
                                        "unknown panic in image loader thread".to_owned()
                                    };
                                    error!(
                                        "Image loader thread panicked for index {}: {}",
                                        idx, msg
                                    );
                                    Err(anyhow::anyhow!("loader thread panicked: {}", msg))
                                });
                        if tx.send((epoch, idx, res)).is_err() {
                            warn!("Failed to send loaded image {} (receiver dropped)", idx);
                        }
                    });
                }
            }
        }
    }

    /// Upload a mip chain to the GPU and insert the resulting [`LoadedTexture`] into the cache.
    ///
    /// `mips` yields `(mip_width, mip_height, bytes_per_row, pixel_bytes)` for each mip level
    /// in ascending order (level 0 first).  The `kind` string (`"SDR"` or `"HDR"`) is used for
    /// the wgpu texture label and the debug log line.
    // All parameters are required to upload a mip chain; splitting into smaller
    // functions would add indirection without reducing the argument surface.
    #[allow(clippy::too_many_arguments)]
    fn upload_mip_chain(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        idx: usize,
        format: wgpu::TextureFormat,
        kind: &str,
        is_hdr_content: bool,
        mips: impl Iterator<Item = (u32, u32, u32, Vec<u8>)>,
    ) {
        let mips: Vec<(u32, u32, u32, Vec<u8>)> = mips.collect();
        let mip_count = mips.len();

        // Base dimensions come from the first (largest) mip level.
        let (width, height) = mips.first().map_or((0, 0), |&(w, h, _, _)| (w, h));

        let texture_label = if kind == "SDR" {
            format!("Image Texture {idx}")
        } else {
            format!("Image Texture {idx} ({kind})")
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&texture_label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, (mip_w, mip_h, bytes_per_row, data)) in mips.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(*bytes_per_row),
                    rows_per_image: Some(*mip_h),
                },
                wgpu::Extent3d {
                    width: *mip_w,
                    height: *mip_h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.textures.insert(
            idx,
            LoadedTexture {
                texture,
                view,
                width,
                height,
                is_hdr_content,
            },
        );
        debug!("Uploaded {kind} image {idx} ({width}x{height}, {mip_count} mips)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager(paths: &[&str]) -> TextureManager {
        let mut mgr = TextureManager::new(2, (1920, 1080));
        mgr.paths = paths.iter().map(|&s| Utf8PathBuf::from(s)).collect();
        mgr.original_paths = mgr.paths.clone();
        mgr
    }

    // --- next() ---

    #[test]
    fn next_advances_index() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg", "c.jpg"]);
        assert!(mgr.next(false));
        assert_eq!(mgr.current_index, 1);
    }

    #[test]
    fn next_wraps_to_start_when_not_pause_at_last() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg"]);
        mgr.current_index = 1;
        assert!(mgr.next(false));
        assert_eq!(mgr.current_index, 0);
    }

    #[test]
    fn next_returns_false_and_stays_at_last_when_pause_at_last() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg"]);
        mgr.current_index = 1;
        assert!(!mgr.next(true));
        assert_eq!(mgr.current_index, 1);
    }

    #[test]
    fn next_returns_false_on_empty() {
        let mut mgr = TextureManager::new(2, (1920, 1080));
        assert!(!mgr.next(false));
    }

    // --- previous() ---

    #[test]
    fn previous_decrements_index() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg", "c.jpg"]);
        mgr.current_index = 2;
        assert!(mgr.previous(false));
        assert_eq!(mgr.current_index, 1);
    }

    #[test]
    fn previous_wraps_to_last() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg", "c.jpg"]);
        mgr.current_index = 0;
        assert!(mgr.previous(false));
        assert_eq!(mgr.current_index, 2);
    }

    #[test]
    fn previous_returns_false_and_stays_at_first_when_pause_at_last() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg", "c.jpg"]);
        mgr.current_index = 0;
        assert!(!mgr.previous(true));
        assert_eq!(mgr.current_index, 0);
    }

    #[test]
    fn previous_returns_false_on_empty() {
        let mut mgr = TextureManager::new(2, (1920, 1080));
        assert!(!mgr.previous(false));
    }

    // --- jump_to() ---

    #[test]
    fn jump_to_valid_index() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg", "c.jpg"]);
        mgr.jump_to(2);
        assert_eq!(mgr.current_index, 2);
    }

    #[test]
    fn jump_to_out_of_bounds_is_ignored() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg"]);
        mgr.jump_to(99);
        assert_eq!(mgr.current_index, 0);
    }

    // --- replace_paths() / append_paths() ---

    #[test]
    fn replace_paths_resets_to_index_zero() {
        let mut mgr = make_manager(&["a.jpg", "b.jpg"]);
        mgr.current_index = 1;
        mgr.replace_paths(vec![Utf8PathBuf::from("c.jpg")]);
        assert_eq!(mgr.current_index, 0);
        assert_eq!(mgr.paths.len(), 1);
        assert!(mgr.textures.is_empty());
    }

    #[test]
    fn append_paths_extends_list_and_preserves_index() {
        let mut mgr = make_manager(&["a.jpg"]);
        mgr.append_paths(vec![Utf8PathBuf::from("b.jpg"), Utf8PathBuf::from("c.jpg")]);
        assert_eq!(mgr.paths.len(), 3);
        assert_eq!(mgr.current_index, 0);
    }

    #[test]
    fn append_paths_to_empty_behaves_like_replace() {
        let mut mgr = TextureManager::new(2, (1920, 1080));
        mgr.append_paths(vec![Utf8PathBuf::from("a.jpg"), Utf8PathBuf::from("b.jpg")]);
        assert_eq!(mgr.paths.len(), 2);
        assert_eq!(mgr.current_index, 0);
    }
}
