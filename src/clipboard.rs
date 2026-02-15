//! Clipboard operations for copying image data.

use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};
use camino::Utf8Path;
use std::borrow::Cow;

/// Copies the image at the given path to the system clipboard.
///
/// Re-reads the image from disk to avoid holding large bitmaps in RAM.
/// The image is converted to RGBA format for clipboard compatibility.
pub fn copy_image_to_clipboard(path: &Utf8Path) -> Result<()> {
    // Re-read from disk to save RAM
    let img = image::open(path)
        .with_context(|| format!("Failed to read image from {}", path))?
        .to_rgba8();

    let width = img.width() as usize;
    let height = img.height() as usize;
    let rgba_data = img.into_raw();

    let image_data = ImageData {
        width,
        height,
        bytes: Cow::Borrowed(&rgba_data),
    };

    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    clipboard
        .set_image(image_data)
        .context("Failed to set clipboard image data")?;

    Ok(())
}
