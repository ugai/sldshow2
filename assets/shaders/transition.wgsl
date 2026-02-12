// Transition shader for sldshow2
// Ported from original sldshow with updated WGSL syntax
// 22 different transition effects

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

// UV adjustment helper functions for letterboxing
fn adjust_uv(uv: vec2<f32>, image_size: vec2<f32>, window_size: vec2<f32>) -> vec2<f32> {
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
    let adjusted = (uv - 0.5) / scale + 0.5;
    return adjusted;
}

fn is_uv_in_bounds(uv: vec2<f32>) -> bool {
    return uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
}

// 0: Basic crossfade
fn ts_crossfading(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    // Adjust UVs for letterboxing
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    var color_a: vec4<f32>;
    var color_b: vec4<f32>;

    // Sample texture or use background color if out of bounds
    if is_uv_in_bounds(uv_a) {
        color_a = textureSample(texture_a, sampler_a, uv_a);
    } else {
        color_a = material.bg_color;
    }

    if is_uv_in_bounds(uv_b) {
        color_b = textureSample(texture_b, sampler_b, uv_b);
    } else {
        color_b = material.bg_color;
    }

    return mix(color_a, color_b, progress);
}

// 1: Smooth crossfade with smoothstep
fn ts_smooth_crossfading(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    var color_a: vec4<f32>;
    var color_b: vec4<f32>;

    if is_uv_in_bounds(uv_a) {
        color_a = textureSample(texture_a, sampler_a, uv_a);
    } else {
        color_a = material.bg_color;
    }

    if is_uv_in_bounds(uv_b) {
        color_b = textureSample(texture_b, sampler_b, uv_b);
    } else {
        color_b = material.bg_color;
    }

    let smooth_progress = smoothstep(0.0, 1.0, progress);
    return mix(color_a, color_b, smooth_progress);
}

// 2-9: Roll transitions (from various directions)
fn ts_roll(uv: vec2<f32>, progress: f32, direction: i32) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

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

    var color: vec4<f32>;
    if progress > threshold {
        if is_uv_in_bounds(uv_b) {
            color = textureSample(texture_b, sampler_b, uv_b);
        } else {
            color = material.bg_color;
        }
    } else {
        if is_uv_in_bounds(uv_a) {
            color = textureSample(texture_a, sampler_a, uv_a);
        } else {
            color = material.bg_color;
        }
    }

    return color;
}

// 10-11: Sliding door (open/close)
fn ts_sliding_door(uv: vec2<f32>, progress: f32, opening: bool) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    let center_distance = abs(uv.x - 0.5) * 2.0;
    var threshold: f32;

    if opening {
        threshold = progress;
    } else {
        threshold = 1.0 - progress;
    }

    var color: vec4<f32>;
    if center_distance < threshold {
        if is_uv_in_bounds(uv_b) {
            color = textureSample(texture_b, sampler_b, uv_b);
        } else {
            color = material.bg_color;
        }
    } else {
        if is_uv_in_bounds(uv_a) {
            color = textureSample(texture_a, sampler_a, uv_a);
        } else {
            color = material.bg_color;
        }
    }

    return color;
}

// 12-15: Blind effects (horizontal/vertical)
fn ts_blind(uv: vec2<f32>, progress: f32, direction: i32) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    let slices = 10.0;
    var slice_progress: f32;

    if direction == 0 || direction == 1 { // horizontal
        let slice_idx = floor(uv.y * slices);
        slice_progress = fract(uv.y * slices);
    } else { // vertical
        let slice_idx = floor(uv.x * slices);
        slice_progress = fract(uv.x * slices);
    }

    var color: vec4<f32>;
    if slice_progress < progress {
        if is_uv_in_bounds(uv_b) {
            color = textureSample(texture_b, sampler_b, uv_b);
        } else {
            color = material.bg_color;
        }
    } else {
        if is_uv_in_bounds(uv_a) {
            color = textureSample(texture_a, sampler_a, uv_a);
        } else {
            color = material.bg_color;
        }
    }

    return color;
}

// 16-17: Box transition (expand/contract)
fn ts_box(uv: vec2<f32>, progress: f32, expanding: bool) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    let center = vec2<f32>(0.5, 0.5);
    let dist = max(abs(uv.x - center.x), abs(uv.y - center.y)) * 2.0;

    var show_new: bool;
    if expanding {
        show_new = dist < progress;
    } else {
        show_new = dist > (1.0 - progress);
    }

    var color: vec4<f32>;
    if show_new {
        if is_uv_in_bounds(uv_b) {
            color = textureSample(texture_b, sampler_b, uv_b);
        } else {
            color = material.bg_color;
        }
    } else {
        if is_uv_in_bounds(uv_a) {
            color = textureSample(texture_a, sampler_a, uv_a);
        } else {
            color = material.bg_color;
        }
    }

    return color;
}

// 18: Random squares (from GL Transitions, MIT license)
fn ts_randomsquares(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

    let size = vec2<f32>(10.0, 10.0);
    let smoothness = 0.5;

    let r = fract(sin(dot(floor(uv * size), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let m = smoothstep(0.0, -smoothness, r - (progress * (1.0 + smoothness)));

    var color_a: vec4<f32>;
    var color_b: vec4<f32>;

    if is_uv_in_bounds(uv_a) {
        color_a = textureSample(texture_a, sampler_a, uv_a);
    } else {
        color_a = material.bg_color;
    }

    if is_uv_in_bounds(uv_b) {
        color_b = textureSample(texture_b, sampler_b, uv_b);
    } else {
        color_b = material.bg_color;
    }

    return mix(color_a, color_b, m);
}

// 19: Angular wipe (from GL Transitions, MIT license)
fn ts_angular(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let uv_a = adjust_uv(uv, material.image_a_size, material.window_size);
    let uv_b = adjust_uv(uv, material.image_b_size, material.window_size);

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

    var color: vec4<f32>;
    if normalized_angle - progress > 0.0 {
        if is_uv_in_bounds(uv_a) {
            color = textureSample(texture_a, sampler_a, uv_a);
        } else {
            color = material.bg_color;
        }
    } else {
        if is_uv_in_bounds(uv_b) {
            color = textureSample(texture_b, sampler_b, uv_b);
        } else {
            color = material.bg_color;
        }
    }

    return color;
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

    // Clamp to valid range
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(c, color.a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let progress = material.blend;
    let mode = material.mode;

    // Early exit optimization for static images (no blending needed)
    if progress <= 0.0 {
        // Show only image A
        let uv_a = adjust_uv(in.uv, material.image_a_size, material.window_size);
        if is_uv_in_bounds(uv_a) {
            return apply_color_adjustments(textureSample(texture_a, sampler_a, uv_a));
        } else {
            return material.bg_color;
        }
    }

    if progress >= 1.0 {
        // Show only image B
        let uv_b = adjust_uv(in.uv, material.image_b_size, material.window_size);
        if is_uv_in_bounds(uv_b) {
            return apply_color_adjustments(textureSample(texture_b, sampler_b, uv_b));
        } else {
            return material.bg_color;
        }
    }

    // Route to appropriate transition effect
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
