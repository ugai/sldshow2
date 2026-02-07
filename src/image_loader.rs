use crate::error::{Result, SldshowError};
use camino::{Utf8Path, Utf8PathBuf};
use image::GenericImageView;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use log::{info, error, warn, debug};

/// Maximum number of concurrent loading tasks
const MAX_CONCURRENT_TASKS: usize = 2;

/// Supported image file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tga", "tiff", "tif", "ico", "hdr",
];

#[allow(dead_code)]
pub struct LoadedTexture {
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
    
    // Async loading
    loading_tasks: HashSet<usize>,
    tx: Sender<(usize, anyhow::Result<image::RgbaImage>)>,
    rx: Receiver<(usize, anyhow::Result<image::RgbaImage>)>,
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
        let mut rng = rand::thread_rng();
        self.paths.shuffle(&mut rng);
    }

    pub fn next(&mut self, pause_at_last: bool) -> bool {
        if self.paths.is_empty() { return false; }
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
        if self.paths.is_empty() { return false; }
        if self.current_index > 0 {
            self.current_index -= 1;
        } else {
            self.current_index = self.paths.len() - 1;
        }
        true
    }
    
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    #[allow(dead_code)]
    pub fn current_path(&self) -> Option<&Utf8Path> {
        self.paths.get(self.current_index).map(|p| p.as_path())
    }

    #[allow(dead_code)]
    pub fn get_current_texture(&self) -> Option<&LoadedTexture> {
        self.textures.get(&self.current_index)
    }

    pub fn get_texture(&self, index: usize) -> Option<&LoadedTexture> {
        self.textures.get(&index)
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.paths.is_empty() { return; }

        // 1. Process received images and upload to GPU
        // Non-blocking try_recv
        while let Ok((idx, result)) = self.rx.try_recv() {
            self.loading_tasks.remove(&idx);
            match result {
                Ok(img) => {
                    let width = img.width();
                    let height = img.height();
                    
                    let texture_size = wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    };

                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&format!("Image Texture {}", idx)),
                        size: texture_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &img,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * width),
                            rows_per_image: Some(height),
                        },
                        texture_size,
                    );

                    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                    self.textures.insert(idx, LoadedTexture {
                        texture,
                        view,
                        width,
                        height,
                    });
                    debug!("Uploaded image {} ({}x{})", idx, width, height);
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
        for i in 1..=self.cache_extent {
            needed_indices.insert((self.current_index + i) % len); // Forward
             needed_indices.insert((self.current_index + len - i) % len); // Backward
        }

        // Cleanup unused textures
        self.textures.retain(|idx, _| needed_indices.contains(idx));
        
        // Start new tasks
        for idx in needed_indices {
            if !self.textures.contains_key(&idx) && !self.loading_tasks.contains(&idx) {
                if self.loading_tasks.len() >= MAX_CONCURRENT_TASKS {
                    break;
                }
                
                if let Some(path) = self.paths.get(idx).cloned() {
                    let tx = self.tx.clone();
                    let max_size = self.max_texture_size;
                    
                    self.loading_tasks.insert(idx);
                    
                    // Spawn thread
                    std::thread::spawn(move || {
                        let res = load_image_rgba(&path, max_size);
                        let _ = tx.send((idx, res));
                    });
                }
            }
        }
    }
}

// Standalone functions

fn load_image_rgba(path: &Utf8Path, max_size: (u32, u32)) -> anyhow::Result<image::RgbaImage> {
    let img = image::open(path.as_std_path()).map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;
    let resized = resize_for_gpu(img, max_size.0, max_size.1);
    Ok(resized.to_rgba8())
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
    
    // Using Triangle filter for speed
    img.resize(new_w, new_h, image::imageops::FilterType::Triangle)
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
                match scan_directory_recursive_parallel(std_path, scan_subfolders) {
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
        }.into());
    }

    Ok(paths)
}

fn scan_directory_recursive_parallel(dir: &Path, recursive: bool) -> Result<Vec<Utf8PathBuf>> {
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
                match scan_directory_recursive_parallel(&path, recursive) {
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

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
