// Transition shader for sldshow2
// Ported from original sldshow with updated WGSL syntax
//
// Mode indices assigned here must stay in sync with TransitionMode::name()
// and TransitionMode::MAX in src/config.rs.

// Vertex output structure
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Vertex shader
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen quad from vertex index
    // 0: (-1, -1), 1: (3, -1), 2: (-1, 3) -> covers screen
    let x = f32(i32(in_vertex_index) & 1);
    let y = f32(i32(in_vertex_index) >> 1);
    out.uv = vec2<f32>(x * 2.0, y * 2.0);
    out.position = vec4<f32>(x * 4.0 - 1.0, 1.0 - y * 4.0, 0.0, 1.0);
    return out;
}

struct TransitionUniform {
    blend: f32,
    mode: i32,
    aspect_ratio: vec2<f32>,
    bg_color: vec4<f32>,
    window_size: vec2<f32>,
    image_a_size: vec2<f32>,
    image_b_size: vec2<f32>,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    saturation: f32,
    fit_mode: i32,     // 0 = Fit (black bars), 1 = AmbientFit (blurred background)
    ambient_blur: f32, // Mip LOD level for ambient fit blur (default: 5.0)
    zoom_scale: f32,   // 1.0 = no zoom; > 1.0 = zoomed in
    zoom_pan_x: f32,     // UV-space pan offset X (split to avoid vec2 alignment padding)
    zoom_pan_y: f32,     // UV-space pan offset Y
    display_mode: i32,   // 0 = SDR (clamp to [0,1]), 1 = HDR (allow > 1.0)
    sdr_scale_a: f32,    // SDR brightness scale for texture A (1.0 or ~2.54)
    sdr_scale_b: f32,    // SDR brightness scale for texture B (1.0 or ~2.54)
}

@group(0) @binding(0)
var<uniform> material: TransitionUniform;

@group(0) @binding(1)
var texture_a: texture_2d<f32>;

@group(0) @binding(2)
var sampler_a: sampler;

@group(0) @binding(3)
var texture_b: texture_2d<f32>;

@group(0) @binding(4)
var sampler_b: sampler;

// Transition effect functions
const TRANSITION_MAX_MODE_IDX: i32 = 19;
const PI: f32 = 3.141592653589793;

// Apply zoom/pan transform to screen UV before contain-fit scaling.
// zoom_scale > 1.0 zooms in (narrows the visible UV window).
// zoom_pan shifts the center of the visible region.
fn apply_zoom(uv: vec2<f32>) -> vec2<f32> {
    let scale = max(material.zoom_scale, 1.0);
    // Map uv from [0,1] into the zoomed window centered at (0.5 + pan)
    let center = vec2<f32>(0.5) + vec2<f32>(material.zoom_pan_x, material.zoom_pan_y);
    return (uv - center) / scale + center;
}

// UV adjustment: contain-fit (letterbox/pillarbox)
fn adjust_uv(uv: vec2<f32>, image_size: vec2<f32>, window_size: vec2<f32>) -> vec2<f32> {
    let zoomed_uv = apply_zoom(uv);

    let img_aspect = image_size.x / image_size.y;
    let win_aspect = window_size.x / window_size.y;

    var scale: vec2<f32>;
    if img_aspect > win_aspect {
        // Wide image: fit to width, letterbox top/bottom
        scale = vec2<f32>(1.0, win_aspect / img_aspect);
    } else {
        // Tall image: fit to height, letterbox left/right
        scale = vec2<f32>(img_aspect / win_aspect, 1.0);
    }

    // Apply scale centered at (0.5, 0.5)
    let adjusted = (zoomed_uv - 0.5) / scale + 0.5;
    return adjusted;
}

// UV adjustment: cover-fit (fill viewport, crop excess)
fn adjust_uv_cover(uv: vec2<f32>, image_size: vec2<f32>, window_size: vec2<f32>) -> vec2<f32> {
    let img_aspect = image_size.x / image_size.y;
    let win_aspect = window_size.x / window_size.y;

    var scale: vec2<f32>;
    if img_aspect > win_aspect {
        scale = vec2<f32>(img_aspect / win_aspect, 1.0);
    } else {
        scale = vec2<f32>(1.0, win_aspect / img_aspect);
    }

    return (uv - 0.5) / scale + 0.5;
}

fn is_uv_in_bounds(uv: vec2<f32>) -> bool {
    return uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
}

