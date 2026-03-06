//! Async image loading with GPU texture management and rolling cache.

use crate::error::{Result, SldshowError};
use camino::{Utf8Path, Utf8PathBuf};
use image::GenericImageView;
use log::{debug, error, info, warn};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Maximum number of concurrent loading tasks
const MAX_CONCURRENT_TASKS: usize = 4;

/// Maximum directory recursion depth to prevent infinite loops
const MAX_SCAN_DEPTH: usize = 128;

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

    pub fn scan_paths(&mut self, input_paths: &[Utf8PathBuf], scan_subfolders: bool) -> Result<()> {
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

// Standalone functions

/// Converts a linear light value to sRGB using the IEC 61966-2-1 piecewise transfer function.
/// This is more accurate than the simple gamma 2.2 approximation, especially for near-black values.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// Helper to perform fast resizing using fast_image_resize
pub(crate) fn fast_resize(
    src_img: fast_image_resize::images::Image,
    dst_width: u32,
    dst_height: u32,
    filter: fast_image_resize::FilterType,
) -> anyhow::Result<image::RgbaImage> {
    // Create destination image
    let mut dst_img = fast_image_resize::images::Image::new(
        dst_width,
        dst_height,
        fast_image_resize::PixelType::U8x4,
    );

    // Create resizer
    let mut resizer = fast_image_resize::Resizer::new();
    let resize_opts = fast_image_resize::ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(filter));

    // Resize
    resizer
        .resize(&src_img, &mut dst_img, &resize_opts)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // Convert back to image::RgbaImage
    let buffer = dst_img.into_vec();
    image::RgbaImage::from_raw(dst_width, dst_height, buffer)
        .ok_or_else(|| anyhow::anyhow!("from_raw failed"))
}

fn load_image_mips(path: &Utf8Path, max_size: (u32, u32), is_hdr: bool) -> anyhow::Result<MipData> {
    use std::fs::File;
    use std::io::{BufReader, Seek, SeekFrom};

    let file = File::open(path.as_std_path())
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;
    let mut reader = BufReader::new(file);

    // Read EXIF orientation before decoding so we open the file only once.
    let orientation = read_exif_orientation(&mut reader);
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| anyhow::anyhow!("Failed to seek image: {}", e))?;

    let img = image::ImageReader::new(&mut reader)
        .with_guessed_format()
        .map_err(|e| anyhow::anyhow!("Failed to guess image format: {}", e))?
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;

    let is_exr = path.extension().unwrap_or("").eq_ignore_ascii_case("exr");

    if is_hdr && is_exr {
        // HDR path: keep linear float data as Rgba32F, skip sRGB conversion.
        // GPU upload will convert f32 → f16 for Rgba16Float texture.
        let rgba32f = img.into_rgba32f();
        let img = apply_orientation(image::DynamicImage::ImageRgba32F(rgba32f), orientation);
        let base = resize_for_gpu_hdr(img.into_rgba32f(), max_size.0, max_size.1);

        // Generate mip chain using image::imageops (fast_image_resize is U8 only)
        let mip_count = mip_level_count(base.width(), base.height());
        let mut mips: Vec<image::Rgba32FImage> = Vec::with_capacity(mip_count as usize);
        mips.push(base);

        for _ in 1..mip_count {
            let prev = mips.last().expect("mip chain is non-empty");
            let new_w = (prev.width() / 2).max(1);
            let new_h = (prev.height() / 2).max(1);
            let resized =
                image::imageops::resize(prev, new_w, new_h, image::imageops::FilterType::Triangle);
            mips.push(resized);
        }

        Ok(MipData::Hdr(mips))
    } else {
        // SDR path: existing behavior — tonemap EXR to sRGB, upload as Rgba8UnormSrgb.
        let mut img = img;
        if is_exr {
            // Apply the IEC 61966-2-1 piecewise sRGB transfer function per channel
            let mut rgba32f = img.into_rgba32f();
            for pixel in rgba32f.pixels_mut() {
                pixel[0] = linear_to_srgb(pixel[0].max(0.0));
                pixel[1] = linear_to_srgb(pixel[1].max(0.0));
                pixel[2] = linear_to_srgb(pixel[2].max(0.0));
                // Alpha remains linear
            }
            img = image::DynamicImage::ImageRgba32F(rgba32f);
        }

        let img = apply_orientation(img, orientation);
        let base = resize_for_gpu(img, max_size.0, max_size.1)?.into_rgba8();

        // Generate mipmap chain on CPU
        let mip_count = mip_level_count(base.width(), base.height());
        let mut mips = Vec::with_capacity(mip_count as usize);
        mips.push(base);

        for _ in 1..mip_count {
            let prev = mips
                .last()
                .ok_or_else(|| anyhow::anyhow!("mip chain is empty"))?;
            let new_w = (prev.width() / 2).max(1);
            let new_h = (prev.height() / 2).max(1);

            let mut prev_clone = prev.clone();

            // Fast image resize wrapper creation
            let src_image = fast_image_resize::images::Image::from_slice_u8(
                prev.width(),
                prev.height(),
                prev_clone.as_mut(),
                fast_image_resize::PixelType::U8x4,
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            let resized = fast_resize(
                src_image,
                new_w,
                new_h,
                fast_image_resize::FilterType::Bilinear,
            )?;
            mips.push(resized);
        }

        Ok(MipData::Sdr(mips))
    }
}

