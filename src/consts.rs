//! Constants for embedded assets and configuration
//!
//! Centralized location for magic numbers and fixed asset handles.

use bevy::prelude::*;
use bevy::asset::uuid_handle;

/// Handle for the embedded M PLUS 2 font
///
/// This font is embedded in the binary using include_bytes! and
/// assigned a fixed weak handle for standalone distribution.
pub const EMBEDDED_FONT_HANDLE: Handle<Font> = uuid_handle!("abcd1234-5678-490e-f000-000000000001");

/// Handle for the embedded transition shader
///
/// The WGSL shader code is embedded in the binary and assigned
/// a fixed weak handle to ensure consistent asset loading.
pub const TRANSITION_SHADER_HANDLE: Handle<Shader> = uuid_handle!("12345678-9abc-4def-0000-000000000002");