// Blurred cover-fit sampling for ambient background using mipmaps
fn sample_ambient_bg(tex: texture_2d<f32>, smp: sampler, uv: vec2<f32>,
                     image_size: vec2<f32>, window_size: vec2<f32>) -> vec4<f32> {
    let cover_uv = adjust_uv_cover(uv, image_size, window_size);
    let max_lod = f32(textureNumLevels(tex)) - 1.0;
    let lod = min(material.ambient_blur, max_lod);
    // 3x3 tap at 1.5 texel offsets to break up mip texel grid (9 taps)
    let texel_step = pow(2.0, lod) / max(image_size.x, image_size.y) * 1.5;
    var color = vec4<f32>(0.0);
    for (var i: i32 = -1; i <= 1; i = i + 1) {
        for (var j: i32 = -1; j <= 1; j = j + 1) {
            let offset = vec2<f32>(f32(i), f32(j)) * texel_step;
            let suv = clamp(cover_uv + offset, vec2<f32>(0.001), vec2<f32>(0.999));
            color = color + textureSampleLevel(tex, smp, suv, lod);
        }
    }
    color = color / 9.0;
    // Darken and slightly desaturate for ambient effect
    let lum = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = vec4<f32>(mix(color.rgb, vec3<f32>(lum), 0.3) * 0.7, 1.0);
    // Vignette: fade to black toward screen edges
    let center_dist = length((uv - 0.5) * 2.0);
    let vignette = 1.0 - 0.6 * smoothstep(0.3, 1.2, center_dist);
    return vec4<f32>(color.rgb * vignette, 1.0);
}

// Unified sampling: contain-fit image with ambient background or solid color for out-of-bounds.
// The scale is applied only to actual texture/ambient samples, not to the solid bg_color fallback.
fn sample_with_fit_scaled(tex: texture_2d<f32>, smp: sampler, uv: vec2<f32>,
                          image_size: vec2<f32>, window_size: vec2<f32>,
                          scale: f32) -> vec4<f32> {
    let fit_uv = adjust_uv(uv, image_size, window_size);
    if is_uv_in_bounds(fit_uv) {
        let c = textureSample(tex, smp, fit_uv);
        return vec4<f32>(c.rgb * scale, c.a);
    } else if material.fit_mode == 1 {
        let c = sample_ambient_bg(tex, smp, uv, image_size, window_size);
        return vec4<f32>(c.rgb * scale, c.a);
    } else {
        return material.bg_color;
    }
}

// Per-texture sampling with SDR brightness compensation on HDR swapchains.
fn sample_a(uv: vec2<f32>) -> vec4<f32> {
    return sample_with_fit_scaled(texture_a, sampler_a, uv,
        material.image_a_size, material.window_size, material.sdr_scale_a);
}

fn sample_b(uv: vec2<f32>) -> vec4<f32> {
    return sample_with_fit_scaled(texture_b, sampler_b, uv,
        material.image_b_size, material.window_size, material.sdr_scale_b);
}

// 0: Basic crossfade
fn ts_crossfading(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let color_a = sample_a(uv);
    let color_b = sample_b(uv);
    return mix(color_a, color_b, progress);
}

// 1: Smooth crossfade with smoothstep
fn ts_smooth_crossfading(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let color_a = sample_a(uv);
    let color_b = sample_b(uv);
    let smooth_progress = smoothstep(0.0, 1.0, progress);
    return mix(color_a, color_b, smooth_progress);
}

// 2-9: Roll transitions (from various directions)
fn ts_roll(uv: vec2<f32>, progress: f32, direction: i32) -> vec4<f32> {
    var threshold: f32;

    if direction == 0 { // from top
        threshold = uv.y;
    } else if direction == 1 { // from bottom
        threshold = 1.0 - uv.y;
    } else if direction == 2 { // from left
        threshold = uv.x;
    } else if direction == 3 { // from right
        threshold = 1.0 - uv.x;
    } else if direction == 4 { // from top-left
        threshold = (uv.x + uv.y) * 0.5;
    } else if direction == 5 { // from top-right
        threshold = (1.0 - uv.x + uv.y) * 0.5;
    } else if direction == 6 { // from bottom-left
        threshold = (uv.x + 1.0 - uv.y) * 0.5;
    } else { // from bottom-right
        threshold = (1.0 - uv.x + 1.0 - uv.y) * 0.5;
    }

    if progress > threshold {
        return sample_b(uv);
    } else {
        return sample_a(uv);
    }
}

// 10-11: Sliding door (open/close)
fn ts_sliding_door(uv: vec2<f32>, progress: f32, opening: bool) -> vec4<f32> {
    let center_distance = abs(uv.x - 0.5) * 2.0;

    if opening {
        // Open: B expands from center outward
        if center_distance < progress {
            return sample_b(uv);
        } else {
            return sample_a(uv);
        }
    } else {
        // Close: B appears from edges inward
        if center_distance > (1.0 - progress) {
            return sample_b(uv);
        } else {
            return sample_a(uv);
        }
    }
}

