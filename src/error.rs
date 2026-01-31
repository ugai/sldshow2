//! Custom error types for sldshow2
//!
//! Provides structured error handling with context using thiserror.

use camino::Utf8PathBuf;
use thiserror::Error;

/// Main error type for sldshow2 operations
#[derive(Error, Debug)]
pub enum SldshowError {
    /// Failed to load an image file
    #[error("Failed to load image from {path}: {source}")]
    ImageLoadError {
        path: Utf8PathBuf,
        #[source]
        source: image::ImageError,
    },

    /// Failed to scan a directory
    #[error("Failed to scan directory {path}: {source}")]
    #[allow(dead_code)]
    DirectoryScanError {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// No images found in the specified paths
    #[error("No images found in paths: {}", paths.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "))]
    NoImagesFound { paths: Vec<Utf8PathBuf> },

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    #[allow(dead_code)]
    ConfigError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Image error
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
}

/// Result type alias for sldshow2 operations
pub type Result<T> = std::result::Result<T, SldshowError>;
