use anyhow::{Context, Result};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Resource, Default)]
pub struct Config {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub viewer: ViewerConfig,
    #[serde(default)]
    pub transition: TransitionConfig,
    #[serde(default)]
    pub style: StyleConfig,
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub decorations: bool,
    #[serde(default)]
    pub resizable: bool,
    #[serde(default)]
    pub monitor_index: usize,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            fullscreen: false,
            always_on_top: false,
            decorations: true,
            resizable: false,
            monitor_index: 0,
        }
    }
}

/// Viewer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerConfig {
    #[serde(default)]
    pub image_paths: Vec<PathBuf>,
    #[serde(default = "default_timer")]
    pub timer: f32,
    #[serde(default = "default_true")]
    pub scan_subfolders: bool,
    #[serde(default = "default_true")]
    pub shuffle: bool,
    #[serde(default)]
    pub pause_at_last: bool,
    #[serde(default = "default_cache_extent")]
    pub cache_extent: usize,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            image_paths: Vec::new(),
            timer: default_timer(),
            scan_subfolders: true,
            shuffle: true,
            pause_at_last: false,
            cache_extent: default_cache_extent(),
        }
    }
}

/// Transition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    #[serde(default = "default_transition_time")]
    pub time: f32,
    #[serde(default = "default_true")]
    pub random: bool,
    #[serde(default)]
    pub mode: i32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            time: default_transition_time(),
            random: true,
            mode: 0,
        }
    }
}

/// Style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    #[serde(default = "default_bg_color")]
    pub bg_color: [u8; 4],
    #[serde(default)]
    pub show_image_path: bool,
    #[serde(default = "default_true")]
    pub show_controls: bool,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            bg_color: default_bg_color(),
            show_image_path: false,
            show_controls: true,
        }
    }
}

// Default value functions
fn default_width() -> u32 {
    1280
}

fn default_height() -> u32 {
    720
}

fn default_timer() -> f32 {
    10.0
}

fn default_cache_extent() -> usize {
    5
}

fn default_transition_time() -> f32 {
    0.5
}

fn default_bg_color() -> [u8; 4] {
    [0, 0, 0, 255]
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load configuration from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.as_ref().display()))?;

        Ok(config)
    }

    /// Load configuration from default locations
    /// 1. Command line argument
    /// 2. ~/.sldshow
    /// 3. Default config
    pub fn load_default(config_path: Option<PathBuf>) -> Result<Self> {
        // Try command line argument first
        if let Some(path) = config_path {
            if path.exists() {
                return Self::load(&path);
            } else {
                anyhow::bail!("Config file not found: {}", path.display());
            }
        }

        // Try home directory
        if let Some(home) = dirs::home_dir() {
            let home_config = home.join(".sldshow");
            if home_config.exists() {
                return Self::load(&home_config);
            }
        }

        // Use default
        Ok(Self::default())
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        std::fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

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