// 12-15: Blind effects (horizontal/vertical, open outward/close inward)
fn ts_blind(uv: vec2<f32>, progress: f32, direction: i32) -> vec4<f32> {
    let slices = 10.0;
    var slice_progress: f32;
    var open_outward: bool;

    if direction == 0 || direction == 1 { // horizontal
        slice_progress = fract(uv.y * slices);
        open_outward = direction == 0;
    } else { // vertical
        slice_progress = fract(uv.x * slices);
        open_outward = direction == 2;
    }

    var show_new: bool;
    if open_outward {
        show_new = slice_progress < progress;
    } else {
        show_new = slice_progress > (1.0 - progress);
    }

    if show_new {
        return sample_b(uv);
    } else {
        return sample_a(uv);
    }
}

// 16-17: Box transition (expand/contract)
fn ts_box(uv: vec2<f32>, progress: f32, expanding: bool) -> vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = max(abs(uv.x - center.x), abs(uv.y - center.y)) * 2.0;

    var show_new: bool;
    if expanding {
        show_new = dist < progress;
    } else {
        show_new = dist > (1.0 - progress);
    }

    if show_new {
        return sample_b(uv);
    } else {
        return sample_a(uv);
    }
}

// 18: Random squares (from GL Transitions, MIT license)
fn ts_randomsquares(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let size = vec2<f32>(10.0, 10.0);
    let smoothness = 0.5;

    let r = fract(sin(dot(floor(uv * size), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let m = smoothstep(0.0, -smoothness, r - (progress * (1.0 + smoothness)));

    let color_a = sample_a(uv);
    let color_b = sample_b(uv);

    return mix(color_a, color_b, m);
}

// 19: Angular wipe (from GL Transitions, MIT license)
fn ts_angular(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let offset = 90.0;
    let center = vec2<f32>(0.5, 0.5);

    // Calculate angle from center, range: [-π, π]
    var angle = atan2(uv.y - center.y, uv.x - center.x);

    // Add offset and normalize to [0, 1]
    angle = angle + radians(offset);

    // Normalize angle to [0, 1] range
    // First shift from [-π, π] to [0, 2π], then divide by 2π
    var normalized_angle = (angle + PI) / (2.0 * PI);

    // Ensure the angle wraps around correctly (handles values outside [0, 1])
    normalized_angle = fract(normalized_angle);

    if normalized_angle - progress > 0.0 {
        return sample_a(uv);
    } else {
        return sample_b(uv);
    }
}

// Color adjustment post-processing (mpv-like: brightness, contrast, gamma, saturation)
fn apply_color_adjustments(color: vec4<f32>) -> vec4<f32> {
    var c = color.rgb;

    // Contrast (around 0.5 midpoint) — applied first to preserve dynamic range
    c = (c - 0.5) * material.contrast + 0.5;

    // Brightness (additive offset) — after contrast
    c = c + vec3<f32>(material.brightness);

    // Gamma correction
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / material.gamma));

    // Saturation (blend with luminance)
    let luminance = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(luminance), c, material.saturation);

    // Clamp to valid range (SDR) or pass through (HDR)
    if material.display_mode == 0 {
        c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    } else {
        c = max(c, vec3<f32>(0.0));
    }

    return vec4<f32>(c, color.a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let progress = material.blend;
    let mode = material.mode;

    // Early exit optimization for static images (no blending needed)
    if progress <= 0.0 {
        return apply_color_adjustments(sample_a(in.uv));
    }

    if progress >= 1.0 {
        return apply_color_adjustments(sample_b(in.uv));
    }

    // Route to appropriate transition effect.
    // Mode assignments mirror TransitionMode::name() in src/config.rs.
    var result: vec4<f32>;
    if mode == 0 {
        result = ts_crossfading(in.uv, progress);
    } else if mode == 1 {
        result = ts_smooth_crossfading(in.uv, progress);
    } else if mode >= 2 && mode <= 9 {
        result = ts_roll(in.uv, progress, mode - 2);
    } else if mode == 10 {
        result = ts_sliding_door(in.uv, progress, true);
    } else if mode == 11 {
        result = ts_sliding_door(in.uv, progress, false);
    } else if mode >= 12 && mode <= 15 {
        result = ts_blind(in.uv, progress, mode - 12);
    } else if mode == 16 {
        result = ts_box(in.uv, progress, true);
    } else if mode == 17 {
        result = ts_box(in.uv, progress, false);
    } else if mode == 18 {
        result = ts_randomsquares(in.uv, progress);
    } else if mode == 19 {
        result = ts_angular(in.uv, progress);
    } else {
        // Default to crossfade
        result = ts_crossfading(in.uv, progress);
    }

    return apply_color_adjustments(result);
}
