//! Constants for embedded assets and configuration
//!
//! Centralized location for magic numbers and fixed asset handles.

use bevy::prelude::*;
use bevy::render::render_resource::Shader;

/// Handle for the embedded M PLUS 2 font
///
/// This font is embedded in the binary using include_bytes! and
/// assigned a fixed weak handle for standalone distribution.
pub const EMBEDDED_FONT_HANDLE: Handle<Font> = Handle::weak_from_u128(0xabcd_1234_5678_90ef);

/// Handle for the embedded transition shader
///
/// The WGSL shader code is embedded in the binary and assigned
/// a fixed weak handle to ensure consistent asset loading.
pub const TRANSITION_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x1234_5678_9abc_def0);
