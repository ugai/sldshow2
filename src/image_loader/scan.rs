//! Image path scanning and directory traversal.

use crate::error::{Result, SldshowError};
use camino::Utf8PathBuf;
use log::warn;
use rayon::prelude::*;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Maximum directory recursion depth to prevent infinite loops
const MAX_SCAN_DEPTH: usize = 128;

pub fn scan_image_paths(
    input_paths: &[Utf8PathBuf],
    scan_subfolders: bool,
) -> Result<Vec<Utf8PathBuf>> {
    // Collect fatal I/O errors so we can surface one if no images are found.
    let errors: Arc<Mutex<Vec<(Utf8PathBuf, std::io::Error)>>> = Arc::new(Mutex::new(Vec::new()));

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
                match scan_directory_recursive_parallel(std_path, scan_subfolders, 0, &errors) {
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
        // Surface the first fatal I/O error rather than a generic NoImagesFound.
        let mut guard = errors.lock().unwrap();
        if let Some((path, source)) = guard.drain(..).next() {
            return Err(SldshowError::ScanFailed { path, source });
        }
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
    errors: &Arc<Mutex<Vec<(Utf8PathBuf, std::io::Error)>>>,
) -> Result<Vec<Utf8PathBuf>> {
    if depth >= MAX_SCAN_DEPTH {
        warn!(
            "Maximum scan depth ({}) reached at: {}",
            MAX_SCAN_DEPTH,
            dir.display()
        );
        return Ok(Vec::new());
    }

    let read_dir = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            if let Ok(utf8) = Utf8PathBuf::try_from(dir.to_path_buf())
                && let Ok(mut guard) = errors.lock()
            {
                // Preserve the first error per unique path.
                if !guard.iter().any(|(p, _)| p == &utf8) {
                    guard.push((utf8, e));
                }
            }
            return Ok(Vec::new());
        }
    };

    let paths: Vec<Utf8PathBuf> = read_dir
        .flatten()
        .par_bridge()
        .flat_map_iter(|entry| {
            // Use DirEntry::file_type() — no extra syscall and does NOT follow
            // symlinks, so a symlink pointing back to a parent cannot cause
            // infinite recursion.
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    warn!("failed to get file type for {:?}: {}", entry.path(), e);
                    return vec![].into_iter();
                }
            };

            // Skip symlinks to prevent cycles.
            if file_type.is_symlink() {
                return vec![].into_iter();
            }

            let path = entry.path();

            if file_type.is_file() && is_supported_image(&path) {
                match Utf8PathBuf::try_from(path.clone()) {
                    Ok(utf8_path) => vec![utf8_path].into_iter(),
                    Err(_) => {
                        warn!("skipping non-UTF-8 path: {:?}", path);
                        vec![].into_iter()
                    }
                }
            } else if file_type.is_dir() && recursive {
                match scan_directory_recursive_parallel(&path, recursive, depth + 1, errors) {
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
    use std::fs;
    use tempfile::tempdir;

    fn create_test_image(dir: &Path, name: &str) {
        // Minimal valid PNG (1x1 white pixel).
        let path = dir.join(name);
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR length + type
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bit depth, color type, etc.
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IHDR CRC, IDAT length + type
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xFF, 0xFF, 0x3F, // IDAT data
            0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA1, 0xF4, 0x7D, // IDAT data cont.
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND
            0xAE, 0x42, 0x60, 0x82, // IEND CRC
        ];
        fs::write(path, png_bytes).unwrap();
    }

    #[test]
    fn test_scan_basic() {
        let dir = tempdir().unwrap();
        create_test_image(dir.path(), "test.png");

        let utf8_dir = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let result = scan_image_paths(&[utf8_dir], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_scan_empty_dir_returns_no_images_found() {
        let dir = tempdir().unwrap();
        let utf8_dir = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let result = scan_image_paths(&[utf8_dir], false);
        assert!(matches!(result, Err(SldshowError::NoImagesFound { .. })));
    }

    /// Symlink pointing back to its parent — scan must not recurse forever and
    /// must not produce duplicate entries.
    #[test]
    fn test_symlink_cycle_does_not_recurse() {
        let dir = tempdir().unwrap();
        create_test_image(dir.path(), "img.png");

        let link_path = dir.path().join("self_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path(), &link_path).unwrap();
        #[cfg(windows)]
        {
            // Requires Developer Mode or elevation; skip silently if unavailable.
            if std::os::windows::fs::symlink_dir(dir.path(), &link_path).is_err() {
                return;
            }
        }

        let utf8_dir = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let result = scan_image_paths(&[utf8_dir], true);
        assert!(result.is_ok(), "scan should not panic or loop forever");
        // Symlink is skipped so only the one real image should appear.
        assert_eq!(
            result.unwrap().len(),
            1,
            "symlink cycle must not produce duplicates"
        );
    }

    /// On Unix a directory with mode 0o000 must surface as ScanFailed, not NoImagesFound.
    #[cfg(unix)]
    #[test]
    fn test_permission_denied_surfaces_scan_failed() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        create_test_image(&locked, "secret.png");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let utf8_locked = Utf8PathBuf::try_from(locked.clone()).unwrap();
        let result = scan_image_paths(&[utf8_locked], false);

        // Restore permissions so tempdir cleanup can remove the directory.
        let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o755));

        assert!(
            matches!(result, Err(SldshowError::ScanFailed { .. })),
            "permission-denied read_dir should yield ScanFailed, got: {:?}",
            result
        );
    }
}
