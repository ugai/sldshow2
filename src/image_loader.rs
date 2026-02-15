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
const MAX_CONCURRENT_TASKS: usize = 2;

/// Maximum directory recursion depth to prevent infinite loops
const MAX_SCAN_DEPTH: usize = 128;

pub struct LoadedTexture {
    #[allow(dead_code)] // Kept alive to prevent GPU texture deallocation
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct TextureManager {
    pub paths: Vec<Utf8PathBuf>,
    pub current_index: usize,
    pub textures: HashMap<usize, LoadedTexture>,
    pub max_texture_size: (u32, u32),
    pub cache_extent: usize,

    // Async loading (sends mip chain: Vec[0]=base, Vec[1]=LOD1, ...)
    loading_tasks: HashSet<usize>,
    tx: Sender<(usize, anyhow::Result<Vec<image::RgbaImage>>)>,
    rx: Receiver<(usize, anyhow::Result<Vec<image::RgbaImage>>)>,
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
            loading_tasks: HashSet::new(),
            tx,
            rx,
        }
    }

    pub fn scan_paths(&mut self, input_paths: &[Utf8PathBuf], scan_subfolders: bool) -> Result<()> {
        let sorted_paths = scan_image_paths(input_paths, scan_subfolders)?;
        self.paths = sorted_paths;
        info!("Scanned {} images", self.paths.len());
        Ok(())
    }

    pub fn shuffle_paths(&mut self) {
        use rand::seq::SliceRandom;
        let mut rng = rand::rng();
        self.paths.shuffle(&mut rng);
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

    pub fn previous(&mut self) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        if self.current_index > 0 {
            self.current_index -= 1;
        } else {
            self.current_index = self.paths.len() - 1;
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
        self.paths = new_paths;
        self.textures.clear();
        self.loading_tasks.clear();
        self.current_index = 0;
        // Drain any in-flight results so stale images aren't uploaded later
        while self.rx.try_recv().is_ok() {}
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

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.paths.is_empty() {
            return;
        }

        // 1. Process received images and upload to GPU
        while let Ok((idx, result)) = self.rx.try_recv() {
            self.loading_tasks.remove(&idx);
            match result {
                Ok(mips) => {
                    let width = mips[0].width();
                    let height = mips[0].height();

                    let texture_size = wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    };

                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&format!("Image Texture {}", idx)),
                        size: texture_size,
                        mip_level_count: mips.len() as u32,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    for (level, mip) in mips.iter().enumerate() {
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &texture,
                                mip_level: level as u32,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            mip,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * mip.width()),
                                rows_per_image: Some(mip.height()),
                            },
                            wgpu::Extent3d {
                                width: mip.width(),
                                height: mip.height(),
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
                        },
                    );
                    debug!(
                        "Uploaded image {} ({}x{}, {} mips)",
                        idx,
                        width,
                        height,
                        mips.len()
                    );
                }
                Err(e) => {
                    error!("Failed to load image {}: {}", idx, e);
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
        self.loading_tasks
            .retain(|idx| needed_indices.contains(idx));

        for idx in needed_indices {
            if !self.textures.contains_key(&idx) && !self.loading_tasks.contains(&idx) {
                if self.loading_tasks.len() >= MAX_CONCURRENT_TASKS {
                    break;
                }

                if let Some(path) = self.paths.get(idx).cloned() {
                    let tx = self.tx.clone();
                    let max_size = self.max_texture_size;

                    self.loading_tasks.insert(idx);

                    std::thread::spawn(move || {
                        let res = load_image_rgba(&path, max_size);
                        if tx.send((idx, res)).is_err() {
                            warn!("Failed to send loaded image {} (receiver dropped)", idx);
                        }
                    });
                }
            }
        }
    }
}

// Standalone functions

fn load_image_rgba(path: &Utf8Path, max_size: (u32, u32)) -> anyhow::Result<Vec<image::RgbaImage>> {
    let img = image::open(path.as_std_path())
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;

    // Apply EXIF rotation
    let img = apply_exif_rotation(img, path);

    let resized = resize_for_gpu(img, max_size.0, max_size.1);
    let base = resized.to_rgba8();

    // Generate mipmap chain on CPU
    let mip_count = mip_level_count(base.width(), base.height());
    let mut mips = Vec::with_capacity(mip_count as usize);
    mips.push(base);

    for _ in 1..mip_count {
        let prev = mips.last().unwrap();
        let new_w = (prev.width() / 2).max(1);
        let new_h = (prev.height() / 2).max(1);
        mips.push(image::imageops::resize(
            prev,
            new_w,
            new_h,
            image::imageops::FilterType::Triangle,
        ));
    }

    Ok(mips)
}

fn mip_level_count(width: u32, height: u32) -> u32 {
    (width.max(height) as f32).log2().floor() as u32 + 1
}

fn apply_exif_rotation(img: image::DynamicImage, path: &Utf8Path) -> image::DynamicImage {
    use std::fs::File;
    use std::io::BufReader;

    let file = match File::open(path.as_std_path()) {
        Ok(f) => f,
        Err(_) => return img,
    };

    let mut reader = BufReader::new(&file);
    let exifreader = exif::Reader::new();
    let exif = match exifreader.read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(_) => return img,
    };

    match exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(field) => match field.value.get_uint(0) {
            Some(2) => img.fliph(),
            Some(3) => img.rotate180(),
            Some(4) => img.flipv(),
            Some(5) => img.rotate90().fliph(),
            Some(6) => img.rotate90(),
            Some(7) => img.rotate270().fliph(),
            Some(8) => img.rotate270(),
            _ => img,
        },
        None => img,
    }
}

fn resize_for_gpu(
    img: image::DynamicImage,
    max_width: u32,
    max_height: u32,
) -> image::DynamicImage {
    let (orig_w, orig_h) = img.dimensions();
    if orig_w <= max_width && orig_h <= max_height {
        return img;
    }
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);
    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);

    img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
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
                match Utf8PathBuf::try_from(path) {
                    Ok(utf8_path) => vec![utf8_path].into_iter(),
                    Err(_) => vec![].into_iter(),
                }
            } else if path.is_dir() && recursive {
                match scan_directory_recursive_parallel(&path, recursive, depth + 1) {
                    Ok(subdir_paths) => subdir_paths.into_iter(),
                    Err(_) => vec![].into_iter(),
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
