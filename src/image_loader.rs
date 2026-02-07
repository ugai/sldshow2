use crate::error::{Result, SldshowError};
use crate::metadata::ImageMetadata;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use camino::{Utf8Path, Utf8PathBuf};
use image::GenericImageView;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

/// Maximum number of GPU texture uploads per frame to prevent blocking
const MAX_UPLOADS_PER_FRAME: usize = 1;

/// Maximum number of concurrent loading tasks to prevent CPU saturation
/// This prevents all CPU cores from being used for image loading/resizing,
/// leaving headroom for the main thread and GPU work.
/// Note: Even with async tasks, the actual image loading and Lanczos3 resizing
/// is CPU-bound work that can block if too many tasks run simultaneously.
const MAX_CONCURRENT_TASKS: usize = 1;

/// Type alias for image loading task result (image only, no metadata)
type ImageLoadResult = Result<(usize, Image)>;

/// Type alias for the image loading task
type ImageLoadTask = Task<ImageLoadResult>;

/// Supported image file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tga", "tiff", "tif", "ico", "hdr",
];

/// Image file entry with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ImageEntry {
    pub path: Utf8PathBuf,
    pub index: usize,
}

/// Image loader state
#[derive(Resource)]
pub struct ImageLoader {
    /// List of all scanned image paths
    pub paths: Vec<Utf8PathBuf>,
    /// Current display index
    pub current_index: usize,
    /// Whether shuffle mode is enabled
    pub shuffle: bool,
    /// Cache extent (number of images to preload before/after current)
    pub cache_extent: usize,
    /// Loaded image handles
    pub handles: HashMap<usize, Handle<Image>>,
    /// Active loading tasks (index -> task) - now includes metadata
    pub loading_tasks: HashMap<usize, ImageLoadTask>,
    /// Cached metadata for images
    pub metadata_cache: HashMap<usize, ImageMetadata>,
    /// Maximum texture size for GPU upload (width, height)
    /// Images larger than this will be downscaled before GPU upload
    pub max_texture_size: (u32, u32),
    /// Queue of images pending GPU upload (throttled to prevent frame stutter)
    /// Metadata is loaded separately/lazily to avoid blocking image display
    pub pending_uploads: VecDeque<(usize, Image)>,
    /// Whether the initial image has been loaded and displayed
    /// When false, only the current image is loaded (not the full cache)
    pub initial_load_complete: bool,
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            current_index: 0,
            shuffle: false,
            cache_extent: 5,
            handles: HashMap::new(),
            loading_tasks: HashMap::new(),
            metadata_cache: HashMap::new(),
            max_texture_size: (1920, 1080), // Default to 1080p
            pending_uploads: VecDeque::new(),
            initial_load_complete: false,
        }
    }
}

impl ImageLoader {
    /// Create a new image loader
    #[allow(dead_code)]
    pub fn new(cache_extent: usize) -> Self {
        Self {
            cache_extent,
            ..Default::default()
        }
    }

    /// Create a new image loader with maximum texture size
    #[allow(dead_code)]
    pub fn with_max_texture_size(cache_extent: usize, max_width: u32, max_height: u32) -> Self {
        Self {
            cache_extent,
            max_texture_size: (max_width, max_height),
            ..Default::default()
        }
    }

    /// Set the maximum texture size for GPU upload
    pub fn set_max_texture_size(&mut self, width: u32, height: u32) {
        self.max_texture_size = (width, height);
    }

    /// Scan paths for images (files or directories)
    #[allow(dead_code)]
    pub fn scan_paths(&mut self, input_paths: &[Utf8PathBuf], scan_subfolders: bool) -> Result<()> {
        let sorted_paths = scan_image_paths(input_paths, scan_subfolders)?;
        self.paths = sorted_paths;
        info!("Scanned {} images", self.paths.len());
        Ok(())
    }

    /// Shuffle the image list
    pub fn shuffle_paths(&mut self) {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.paths.shuffle(&mut rng);
    }

    /// Get current image path
    #[allow(dead_code)]
    pub fn current_path(&self) -> Option<&Utf8Path> {
        self.paths.get(self.current_index).map(|p| p.as_path())
    }

