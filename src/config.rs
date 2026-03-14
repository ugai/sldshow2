//! TOML-based application configuration with validation.

use crate::error::{Result, SldshowError};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use validator::Validate;

/// Minimum valid slideshow timer interval in seconds.
///
/// `0.0` is the sentinel for "paused". Values in `(0.0, TIMER_MIN)` are
/// rejected by config validation and clamped by the runtime timer.
/// This is the single source of truth referenced by `validate_timer`,
/// `timer::sanitize_timer`, and the settings UI.
pub const TIMER_MIN: f32 = 0.1;

/// Texture filtering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
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

/// Display fit mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum FitMode {
    /// Fit image with black bars
    #[default]
    Fit,
    /// Fill letterbox with blurred background
    AmbientFit,
}

impl FitMode {
    /// Convert to shader uniform value
    pub fn to_uniform_value(self) -> i32 {
        match self {
            FitMode::Fit => 0,
            FitMode::AmbientFit => 1,
        }
    }

    /// Toggle between Fit and AmbientFit
    pub fn toggle(&mut self) {
        *self = match *self {
            FitMode::Fit => FitMode::AmbientFit,
            FitMode::AmbientFit => FitMode::Fit,
        };
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
        }
    }
}

/// Playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum PlaybackMode {
    #[default]
    Slideshow,
    Sequence,
}

/// Viewer configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct ViewerConfig {
    pub image_paths: Vec<Utf8PathBuf>,
    #[validate(custom(function = "validate_timer"))]
    pub timer: f32,
    pub scan_subfolders: bool,
    pub shuffle: bool,
    pub pause_at_last: bool,
    #[validate(range(min = 1, max = 100))]
    pub cache_extent: usize,
    pub playback_mode: PlaybackMode,
    #[validate(range(min = 1.0, max = 240.0))]
    pub sequence_fps: f32,
    /// Maximum texture size [width, height] for GPU upload.
    /// Images larger than this are downscaled before GPU upload to reduce frame spikes.
    /// Lower values = faster uploads but lower quality. [1920, 1080] is a good balance.
    /// Set to [0, 0] for no limit (upload at full resolution; may cause frame spikes at 4K+).
    pub max_texture_size: [u32; 2],
    pub filter_mode: FilterMode,
    /// Display mode: Fit (black bars) or AmbientFit (blurred background fills letterbox)
    pub fit_mode: FitMode,
    /// Mip LOD level for ambient fit blur (higher = blurrier, default 5.0)
    #[validate(range(min = 0.0, max = 10.0))]
    pub ambient_blur: f32,
}

fn validate_timer(value: f32) -> std::result::Result<(), validator::ValidationError> {
    if !value.is_finite() {
        let mut err = validator::ValidationError::new("timer_finite");
        err.message = Some(std::borrow::Cow::Borrowed("timer must be a finite number"));
        return Err(err);
    }
    if value == 0.0 || value >= TIMER_MIN {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("timer_range");
        err.message = Some(std::borrow::Cow::Owned(format!(
            "timer must be 0.0 (paused) or >= {TIMER_MIN} seconds"
        )));
        Err(err)
    }
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
            playback_mode: PlaybackMode::Slideshow,
            sequence_fps: 24.0,
            max_texture_size: [1920, 1080],
            filter_mode: FilterMode::Linear,
            fit_mode: FitMode::Fit,
            ambient_blur: 5.0,
        }
    }
}

/// A validated transition mode index in the range `0..=19`.
///
/// Enforces the range invariant at construction time via [`TryFrom<i32>`].
/// Serializes and deserializes as a plain integer for TOML compatibility.
///
/// The mode index must stay in sync with the WGSL router and the mode list
/// comment at the top of `assets/shaders/transition.wgsl`. When adding or
/// renaming a mode, update both files together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransitionMode(i32);

impl TransitionMode {
    /// The minimum valid mode index.
    pub const MIN: i32 = 0;
    /// The maximum valid mode index.
    pub const MAX: i32 = 19;

    /// Returns the inner `i32` value.
    #[inline]
    pub fn value(self) -> i32 {
        self.0
    }

    /// Human-readable name for this transition mode.
    ///
    /// Must stay in sync with the WGSL router in `assets/shaders/transition.wgsl`.
    pub fn name(self) -> &'static str {
        match self.0 {
            0 => "Crossfade",
            1 => "Smooth Crossfade",
            2 => "Roll from Top",
            3 => "Roll from Bottom",
            4 => "Roll from Left",
            5 => "Roll from Right",
            6 => "Roll from Top-Left",
            7 => "Roll from Top-Right",
            8 => "Roll from Bottom-Left",
            9 => "Roll from Bottom-Right",
            10 => "Sliding Door Open",
            11 => "Sliding Door Close",
            12 => "Blinds H Open",
            13 => "Blinds H Close",
            14 => "Blinds V Open",
            15 => "Blinds V Close",
            16 => "Box Expand",
            17 => "Box Contract",
            18 => "Random Squares",
            19 => "Angular Wipe",
            _ => "Unknown",
        }
    }
}

impl TryFrom<i32> for TransitionMode {
    type Error = SldshowError;

    fn try_from(value: i32) -> std::result::Result<Self, Self::Error> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(SldshowError::InvalidTransitionMode(value))
        }
    }
}

impl From<TransitionMode> for i32 {
    fn from(m: TransitionMode) -> i32 {
        m.0
    }
}

