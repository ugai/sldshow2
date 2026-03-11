//! EXIF orientation handling and EXR metadata extraction.

use camino::Utf8Path;

/// Read the EXIF orientation tag from a reader without consuming the whole stream.
pub(crate) fn read_exif_orientation<R: std::io::BufRead + std::io::Seek>(
    reader: &mut R,
) -> Option<u32> {
    exif::Reader::new()
        .read_from_container(reader)
        .ok()?
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

/// Apply a raw EXIF orientation value to an image.
pub(crate) fn apply_orientation(
    img: image::DynamicImage,
    orientation: Option<u32>,
) -> image::DynamicImage {
    match orientation {
        Some(2) => img.fliph(),
        Some(3) => img.rotate180(),
        Some(4) => img.flipv(),
        Some(5) => img.rotate90().fliph(),
        Some(6) => img.rotate90(),
        Some(7) => img.rotate270().fliph(),
        Some(8) => img.rotate270(),
        _ => img,
    }
}

/// Extract framerate from EXR metadata.
/// Returns None if the file is not readable or lacks framesPerSecond attribute.
pub(super) fn extract_exr_fps(path: &Utf8Path) -> Option<f32> {
    use exr::prelude::*;

    let reader = match read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .all_layers()
        .all_attributes()
        .non_parallel()
        .from_file(path.as_std_path())
    {
        Ok(r) => r,
        Err(_) => return None,
    };

    // Check standard framesPerSecond attribute
    for layer in &reader.layer_data {
        for (name, value) in &layer.attributes.other {
            if name == "framesPerSecond" {
                if let AttributeValue::F32(fps) = value {
                    if *fps > 0.0 && fps.is_finite() {
                        return Some(*fps);
                    }
                }
            }
        }
    }

    None
}