    /// Move to next image
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

    /// Move to previous image
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

    /// Get indices to preload based on cache extent
    ///
    /// During initial load (before first image is displayed), only returns
    /// the current image index to minimize startup time and prevent stuttering.
    pub fn get_preload_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();

        if self.paths.is_empty() {
            return indices;
        }

        // During initial load, only load the current image
        // This prevents 11+ simultaneous loads that cause startup freeze
        if !self.initial_load_complete {
            indices.push(self.current_index);
            return indices;
        }

        let len = self.paths.len();

        // Current image first
        indices.push(self.current_index);

        // Then alternate: next, previous, next+1, previous-1, etc.
        for i in 1..=self.cache_extent {
            // Next images
            let next_idx = (self.current_index + i) % len;
            indices.push(next_idx);

            // Previous images
            let prev_idx = if self.current_index >= i {
                self.current_index - i
            } else {
                len - (i - self.current_index)
            };
            indices.push(prev_idx);
        }

        indices
    }

    /// Get current image handle
    pub fn current_handle(&self) -> Option<&Handle<Image>> {
        self.handles.get(&self.current_index)
    }

    /// Get image count
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Get metadata for an image (cache-only, non-blocking)
    /// Returns cached metadata if available, None otherwise
    #[allow(dead_code)]
    pub fn get_metadata(&self, index: usize) -> Option<ImageMetadata> {
        self.metadata_cache.get(&index).cloned()
    }

    /// Get cached metadata for the current image (read-only)
    pub fn current_metadata(&self) -> Option<&ImageMetadata> {
        self.metadata_cache.get(&self.current_index)
    }

    /// Load metadata lazily for the current image (blocking, use sparingly)
    /// This loads EXIF data synchronously - only call when metadata is actually needed
    #[allow(dead_code)]
    pub fn load_metadata_lazy(&mut self, index: usize) -> Option<&ImageMetadata> {
        // Return cached if already loaded
        if self.metadata_cache.contains_key(&index) {
            return self.metadata_cache.get(&index);
        }

        // Load metadata synchronously (blocking but only when needed)
        if let Some(path) = self.paths.get(index) {
            let metadata = ImageMetadata::from_path(path);
            self.metadata_cache.insert(index, metadata);
            return self.metadata_cache.get(&index);
        }

        None
    }
}

/// Check if a path is a supported image file
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Resize an image to fit within maximum bounds while preserving aspect ratio
/// Only downscales; never upscales images smaller than the bounds.
fn resize_for_gpu(
    img: image::DynamicImage,
    max_width: u32,
    max_height: u32,
) -> image::DynamicImage {
    let (orig_w, orig_h) = img.dimensions();

    // Only downscale, never upscale
    if orig_w <= max_width && orig_h <= max_height {
        return img;
    }

    // Calculate scale to fit within bounds while preserving aspect ratio
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);

    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);

    debug!(
        "Resizing image from {}x{} to {}x{} (scale: {:.3})",
        orig_w, orig_h, new_w, new_h, scale
    );

    // Use Triangle filter (bilinear) for faster resize with acceptable quality
    // Lanczos3 gives better quality but is significantly slower for large images
    // Triangle is ~4-5x faster and quality difference is minimal at display sizes
    img.resize(new_w, new_h, image::imageops::FilterType::Triangle)
}

/// Load an image directly from a filesystem path (for absolute paths)
/// Optionally resizes to fit within max_texture_size bounds.
fn load_image_from_path(path: &Path, max_texture_size: Option<(u32, u32)>) -> Result<Image> {
    // Load image
    let img = image::open(path).map_err(|e| SldshowError::ImageLoadError {
        path: Utf8PathBuf::try_from(path.to_path_buf())
            .unwrap_or_else(|_| Utf8PathBuf::from(path.to_string_lossy().to_string())),
        source: e,
    })?;

    // Resize if max_texture_size is specified
    let img = if let Some((max_w, max_h)) = max_texture_size {
        resize_for_gpu(img, max_w, max_h)
    } else {
        img
    };

    let img_rgba = img.to_rgba8();
    let (width, height) = img_rgba.dimensions();

    Ok(Image::new(
        bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        img_rgba.into_raw(),
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    ))
}

