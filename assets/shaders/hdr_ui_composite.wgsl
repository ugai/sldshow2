// Composites the egui UI overlay (rendered to an Rgba8Unorm intermediate texture)
// onto the HDR Rgba16Float swapchain at SDR reference-white brightness.
//
// egui outputs linear values in [0, 1], which on a Rgba16Float swapchain maps to
// [0, 80 nit].  The main transition shader raises SDR content to the 203-nit
// reference white by multiplying by SDR_WHITE_SCALE.  This pass applies the same
// scale so that UI elements (including gallery thumbnails) appear at the same
// perceived brightness as images in the main view.
//
// The intermediate texture stores premultiplied-alpha linear values written by
// egui_wgpu, so the composite blend state is also premultiplied-alpha.

// SDR reference white on an scRGB (Rgba16Float) swapchain: 203 nits / 80 nits.
const SDR_WHITE_SCALE: f32 = 203.0 / 80.0;

@group(0) @binding(0) var t_egui: texture_2d<f32>;
@group(0) @binding(1) var s_egui: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen triangle: three vertices cover the entire clip-space quad.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let xs = array<f32, 3>(-1.0, 3.0, -1.0);
    let ys = array<f32, 3>(-1.0, -1.0, 3.0);
    let x = xs[vi];
    let y = ys[vi];
    var out: VertexOutput;
    out.clip_pos = vec4<f32>(x, y, 0.0, 1.0);
    // Texture UV: (0,0) = top-left; clip Y = +1 → UV.y = 0.
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(t_egui, s_egui, in.uv);
    // c.rgb is premultiplied linear (egui_wgpu writes linear to Rgba8Unorm).
    // Scale RGB to HDR reference white; alpha is unchanged.
    return vec4<f32>(c.rgb * SDR_WHITE_SCALE, c.a);
}
