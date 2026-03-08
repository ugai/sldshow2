//! Clipboard operations for copying image data.

use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};
use camino::Utf8Path;
use std::borrow::Cow;
use std::io::{BufReader, Seek, SeekFrom};

use crate::image_loader::{apply_orientation, read_exif_orientation};

/// Copies the image at the given path to the system clipboard.
///
/// Re-reads the image from disk to avoid holding large bitmaps in RAM.
/// Applies EXIF orientation so the copied image matches the viewer display.
pub fn copy_image_to_clipboard(path: &Utf8Path) -> Result<()> {
    let file = std::fs::File::open(path.as_std_path())
        .with_context(|| format!("Failed to open image from {}", path))?;
    let mut reader = BufReader::new(file);

    let orientation = read_exif_orientation(&mut reader);
    reader
        .seek(SeekFrom::Start(0))
        .with_context(|| format!("Failed to seek image: {}", path))?;

    let img = image::ImageReader::new(&mut reader)
        .with_guessed_format()
        .with_context(|| format!("Failed to guess image format: {}", path))?
        .decode()
        .with_context(|| format!("Failed to decode image from {}", path))?;

    let img = apply_orientation(img, orientation).to_rgba8();

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
