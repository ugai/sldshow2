//! File system watcher for hot-reload functionality
//!
//! Monitors image directories for changes and automatically refreshes the image list.

use bevy::prelude::*;
use camino::Utf8PathBuf;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::sync::{mpsc::{channel, Receiver, Sender}, Arc, Mutex};
use std::time::Duration;

use crate::image_loader::ImageLoader;

/// Resource for managing file system watchers
#[derive(Resource)]
pub struct FileWatcher {
    #[allow(dead_code)]
    debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    receiver: Arc<Mutex<Receiver<DebounceEventResult>>>,
    watched_paths: Vec<Utf8PathBuf>,
    scan_subfolders: bool,
}

impl FileWatcher {
    /// Create a new FileWatcher for the given paths
    pub fn new(paths: Vec<Utf8PathBuf>, scan_subfolders: bool) -> anyhow::Result<Self> {
        let (tx, rx): (Sender<DebounceEventResult>, Receiver<DebounceEventResult>) = channel();

        // Create debouncer with 500ms delay to avoid rapid re-scans
        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                if tx.send(result).is_err() {
                    error!("Failed to send file watcher event");
                }
            },
        )?;

        // Watch all input paths
        let recursive_mode = if scan_subfolders {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        for path in &paths {
            if path.as_std_path().is_dir() {
                debouncer
                    .watch(path.as_std_path(), recursive_mode)
                    .map_err(|e| anyhow::anyhow!("Failed to watch path {}: {}", path, e))?;
                info!("Watching directory: {}", path);
            }
        }

        Ok(Self {
            debouncer,
            receiver: Arc::new(Mutex::new(rx)),
            watched_paths: paths,
            scan_subfolders,
        })
    }

    /// Poll for file system events and return true if a rescan is needed
    pub fn poll_events(&self) -> bool {
        let mut needs_rescan = false;

        // Lock the receiver to process events
        if let Ok(receiver) = self.receiver.lock() {
            // Process all pending events
            while let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(events) => {
                        for event in events {
                            if self.is_image_event(&event.event) {
                                debug!("File system event: {:?}", event.event);
                                needs_rescan = true;
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            warn!("File watcher error: {}", error);
                        }
                    }
                }
            }
        }

        needs_rescan
    }

    /// Check if an event is related to image files
    fn is_image_event(&self, event: &Event) -> bool {
        match &event.kind {
            EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {
                // Check if any path in the event is an image file
                event.paths.iter().any(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| {
                            matches!(
                                ext.to_lowercase().as_str(),
                                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                            )
                        })
                        .unwrap_or(false)
                })
            }
            _ => false,
        }
    }

    pub fn watched_paths(&self) -> &[Utf8PathBuf] {
        &self.watched_paths
    }
}

/// System that polls the file watcher and triggers rescans when needed
pub fn poll_file_watcher_system(
    watcher: Option<Res<FileWatcher>>,
    mut loader: ResMut<ImageLoader>,
) {
    if let Some(watcher) = watcher {
        if watcher.poll_events() {
            info!("File system changes detected, rescanning images...");

            // Rescan the watched paths
            match crate::image_loader::scan_image_paths(watcher.watched_paths(), watcher.scan_subfolders)
            {
                Ok(new_paths) => {
                    let old_count = loader.paths.len();
                    let new_count = new_paths.len();

                    loader.paths = new_paths;

                    // Clear current index if out of bounds
                    if loader.current_index >= loader.paths.len() {
                        loader.current_index = if !loader.paths.is_empty() {
                            0
                        } else {
                            0  // Will be handled by loader logic
                        };
                    }

                    info!(
                        "Image list updated: {} → {} images",
                        old_count, new_count
                    );
                }
                Err(e) => {
                    error!("Failed to rescan images: {}", e);
                }
            }
        }
    }
}