impl Serialize for TransitionMode {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for TransitionMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = i32::deserialize(d)?;
        TransitionMode::try_from(value).map_err(serde::de::Error::custom)
    }
}

/// Transition configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct TransitionConfig {
    #[validate(range(min = 0.0, max = 10.0))]
    pub time: f32,
    pub random: bool,
    pub mode: TransitionMode,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            time: 0.5,
            random: true,
            mode: TransitionMode::default(),
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
            bg_color: [10, 10, 10, 255], // Dark gray background
            show_image_path: false,
            show_controls: true,
            font_family: Some("Inter".to_string()),
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
            SldshowError::ConfigSaveError {
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
        let toml_str = toml::to_string(&config).expect("default Config is serializable");
        let deserialized: Config =
            toml::from_str(&toml_str).expect("serialized Config is valid TOML");

        assert_eq!(config.window.width, deserialized.window.width);
        assert_eq!(config.viewer.timer, deserialized.viewer.timer);
    }

    #[test]
    fn test_transition_mode_valid_range() {
        assert!(TransitionMode::try_from(0).is_ok());
        assert!(TransitionMode::try_from(19).is_ok());
        assert!(TransitionMode::try_from(10).is_ok());
    }

    #[test]
    fn test_transition_mode_invalid_range() {
        assert!(TransitionMode::try_from(-1).is_err());
        assert!(TransitionMode::try_from(20).is_err());
        assert!(TransitionMode::try_from(100).is_err());
    }

    #[test]
    fn test_transition_mode_roundtrip() {
        let m = TransitionMode::try_from(7).expect("7 is within 0..=19");
        assert_eq!(i32::from(m), 7);
        assert_eq!(m.value(), 7);
    }

    #[test]
    fn test_transition_mode_serde() {
        // Test round-trip through TransitionConfig (TOML requires a table at root)
        let config = TransitionConfig {
            mode: TransitionMode::try_from(5).expect("5 is within 0..=19"),
            ..TransitionConfig::default()
        };
        let serialized = toml::to_string(&config).expect("TransitionConfig is serializable");
        let deserialized: TransitionConfig =
            toml::from_str(&serialized).expect("serialized TransitionConfig is valid TOML");
        assert_eq!(config.mode, deserialized.mode);
    }

    #[test]
    fn test_transition_mode_serde_invalid() {
        let result: std::result::Result<TransitionConfig, _> =
            toml::from_str("mode = 99\ntime = 0.5\nrandom = false");
        assert!(result.is_err());
    }

    #[test]
    fn test_save_error() {
        let config = Config::default();
        // Trying to save to a path where parent directory likely doesn't exist
        let path = Utf8PathBuf::from("non_existent_dir_12345/config.toml");
        let result = config.save(&path);

        match result {
            Err(SldshowError::ConfigSaveError { path: p, source: _ }) => {
                assert_eq!(p, path);
            }
            _ => panic!("Expected ConfigSaveError, got {:?}", result),
        }
    }

    #[test]
    fn test_fitmode_toggle() {
        let mut mode = FitMode::Fit;
        mode.toggle();
        assert_eq!(mode, FitMode::AmbientFit);
        mode.toggle();
        assert_eq!(mode, FitMode::Fit);
    }

    #[test]
    fn test_fitmode_uniform_values() {
        assert_eq!(FitMode::Fit.to_uniform_value(), 0);
        assert_eq!(FitMode::AmbientFit.to_uniform_value(), 1);
    }

    #[test]
    fn test_bg_color_f32_conversion() {
        let mut config = Config::default();
        config.style.bg_color = [0, 128, 255, 255];
        let c = config.bg_color_f32();
        assert_eq!(c[0], 0.0);
        assert!((c[1] - 128.0 / 255.0).abs() < 1e-6);
        assert!((c[2] - 1.0).abs() < 1e-6);
        assert_eq!(c[3], 1.0);
    }

    #[test]
    fn test_transition_mode_all_names_known() {
        for i in TransitionMode::MIN..=TransitionMode::MAX {
            let mode = TransitionMode::try_from(i).expect("i is within MIN..=MAX");
            assert_ne!(
                mode.name(),
                "Unknown",
                "TransitionMode({i}) returned Unknown — update name() to match"
            );
        }
    }

    #[test]
    fn test_window_config_defaults() {
        let w = WindowConfig::default();
        assert_eq!(w.width, 1280);
        assert_eq!(w.height, 720);
        assert!(!w.fullscreen);
        assert!(!w.always_on_top);
        assert!(w.decorations);
    }

    #[test]
    fn test_timer_validation_zero_is_valid() {
        let mut config = ViewerConfig::default();
        config.timer = 0.0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_timer_validation_above_minimum_is_valid() {
        let mut config = ViewerConfig::default();
        config.timer = 0.1;
        assert!(config.validate().is_ok());
        config.timer = 5.0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_timer_validation_between_zero_and_minimum_is_invalid() {
        let mut config = ViewerConfig::default();
        config.timer = 0.05;
        assert!(config.validate().is_err());
        config.timer = 0.01;
        assert!(config.validate().is_err());
        config.timer = 0.099;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_timer_validation_negative_is_invalid() {
        let mut config = ViewerConfig::default();
        config.timer = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_timer_validation_non_finite_is_invalid() {
        let mut config = ViewerConfig::default();
        config.timer = f32::INFINITY;
        assert!(config.validate().is_err());
        config.timer = f32::NEG_INFINITY;
        assert!(config.validate().is_err());
        config.timer = f32::NAN;
        assert!(config.validate().is_err());
    }
}