fn mip_level_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    max_dim.ilog2() + 1
}

/// Read the EXIF orientation tag from a reader without consuming the whole stream.
pub(crate) fn read_exif_orientation<R: std::io::BufRead + std::io::Seek>(
    reader: &mut R,
) -> Option<u32> {
    exif::Reader::new()
        .read_from_container(reader)
        .ok()?
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

/// Apply a raw EXIF orientation value to an image.
pub(crate) fn apply_orientation(
    img: image::DynamicImage,
    orientation: Option<u32>,
) -> image::DynamicImage {
    match orientation {
        Some(2) => img.fliph(),
        Some(3) => img.rotate180(),
        Some(4) => img.flipv(),
        Some(5) => img.rotate90().fliph(),
        Some(6) => img.rotate90(),
        Some(7) => img.rotate270().fliph(),
        Some(8) => img.rotate270(),
        _ => img,
    }
}

/// Extract framerate from EXR metadata.
/// Returns None if the file is not readable or lacks framesPerSecond attribute.
fn extract_exr_fps(path: &Utf8Path) -> Option<f32> {
    use exr::prelude::*;

    let reader = match read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .all_layers()
        .all_attributes()
        .non_parallel()
        .from_file(path.as_std_path())
    {
        Ok(r) => r,
        Err(_) => return None,
    };

    // Check standard framesPerSecond attribute
    for layer in &reader.layer_data {
        for (name, value) in &layer.attributes.other {
            if name == "framesPerSecond" {
                if let AttributeValue::F32(fps) = value {
                    if *fps > 0.0 && fps.is_finite() {
                        return Some(*fps);
                    }
                }
            }
        }
    }

    None
}

fn resize_for_gpu(
    img: image::DynamicImage,
    max_width: u32,
    max_height: u32,
) -> anyhow::Result<image::DynamicImage> {
    // [0, 0] means "no limit" — upload at full resolution
    if max_width == 0 || max_height == 0 {
        return Ok(img);
    }
    let (orig_w, orig_h) = img.dimensions();
    if orig_w <= max_width && orig_h <= max_height {
        return Ok(img);
    }
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);
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
    Ok(image::DynamicImage::ImageRgba8(resized))
}

/// Resize an HDR (Rgba32F) image to fit within max_width×max_height, preserving aspect ratio.
/// Uses bilinear (Triangle) filter since fast_image_resize does not support float pixels.
fn resize_for_gpu_hdr(
    img: image::Rgba32FImage,
    max_width: u32,
    max_height: u32,
) -> image::Rgba32FImage {
    if max_width == 0 || max_height == 0 {
        return img;
    }
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w <= max_width && orig_h <= max_height {
        return img;
    }
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);
    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle)
}