/// Image loading plugin
///
/// Note: `load_images_system` is registered in `main.rs` to access `TransitionState`.
/// This plugin only initializes the `ImageLoader` resource.
pub struct ImageLoaderPlugin;

impl Plugin for ImageLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImageLoader>();
        // Note: load_images_system is registered in main.rs for TransitionState access
    }
}

/// System to handle image loading (async version)
///
/// Uses a throttled upload queue to prevent GPU stalls from multiple
/// simultaneous texture uploads. Only MAX_UPLOADS_PER_FRAME images
/// are uploaded to the GPU per frame.
///
/// The `transition_active` parameter controls preload behavior:
/// - When true: only the current image is uploaded (defers preloads)
/// - When false: preload images are also uploaded
///
/// This prevents frame spikes during transitions by deferring heavy
/// GPU uploads until the animation completes.
pub fn load_images_system_inner(
    loader: &mut ImageLoader,
    images: &mut Assets<Image>,
    transition_active: bool,
) {
    if loader.paths.is_empty() {
        return;
    }

    // Poll existing tasks and collect completed results (non-blocking check)
    let mut completed_results = Vec::new();
    loader.loading_tasks.retain(|idx, task| {
        if task.is_finished() {
            // Task is done, extract result (block_on is instant for finished tasks)
            let result = bevy::tasks::block_on(task);
            completed_results.push((*idx, result));
            false // Remove completed task
        } else {
            true // Keep pending task
        }
    });

    // Queue completed images for GPU upload (instead of immediate upload)
    // Note: Metadata is NOT loaded here - it's loaded lazily when needed
    for (idx, result) in completed_results {
        match result {
            Ok((task_idx, image)) => {
                debug!(
                    "Image loaded (queued for upload): {}x{}",
                    image.width(),
                    image.height()
                );
                // Queue for GPU upload instead of immediate upload
                loader.pending_uploads.push_back((task_idx, image));
            }
            Err(e) => {
                error!("Failed to load image at index {}: {}", idx, e);
            }
        }
    }

    // Process pending uploads with throttling (MAX_UPLOADS_PER_FRAME per frame)
    // This prevents GPU stalls from uploading many textures at once
    //
    // IMPORTANT: After initial load, we only upload when:
    // 1. The current image needs uploading (priority)
    // 2. There's no active transition (to avoid stuttering during animation)
    let current_index = loader.current_index;
    let mut uploads_this_frame = 0;

    // Always prioritize current image upload
    let current_needs_upload = !loader.handles.contains_key(&current_index)
        && loader
            .pending_uploads
            .iter()
            .any(|(idx, _)| *idx == current_index);

    while uploads_this_frame < MAX_UPLOADS_PER_FRAME {
        // Prioritize current image upload for faster initial display
        let upload = if let Some(pos) = loader
            .pending_uploads
            .iter()
            .position(|(idx, _)| *idx == current_index)
        {
            loader.pending_uploads.remove(pos)
        } else if loader.initial_load_complete && !current_needs_upload && !transition_active {
            // After initial load, only upload preload images if:
            // - Current image is ready (not needs_upload)
            // - No transition is active (prevents frame spikes during animation)
            loader.pending_uploads.pop_front()
        } else if !loader.initial_load_complete {
            // During initial load, process any pending upload
            loader.pending_uploads.pop_front()
        } else {
            // Current image needs upload but isn't in queue yet - wait
            break;
        };

        let Some((task_idx, image)) = upload else {
            break;
        };

        // GPU texture upload happens here (this is the blocking operation)
        let handle = images.add(image);
        loader.handles.insert(task_idx, handle);
        // Note: Metadata is NOT loaded here - loaded lazily via get_metadata_lazy()
        uploads_this_frame += 1;

        // Mark initial load complete after first image is uploaded
        if task_idx == current_index && !loader.initial_load_complete {
            loader.initial_load_complete = true;
            info!("Initial image loaded, enabling full preload cache");
        }
    }

    // Log pending queue status if there are waiting uploads
    if !loader.pending_uploads.is_empty() {
        debug!(
            "Pending GPU uploads: {} (throttled to {}/frame)",
            loader.pending_uploads.len(),
            MAX_UPLOADS_PER_FRAME
        );
    }

    // Get indices that should be loaded
    let preload_indices = loader.get_preload_indices();

    // Start loading tasks for images that aren't already loaded, loading, or pending
    // CRITICAL: Limit concurrent tasks to prevent CPU saturation from many simultaneous
    // image loads. Without this limit, initial_load_complete triggers 10+ task spawns
    // that can freeze the main thread for 10+ seconds while CPU processes all images.
    let pending_indices: HashSet<usize> =
        loader.pending_uploads.iter().map(|(idx, _)| *idx).collect();

    let task_pool = AsyncComputeTaskPool::get();
    let max_texture_size = loader.max_texture_size;
    let current_task_count = loader.loading_tasks.len();

    for &idx in &preload_indices {
        // Don't start new tasks if we're at the limit
        if loader.loading_tasks.len() >= MAX_CONCURRENT_TASKS {
            break;
        }

        if !loader.handles.contains_key(&idx)
            && !loader.loading_tasks.contains_key(&idx)
            && !pending_indices.contains(&idx)
        {
            if let Some(path) = loader.paths.get(idx).cloned() {
                debug!("Starting async load for image: {}", path);

                // Spawn async task to load image ONLY (no metadata - loaded lazily)
                // This ensures fast image display without EXIF parsing delay
                let task = task_pool.spawn(async move {
                    // Load image from file with optional resizing
                    let image = load_image_from_path(path.as_std_path(), Some(max_texture_size))?;
                    Ok((idx, image))
                });

                loader.loading_tasks.insert(idx, task);
            }
        }
    }

    // Log when tasks are throttled
    if loader.loading_tasks.len() > current_task_count {
        debug!(
            "Active loading tasks: {} (max: {})",
            loader.loading_tasks.len(),
            MAX_CONCURRENT_TASKS
        );
    }

    // Remove handles and tasks that are too far from current index
    let indices_to_keep: HashSet<usize> = preload_indices.into_iter().collect();
    loader.handles.retain(|idx, handle| {
        if indices_to_keep.contains(idx) {
            true
        } else {
            // Only remove if the image is actually loaded (to avoid thrashing)
            if images.get(handle).is_some() {
                // Image is loaded, we can drop it
                false
            } else {
                // Still loading, keep it
                true
            }
        }
    });
    loader
        .loading_tasks
        .retain(|idx, _| indices_to_keep.contains(idx));

    // Also clean up pending uploads for images no longer needed
    loader
        .pending_uploads
        .retain(|(idx, _)| indices_to_keep.contains(idx));
}

