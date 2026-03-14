//! Color adjustment parameters (mpv-like) for brightness, contrast, gamma, and saturation.

use winit::keyboard::KeyCode;

/// Color adjustment parameters (mpv-like).
///
/// Each field ranges from -100 to 100, where 0 is the neutral default.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorAdjustments {
    pub brightness: i32,
    pub contrast: i32,
    pub gamma: i32,
    pub saturation: i32,
}

/// Result of a color key press: the display name and the new value.
pub struct ColorKeyResult {
    pub name: &'static str,
    pub value: i32,
}

impl ColorAdjustments {
    /// Convert brightness (-100..100) to shader float: value / 100.0
    pub fn shader_brightness(&self) -> f32 {
        self.brightness as f32 / 100.0
    }

    /// Convert contrast (-100..100) to shader float: (value + 100) / 100.0
    pub fn shader_contrast(&self) -> f32 {
        (self.contrast + 100) as f32 / 100.0
    }

    /// Convert gamma (-100..100) to shader float: exp(ln(8) * value / 100.0)
    pub fn shader_gamma(&self) -> f32 {
        (8.0_f32.ln() * self.gamma as f32 / 100.0).exp()
    }

    /// Convert saturation (-100..100) to shader float: (value + 100) / 100.0
    pub fn shader_saturation(&self) -> f32 {
        (self.saturation + 100) as f32 / 100.0
    }

    /// Handle a digit-key color adjustment. Returns the name and new value if
    /// the key maps to a color parameter, or `None` for unrecognized keys.
    pub fn handle_key(&mut self, key: KeyCode) -> Option<ColorKeyResult> {
        let (value, delta, name) = match key {
            KeyCode::Digit1 => (&mut self.contrast, -1i32, "Contrast"),
            KeyCode::Digit2 => (&mut self.contrast, 1, "Contrast"),
            KeyCode::Digit3 => (&mut self.brightness, -1, "Brightness"),
            KeyCode::Digit4 => (&mut self.brightness, 1, "Brightness"),
            KeyCode::Digit5 => (&mut self.gamma, -1, "Gamma"),
            KeyCode::Digit6 => (&mut self.gamma, 1, "Gamma"),
            KeyCode::Digit7 => (&mut self.saturation, -1, "Saturation"),
            KeyCode::Digit8 => (&mut self.saturation, 1, "Saturation"),
            _ => return None,
        };
        *value = (*value + delta).clamp(-100, 100);
        Some(ColorKeyResult {
            name,
            value: *value,
        })
    }

    /// Append non-zero adjustment values to the given string.
    pub fn append_info(&self, info: &mut String) {
        if self.contrast != 0 {
            info.push_str(&format!("\nContrast: {}", self.contrast));
        }
        if self.brightness != 0 {
            info.push_str(&format!("\nBrightness: {}", self.brightness));
        }
        if self.gamma != 0 {
            info.push_str(&format!("\nGamma: {}", self.gamma));
        }
        if self.saturation != 0 {
            info.push_str(&format!("\nSaturation: {}", self.saturation));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_neutral() {
        let c = ColorAdjustments::default();
        assert_eq!(c.shader_brightness(), 0.0);
        assert!((c.shader_contrast() - 1.0).abs() < f32::EPSILON);
        assert!((c.shader_gamma() - 1.0).abs() < f32::EPSILON);
        assert!((c.shader_saturation() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn shader_brightness_range() {
        let c = ColorAdjustments {
            brightness: 100,
            ..Default::default()
        };
        assert!((c.shader_brightness() - 1.0).abs() < f32::EPSILON);

        let c = ColorAdjustments {
            brightness: -100,
            ..Default::default()
        };
        assert!((c.shader_brightness() - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn handle_key_clamps() {
        let mut c = ColorAdjustments {
            contrast: 100,
            ..Default::default()
        };
        let result = c
            .handle_key(KeyCode::Digit2)
            .expect("Digit2 maps to a color key");
        assert_eq!(result.name, "Contrast");
        assert_eq!(result.value, 100); // clamped at max
    }

    #[test]
    fn handle_key_unknown() {
        let mut c = ColorAdjustments::default();
        assert!(c.handle_key(KeyCode::KeyA).is_none());
    }

    #[test]
    fn append_info_only_nonzero() {
        let c = ColorAdjustments {
            brightness: 10,
            saturation: -5,
            ..Default::default()
        };
        let mut s = String::new();
        c.append_info(&mut s);
        assert!(s.contains("Brightness: 10"));
        assert!(s.contains("Saturation: -5"));
        assert!(!s.contains("Contrast"));
        assert!(!s.contains("Gamma"));
    }
}
