//! TOML-based application configuration with validation.

use crate::error::{Result, SldshowError};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Texture filtering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilterMode {
    /// Nearest-neighbor filtering (pixelated, sharp)
    Nearest,
    /// Linear filtering (smooth, blurred)
    #[default]
    Linear,
}

impl FilterMode {
    /// Convert to wgpu FilterMode
    pub fn to_wgpu(self) -> wgpu::FilterMode {
        match self {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
pub struct Config {
    #[serde(default)]
    #[validate(nested)]
    pub window: WindowConfig,
    #[serde(default)]
    #[validate(nested)]
    pub viewer: ViewerConfig,
    #[serde(default)]
    #[validate(nested)]
    pub transition: TransitionConfig,
    #[serde(default)]
    pub style: StyleConfig,
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct WindowConfig {
    #[validate(range(min = 320, max = 7680))]
    pub width: u32,
    #[validate(range(min = 240, max = 4320))]
    pub height: u32,
    pub fullscreen: bool,
    pub always_on_top: bool,
    pub decorations: bool,
    pub resizable: bool,
    pub monitor_index: usize,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fullscreen: false,
            always_on_top: false,
            decorations: true,
            resizable: false,
            monitor_index: 0,
        }
    }
}

/// Viewer configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct ViewerConfig {
    pub image_paths: Vec<Utf8PathBuf>,
    #[validate(range(min = 0.0))]
    pub timer: f32,
    pub scan_subfolders: bool,
    pub shuffle: bool,
    pub pause_at_last: bool,
    #[validate(range(min = 1, max = 100))]
    pub cache_extent: usize,
    pub hot_reload: bool,
    /// Maximum texture size [width, height] for GPU upload.
    /// Images larger than this are downscaled before GPU upload to reduce frame spikes.
    /// Lower values = faster uploads but lower quality. [1920, 1080] is a good balance.
    /// Set to [0, 0] to use window dimensions (may cause frame spikes at 4K+).
    pub max_texture_size: [u32; 2],
    pub filter_mode: FilterMode,
    /// Display mode: "Fit" (black bars) or "AmbientFit" (blurred background fills letterbox)
    pub fit_mode: String,
    /// Mip LOD level for ambient fit blur (higher = blurrier, default 5.0)
    #[validate(range(min = 0.0, max = 10.0))]
    pub ambient_blur: f32,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            image_paths: Vec::new(),
            timer: 10.0,
            scan_subfolders: true,
            shuffle: true,
            pause_at_last: false,
            cache_extent: 5,
            hot_reload: true,
            max_texture_size: [1920, 1080],
            filter_mode: FilterMode::Linear,
            fit_mode: "Fit".to_string(),
            ambient_blur: 5.0,
        }
    }
}

/// Transition configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct TransitionConfig {
    #[validate(range(min = 0.0, max = 10.0))]
    pub time: f32,
    pub random: bool,
    #[validate(range(min = 0, max = 19))]
    pub mode: i32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            time: 0.5,
            random: true,
            mode: 0,
        }
    }
}

/// Style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    pub bg_color: [u8; 4],
    pub show_image_path: bool,
    pub show_controls: bool,
    pub font_family: Option<String>,
    pub text_color: [u8; 4],
    pub font_size: f32,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            bg_color: [0, 0, 0, 255],
            show_image_path: false,
            show_controls: true,
            font_family: None,
            text_color: [255, 255, 255, 255],
            font_size: 20.0,
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load<P: AsRef<Utf8Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref.as_std_path()).map_err(|e| {
            SldshowError::ConfigLoadError {
                path: path_ref.to_path_buf(),
                source: e,
            }
        })?;

        let config: Config =
            toml::from_str(&content).map_err(|e| SldshowError::ConfigParseError {
                path: path_ref.to_path_buf(),
                source: Box::new(e),
            })?;

        config.validate()?;

        Ok(config)
    }

    /// Load configuration from default locations
    /// 1. Command line argument
    /// 2. ~/.sldshow
    /// 3. Default config
    pub fn load_default(config_path: Option<Utf8PathBuf>) -> Result<Self> {
        if let Some(path) = config_path {
            if path.as_std_path().exists() {
                return Self::load(&path);
            } else {
                return Err(SldshowError::ConfigLoadError {
                    path: path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Config file not found: {}", path),
                    ),
                });
            }
        }

        if let Some(home) = dirs::home_dir() {
            let home_config = home.join(".sldshow");
            if home_config.exists() {
                if let Ok(utf8_path) = Utf8PathBuf::try_from(home_config) {
                    return Self::load(&utf8_path);
                }
            }
        }

        Ok(Self::default())
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save<P: AsRef<Utf8Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;

        std::fs::write(path.as_ref().as_std_path(), content).map_err(|e| {
            SldshowError::ConfigLoadError {
                path: path.as_ref().to_path_buf(),
                source: e,
            }
        })?;

        Ok(())
    }

    /// Get background color as normalized f32 array
    pub fn bg_color_f32(&self) -> [f32; 4] {
        [
            self.style.bg_color[0] as f32 / 255.0,
            self.style.bg_color[1] as f32 / 255.0,
            self.style.bg_color[2] as f32 / 255.0,
            self.style.bg_color[3] as f32 / 255.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.window.width, 1280);
        assert_eq!(config.window.height, 720);
        assert_eq!(config.viewer.timer, 10.0);
        assert_eq!(config.transition.time, 0.5);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.window.width, deserialized.window.width);
        assert_eq!(config.viewer.timer, deserialized.viewer.timer);
    }
}
