//! Custom error types for sldshow2.

use camino::Utf8PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SldshowError {
    #[error("Failed to load image from {path}: {source}")]
    #[allow(dead_code)]
    ImageLoadError {
        path: Utf8PathBuf,
        #[source]
        source: image::ImageError,
    },

    #[error("Failed to scan directory {path}: {source}")]
    #[allow(dead_code)]
    DirectoryScanError {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("No images found in paths: {}", paths.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "))]
    NoImagesFound { paths: Vec<Utf8PathBuf> },

    #[error("Failed to load config from {path}: {source}")]
    ConfigLoadError {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config from {path}: {source}")]
    ConfigParseError {
        path: Utf8PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("Invalid configuration: {0}")]
    ConfigValidationError(#[from] validator::ValidationErrors),

    #[error("Failed to serialize config: {0}")]
    ConfigSerializeError(#[from] toml::ser::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, SldshowError>;
