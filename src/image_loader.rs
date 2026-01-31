use crate::error::{Result, SldshowError};
use crate::metadata::ImageMetadata;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Supported image file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tga", "tiff", "tif", "ico", "hdr",
];

/// Image file entry with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ImageEntry {
    pub path: PathBuf,
    pub index: usize,
}

/// Image loader state
#[derive(Resource)]
pub struct ImageLoader {
    /// List of all scanned image paths
    pub paths: Vec<PathBuf>,
    /// Current display index
    pub current_index: usize,
    /// Whether shuffle mode is enabled
    pub shuffle: bool,
    /// Cache extent (number of images to preload before/after current)
    pub cache_extent: usize,
    /// Loaded image handles
    pub handles: HashMap<usize, Handle<Image>>,
    /// Active loading tasks (index -> task)
    pub loading_tasks: HashMap<usize, Task<Result<(usize, Image)>>>,
    /// Cached metadata for images
    pub metadata_cache: HashMap<usize, ImageMetadata>,
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

    /// Scan paths for images (files or directories)
    #[allow(dead_code)]
    pub fn scan_paths(&mut self, input_paths: &[PathBuf], scan_subfolders: bool) -> Result<()> {
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
    pub fn current_path(&self) -> Option<&Path> {
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
    pub fn get_preload_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();

        if self.paths.is_empty() {
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

    /// Get metadata for an image, loading it if not cached
    pub fn get_metadata(&mut self, index: usize) -> Option<ImageMetadata> {
        // Return cached if available
        if let Some(metadata) = self.metadata_cache.get(&index) {
            return Some(metadata.clone());
        }

        // Load metadata if path exists
        if let Some(path) = self.paths.get(index) {
            let metadata = ImageMetadata::from_path(path);
            self.metadata_cache.insert(index, metadata.clone());
            Some(metadata)
        } else {
            None
        }
    }

    /// Get metadata for the current image
    pub fn current_metadata(&mut self) -> Option<ImageMetadata> {
        self.get_metadata(self.current_index)
    }
}

/// Check if a path is a supported image file
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Load an image directly from a filesystem path (for absolute paths)
fn load_image_from_path(path: &Path) -> Result<Image> {
    // Load image
    let img = image::open(path).map_err(|e| SldshowError::ImageLoadError {
        path: path.to_path_buf(),
        source: e,
    })?;
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
pub struct ImageLoaderPlugin;

impl Plugin for ImageLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImageLoader>()
            .add_systems(Update, load_images_system);
    }
}

/// System to handle image loading (async version)
fn load_images_system(
    mut loader: ResMut<ImageLoader>,
    mut images: ResMut<Assets<Image>>,
) {
    if loader.paths.is_empty() {
        return;
    }

    // Poll existing tasks and collect completed results
    let mut completed_results = Vec::new();
    loader.loading_tasks.retain(|idx, task| {
        if let Some(result) = bevy::tasks::block_on(bevy::tasks::poll_once(task)) {
            completed_results.push((*idx, result));
            false // Remove completed task
        } else {
            true // Keep pending task
        }
    });

    // Add completed images to assets
    for (idx, result) in completed_results {
        match result {
            Ok((task_idx, image)) => {
                debug!("Successfully loaded image: {}x{}", image.width(), image.height());
                let handle = images.add(image);
                loader.handles.insert(task_idx, handle);
            }
            Err(e) => {
                error!("Failed to load image at index {}: {}", idx, e);
            }
        }
    }

    // Get indices that should be loaded
    let preload_indices = loader.get_preload_indices();

    // Start loading tasks for images that aren't already loaded or loading
    let task_pool = AsyncComputeTaskPool::get();
    for &idx in &preload_indices {
        if !loader.handles.contains_key(&idx) && !loader.loading_tasks.contains_key(&idx) {
            if let Some(path) = loader.paths.get(idx).cloned() {
                debug!("Starting async load for image: {}", path.display());

                // Spawn async task to load image
                let task = task_pool.spawn(async move {
                    load_image_from_path(&path).map(|img| (idx, img))
                });

                loader.loading_tasks.insert(idx, task);
            }
        }
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
    loader.loading_tasks.retain(|idx, _| indices_to_keep.contains(idx));
}


/// Standalone function to scan paths (can be run in background thread)
/// Uses parallel iteration for improved performance on large directories
pub fn scan_image_paths(input_paths: &[PathBuf], scan_subfolders: bool) -> Result<Vec<PathBuf>> {
    // Parallel iteration over input paths
    let mut paths: Vec<PathBuf> = input_paths
        .par_iter()
        .flat_map_iter(|path| {
            if path.is_file() {
                // File case: return iterator with single element if supported
                if is_supported_image(path) {
                    vec![path.clone()].into_iter()
                } else {
                    vec![].into_iter()
                }
            } else if path.is_dir() {
                // Directory case: scan recursively in parallel
                match scan_directory_recursive_parallel(path, scan_subfolders) {
                    Ok(dir_paths) => dir_paths.into_iter(),
                    Err(e) => {
                        warn!("Failed to scan directory {}: {}", path.display(), e);
                        vec![].into_iter()
                    }
                }
            } else {
                vec![].into_iter()
            }
        })
        .collect();

    // Sort paths alphanumerically (must be sequential for consistent ordering)
    paths.sort_by(|a, b| {
        alphanumeric_sort::compare_str(
            a.to_string_lossy().as_ref(),
            b.to_string_lossy().as_ref(),
        )
    });

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
fn scan_directory_recursive_parallel(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            return Ok(Vec::new());  // Return empty vec, don't fail entire scan
        }
    };

    // Parallel iteration over directory entries
    let paths: Vec<PathBuf> = entries
        .flatten()  // Filter out Err entries
        .par_bridge()  // Convert iterator to parallel iterator
        .flat_map_iter(|entry| {
            let path = entry.path();

            if path.is_file() && is_supported_image(&path) {
                // File case: return single-element iterator
                vec![path].into_iter()
            } else if path.is_dir() && recursive {
                // Recursive case: scan subdirectory in parallel
                match scan_directory_recursive_parallel(&path, recursive) {
                    Ok(subdir_paths) => subdir_paths.into_iter(),
                    Err(_) => vec![].into_iter(),  // Silently skip failed subdirs
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
            PathBuf::from("1.png"),
            PathBuf::from("2.png"),
            PathBuf::from("3.png"),
            PathBuf::from("4.png"),
            PathBuf::from("5.png"),
        ];
        loader.current_index = 2;

        let indices = loader.get_preload_indices();
        assert!(indices.contains(&2)); // current
        assert!(indices.contains(&3)); // next
        assert!(indices.contains(&1)); // previous
    }

    #[test]
    fn test_next_wraps_around() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            PathBuf::from("1.png"),
            PathBuf::from("2.png"),
            PathBuf::from("3.png"),
        ];
        loader.current_index = 2; // Last image

        // Next should wrap to first
        assert!(loader.next(false));
        assert_eq!(loader.current_index, 0);
    }

    #[test]
    fn test_next_pause_at_last() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            PathBuf::from("1.png"),
            PathBuf::from("2.png"),
        ];
        loader.current_index = 1; // Last image

        // Should not advance when pause_at_last is true
        assert!(!loader.next(true));
        assert_eq!(loader.current_index, 1);
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut loader = ImageLoader::new(1);
        loader.paths = vec![
            PathBuf::from("1.png"),
            PathBuf::from("2.png"),
            PathBuf::from("3.png"),
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
            PathBuf::from("1.png"),
            PathBuf::from("2.png"),
            PathBuf::from("3.png"),
            PathBuf::from("4.png"),
            PathBuf::from("5.png"),
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