pub fn scan_image_paths(
    input_paths: &[Utf8PathBuf],
    scan_subfolders: bool,
) -> Result<Vec<Utf8PathBuf>> {
    let mut paths: Vec<Utf8PathBuf> = input_paths
        .par_iter()
        .flat_map_iter(|path| {
            let std_path = path.as_std_path();
            if std_path.is_file() {
                if is_supported_image(std_path) {
                    vec![path.clone()].into_iter()
                } else {
                    vec![].into_iter()
                }
            } else if std_path.is_dir() {
                match scan_directory_recursive_parallel(std_path, scan_subfolders, 0) {
                    Ok(dir_paths) => dir_paths.into_iter(),
                    Err(e) => {
                        warn!("Failed to scan directory {}: {}", path, e);
                        vec![].into_iter()
                    }
                }
            } else {
                vec![].into_iter()
            }
        })
        .collect();

    paths.sort_by(|a, b| alphanumeric_sort::compare_str(a.as_str(), b.as_str()));

    if paths.is_empty() {
        return Err(SldshowError::NoImagesFound {
            paths: input_paths.to_vec(),
        });
    }

    Ok(paths)
}

fn scan_directory_recursive_parallel(
    dir: &Path,
    recursive: bool,
    depth: usize,
) -> Result<Vec<Utf8PathBuf>> {
    if depth >= MAX_SCAN_DEPTH {
        warn!(
            "Maximum scan depth ({}) reached at: {}",
            MAX_SCAN_DEPTH,
            dir.display()
        );
        return Ok(Vec::new());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            return Ok(Vec::new());
        }
    };

    let paths: Vec<Utf8PathBuf> = entries
        .flatten()
        .par_bridge()
        .flat_map_iter(|entry| {
            let path = entry.path();
            if path.is_file() && is_supported_image(&path) {
                match Utf8PathBuf::try_from(path.clone()) {
                    Ok(utf8_path) => vec![utf8_path].into_iter(),
                    Err(_) => {
                        warn!("skipping non-UTF-8 path: {:?}", path);
                        vec![].into_iter()
                    }
                }
            } else if path.is_dir() && recursive {
                match scan_directory_recursive_parallel(&path, recursive, depth + 1) {
                    Ok(subdir_paths) => subdir_paths.into_iter(),
                    Err(e) => {
                        warn!("failed to scan directory {:?}: {}", path, e);
                        vec![].into_iter()
                    }
                }
            } else {
                vec![].into_iter()
            }
        })
        .collect();

    Ok(paths)
}

fn is_supported_image(path: &Path) -> bool {
    image::ImageFormat::from_path(path).is_ok()
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

    // --- mip_level_count() ---

    #[test]
    fn mip_level_count_1x1() {
        assert_eq!(mip_level_count(1, 1), 1);
    }

    #[test]
    fn mip_level_count_1024x1024() {
        // ilog2(1024) = 10, so 11 levels
        assert_eq!(mip_level_count(1024, 1024), 11);
    }

    #[test]
    fn mip_level_count_non_square_uses_max_dim() {
        // max dim = 1920, ilog2(1920) = 10, so 11 levels
        assert_eq!(mip_level_count(1920, 1080), 11);
    }

    // --- linear_to_srgb() ---

    #[test]
    fn linear_to_srgb_zero_maps_to_zero() {
        assert_eq!(linear_to_srgb(0.0), 0.0);
    }

    #[test]
    fn linear_to_srgb_one_maps_to_one() {
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn linear_to_srgb_low_value_uses_linear_segment() {
        // Values <= 0.003_130_8 use the linear c * 12.92 branch
        let v = 0.001_f32;
        assert!((linear_to_srgb(v) - v * 12.92).abs() < 1e-6);
    }
}