/// Standalone function to scan paths (can be run in background thread)
/// Uses parallel iteration for improved performance on large directories
pub fn scan_image_paths(
    input_paths: &[Utf8PathBuf],
    scan_subfolders: bool,
) -> Result<Vec<Utf8PathBuf>> {
    // Parallel iteration over input paths
    let mut paths: Vec<Utf8PathBuf> = input_paths
        .par_iter()
        .flat_map_iter(|path| {
            let std_path = path.as_std_path();
            if std_path.is_file() {
                // File case: return iterator with single element if supported
                if is_supported_image(std_path) {
                    vec![path.clone()].into_iter()
                } else {
                    vec![].into_iter()
                }
            } else if std_path.is_dir() {
                // Directory case: scan recursively in parallel
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

    // Sort paths alphanumerically (must be sequential for consistent ordering)
    paths.sort_by(|a, b| alphanumeric_sort::compare_str(a.as_str(), b.as_str()));

    // Return error if no images found
    if paths.is_empty() {
        return Err(SldshowError::NoImagesFound {
            paths: input_paths.to_vec(),
        });
    }

    Ok(paths)
}

/// Parallel recursive directory scanning
/// Uses rayon for work-stealing parallelism across directory tree
fn scan_directory_recursive_parallel(dir: &Path, recursive: bool) -> Result<Vec<Utf8PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            return Ok(Vec::new()); // Return empty vec, don't fail entire scan
        }
    };

    // Parallel iteration over directory entries
    let paths: Vec<Utf8PathBuf> = entries
        .flatten() // Filter out Err entries
        .par_bridge() // Convert iterator to parallel iterator
        .flat_map_iter(|entry| {
            let path = entry.path();

            if path.is_file() && is_supported_image(&path) {
                // File case: convert to Utf8PathBuf, skip if not valid UTF-8
                match Utf8PathBuf::try_from(path) {
                    Ok(utf8_path) => vec![utf8_path].into_iter(),
                    Err(_) => vec![].into_iter(),
                }
            } else if path.is_dir() && recursive {
                // Recursive case: scan subdirectory in parallel
                match scan_directory_recursive_parallel(&path, recursive) {
                    Ok(subdir_paths) => subdir_paths.into_iter(),
                    Err(_) => vec![].into_iter(), // Silently skip failed subdirs
                }
            } else {
                vec![].into_iter()
            }
        })
        .collect();

    Ok(paths)
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        assert!(is_supported_image(Path::new("test.png")));
        assert!(is_supported_image(Path::new("test.jpg")));
        assert!(is_supported_image(Path::new("test.JPEG")));
        assert!(!is_supported_image(Path::new("test.txt")));
        assert!(!is_supported_image(Path::new("test")));
    }

    #[test]
    fn test_preload_indices() {
        let mut loader = ImageLoader::new(2);
        loader.paths = vec![
            Utf8PathBuf::from("1.png"),
            Utf8PathBuf::from("2.png"),
            Utf8PathBuf::from("3.png"),
            Utf8PathBuf::from("4.png"),
            Utf8PathBuf::from("5.png"),
        ];
        loader.current_index = 2;

        // Before initial load, only current image is returned
        let indices = loader.get_preload_indices();
        assert_eq!(indices, vec![2]); // only current

        // After initial load complete, full preload is returned
        loader.initial_load_complete = true;
        let indices = loader.get_preload_indices();
        assert!(indices.contains(&2)); // current
        assert!(indices.contains(&3)); // next
        assert!(indices.contains(&1)); // previous
    }

    #[test]
    fn test_next_wraps_around() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            Utf8PathBuf::from("1.png"),
            Utf8PathBuf::from("2.png"),
            Utf8PathBuf::from("3.png"),
        ];
        loader.current_index = 2; // Last image

        // Next should wrap to first
        assert!(loader.next(false));
        assert_eq!(loader.current_index, 0);
    }

    #[test]
    fn test_next_pause_at_last() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![Utf8PathBuf::from("1.png"), Utf8PathBuf::from("2.png")];
        loader.current_index = 1; // Last image

        // Should not advance when pause_at_last is true
        assert!(!loader.next(true));
        assert_eq!(loader.current_index, 1);
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            Utf8PathBuf::from("1.png"),
            Utf8PathBuf::from("2.png"),
            Utf8PathBuf::from("3.png"),
        ];
        loader.current_index = 0; // First image

        // Previous should wrap to last
        assert!(loader.previous());
        assert_eq!(loader.current_index, 2);
    }

    #[test]
    fn test_empty_loader() {
        let loader = ImageLoader::default();
        assert!(loader.is_empty());
        assert_eq!(loader.len(), 0);
        assert!(loader.current_path().is_none());
    }

    #[test]
    fn test_shuffle_changes_order() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            Utf8PathBuf::from("1.png"),
            Utf8PathBuf::from("2.png"),
            Utf8PathBuf::from("3.png"),
            Utf8PathBuf::from("4.png"),
            Utf8PathBuf::from("5.png"),
        ];
        let original = loader.paths.clone();

        loader.shuffle_paths();

        // Paths should exist but may be in different order
        assert_eq!(loader.paths.len(), original.len());
        for path in &original {
            assert!(loader.paths.contains(path));
        }
    }
}
