//! Image metadata extraction using EXIF data
//!
//! Provides lightweight metadata reading without full image decode.

use camino::Utf8Path;

/// Image metadata extracted from EXIF and file info
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Image width in pixels (from EXIF or actual)
    pub width: Option<u32>,
    /// Image height in pixels (from EXIF or actual)
    pub height: Option<u32>,
    /// Camera make (from EXIF)
    #[allow(dead_code)]
    pub camera_make: Option<String>,
    /// Camera model (from EXIF)
    #[allow(dead_code)]
    pub camera_model: Option<String>,
    /// Date/time taken (from EXIF)
    pub datetime: Option<String>,
    /// Orientation (from EXIF)
    #[allow(dead_code)]
    pub orientation: Option<u32>,
}

impl ImageMetadata {
    /// Extract metadata from an image file
    pub fn from_path(path: &Utf8Path) -> Self {
        match std::fs::File::open(path.as_std_path()) {
            Ok(mut file) => {
                let mut bufreader = std::io::BufReader::new(&mut file);
                match exif::Reader::new().read_from_container(&mut bufreader) {
                    Ok(exif) => Self::from_exif(exif),
                    Err(_) => Self::default(),
                }
            }
            Err(_) => Self::default(),
        }
    }

    /// Extract metadata from EXIF data
    fn from_exif(exif: exif::Exif) -> Self {
        let width = exif
            .get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0));

        let height = exif
            .get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0));

        let camera_make = exif
            .get_field(exif::Tag::Make, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into());

        let camera_model = exif
            .get_field(exif::Tag::Model, exif::In::PRIMARY)
            .and_then(|f| Some(f.display_value().to_string()));

        let datetime = exif
            .get_field(exif::Tag::DateTime, exif::In::PRIMARY)
            .and_then(|f| Some(f.display_value().to_string()));

        let orientation = exif
            .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0));

        Self {
            width,
            height,
            camera_make,
            camera_model,
            datetime,
            orientation,
        }
    }

    /// Get a display string for dimensions
    pub fn dimensions_string(&self) -> Option<String> {
        match (self.width, self.height) {
            (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
            _ => None,
        }
    }

    /// Get a short summary string
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(dims) = self.dimensions_string() {
            parts.push(dims);
        }

        if let Some(datetime) = &self.datetime {
            // Extract just the date part (YYYY:MM:DD)
            if let Some(date) = datetime.split_whitespace().next() {
                parts.push(date.replace(':', "-"));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join(" • ")
        }
    }
}

impl Default for ImageMetadata {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            camera_make: None,
            camera_model: None,
            datetime: None,
            orientation: None,
        }
    }
}
