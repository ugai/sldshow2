//! Shared GPU test infrastructure for offscreen transition shader tests.
//!
//! Key helpers:
//! - [`try_setup_gpu`] — obtains a wgpu device/queue without a surface; returns
//!   `None` if no adapter is available so tests can skip gracefully in CI.
//! - [`create_solid_color_texture`] — uploads a 1×1 solid-colour RGBA8 texture.
//! - [`render_transition`] — runs the transition pipeline to a 4×4 offscreen
//!   `Rgba8Unorm` texture and returns the raw pixel bytes.
//! - [`pixel_avg`] / [`assert_pixels_approx`] — comparison helpers with epsilon
//!   tolerance for floating-point colour differences.

use sldshow2::config::FilterMode;
use sldshow2::transition::{TransitionPipeline, TransitionUniform};
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Public constants
// ---------------------------------------------------------------------------

/// Side length of the offscreen render target (pixels).
pub const RENDER_SIZE: u32 = 4;

/// Epsilon for per-channel byte comparisons (0–255 scale).
pub const PIXEL_EPSILON: u8 = 8;

// ---------------------------------------------------------------------------
// GPU context
// ---------------------------------------------------------------------------

/// Minimal wgpu state needed by tests.
pub struct GpuCtx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Try to obtain a wgpu device+queue without a surface.
///
/// Returns `None` if no suitable adapter is available (e.g. headless CI);
/// tests should call `return` (skip) in that case rather than panicking.
pub fn try_setup_gpu() -> Option<GpuCtx> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok()?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("test-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()?;

    Some(GpuCtx { device, queue })
}

// ---------------------------------------------------------------------------
// Texture helpers
// ---------------------------------------------------------------------------

/// Create a 1×1 solid-colour RGBA8Unorm texture on the GPU.
///
/// `rgba` is `[r, g, b, a]` in the range `0..=255`.
pub fn create_solid_color_texture(ctx: &GpuCtx, rgba: [u8; 4]) -> wgpu::Texture {
    ctx.device.create_texture_with_data(
        &ctx.queue,
        &wgpu::TextureDescriptor {
            label: Some("solid-color-texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &rgba,
    )
}

// ---------------------------------------------------------------------------
// Offscreen rendering
// ---------------------------------------------------------------------------

/// The output texture format used for all offscreen renders.
///
/// `Rgba8Unorm` keeps readback arithmetic simple (byte values 0–255).
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Build the default-value [`TransitionUniform`] for tests.
///
/// Image size and window size are both set to `RENDER_SIZE × RENDER_SIZE`
/// (square, 1:1 aspect ratio — avoids letterboxing).  All colour adjustments
/// are set to identity values.
pub fn default_uniform(mode: i32, blend: f32) -> TransitionUniform {
    let sz = RENDER_SIZE as f32;
    TransitionUniform {
        blend,
        mode,
        aspect_ratio: [1.0, 1.0],
        bg_color: [0.0, 0.0, 0.0, 1.0],
        window_size: [sz, sz],
        image_a_size: [1.0, 1.0],
        image_b_size: [1.0, 1.0],
        brightness: 0.0,
        contrast: 1.0,
        gamma: 1.0,
        saturation: 1.0,
        fit_mode: 0,       // Fit (letterbox)
        ambient_blur: 5.0, // default, unused since fit_mode=0
        zoom_scale: 1.0,
        zoom_pan: [0.0, 0.0],
        display_mode: 0, // SDR
        sdr_scale_a: 1.0,
        sdr_scale_b: 1.0,
        _pad: [0.0; 2],
    }
}

/// Render one frame of the transition shader to a `RENDER_SIZE×RENDER_SIZE`
/// offscreen texture and return the raw RGBA8 pixel bytes.
///
/// # Arguments
/// * `ctx`     — GPU device+queue from [`try_setup_gpu`].
/// * `tex_a`   — "from" texture (displayed at `blend = 0`).
/// * `tex_b`   — "to" texture  (displayed at `blend = 1`).
/// * `mode`    — transition mode index (0–19).
/// * `blend`   — progress in `[0.0, 1.0]`.
///
/// Returns `RENDER_SIZE * RENDER_SIZE * 4` bytes (RGBA8, row-major).
pub fn render_transition(
    ctx: &GpuCtx,
    tex_a: &wgpu::Texture,
    tex_b: &wgpu::Texture,
    mode: i32,
    blend: f32,
) -> Vec<u8> {
    let GpuCtx { device, queue } = ctx;

    // ── pipeline ────────────────────────────────────────────────────────────
    let pipeline = TransitionPipeline::new(device, OFFSCREEN_FORMAT, FilterMode::Nearest);

    // ── uniform buffer ───────────────────────────────────────────────────────
    let uniform = default_uniform(mode, blend);
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test-uniform"),
        contents: bytemuck::cast_slice(&[uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // ── texture views ────────────────────────────────────────────────────────
    let view_a = tex_a.create_view(&wgpu::TextureViewDescriptor::default());
    let view_b = tex_b.create_view(&wgpu::TextureViewDescriptor::default());

    let bind_group = pipeline.create_bind_group(device, &uniform_buf, &view_a, &view_b);

    // ── offscreen render target ──────────────────────────────────────────────
    let render_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen-target"),
        size: wgpu::Extent3d {
            width: RENDER_SIZE,
            height: RENDER_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render_view = render_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // ── readback buffer ──────────────────────────────────────────────────────
    // Row pitch must be a multiple of 256 (wgpu COPY_BYTES_PER_ROW_ALIGNMENT).
    let bytes_per_pixel: u32 = 4; // Rgba8Unorm
    let unpadded_row_bytes = RENDER_SIZE * bytes_per_pixel;
    let padded_row_bytes = align_up(unpadded_row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback_size = (padded_row_bytes * RENDER_SIZE) as u64;

    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: readback_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // ── encode & submit ──────────────────────────────────────────────────────
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("test-encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&pipeline.render_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1); // fullscreen triangle
    }

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &render_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(RENDER_SIZE),
            },
        },
        wgpu::Extent3d {
            width: RENDER_SIZE,
            height: RENDER_SIZE,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // ── map_async readback ───────────────────────────────────────────────────
    let slice = readback_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).expect("channel send failed");
    });

    // Poll until the GPU finishes.
    device.poll(wgpu::PollType::Wait).expect("GPU poll failed");
    rx.recv()
        .expect("map_async channel closed")
        .expect("map_async failed");

    // Strip row padding so the caller gets exactly RENDER_SIZE * RENDER_SIZE * 4 bytes.
    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((RENDER_SIZE * RENDER_SIZE * bytes_per_pixel) as usize);
    for row in 0..RENDER_SIZE as usize {
        let row_start = row * padded_row_bytes as usize;
        let row_end = row_start + unpadded_row_bytes as usize;
        pixels.extend_from_slice(&mapped[row_start..row_end]);
    }
    drop(mapped);
    readback_buf.unmap();

    pixels
}

// ---------------------------------------------------------------------------
// Pixel analysis helpers
// ---------------------------------------------------------------------------

/// Average RGBA value across all pixels in a readback buffer.
///
/// Returns `[r_avg, g_avg, b_avg, a_avg]` as `f32` in `0.0..=255.0`.
pub fn pixel_avg(pixels: &[u8]) -> [f32; 4] {
    assert!(
        pixels.len() % 4 == 0,
        "pixel buffer length not a multiple of 4"
    );
    let count = (pixels.len() / 4) as f32;
    let mut sum = [0.0f32; 4];
    for chunk in pixels.chunks_exact(4) {
        sum[0] += chunk[0] as f32;
        sum[1] += chunk[1] as f32;
        sum[2] += chunk[2] as f32;
        sum[3] += chunk[3] as f32;
    }
    [
        sum[0] / count,
        sum[1] / count,
        sum[2] / count,
        sum[3] / count,
    ]
}

/// Assert that the average pixel colour of `pixels` is approximately `expected`
/// (per-channel, on a 0–255 scale) within `epsilon`.
///
/// On failure, the assertion message shows mode, blend, and actual vs. expected.
pub fn assert_avg_approx(pixels: &[u8], expected: [f32; 4], epsilon: f32, label: &str) {
    let avg = pixel_avg(pixels);
    for (ch, (a, e)) in avg.iter().zip(expected.iter()).enumerate() {
        assert!(
            (a - e).abs() <= epsilon,
            "{label}: channel {ch} avg={a:.1} expected≈{e:.1} (ε={epsilon})"
        );
    }
}

/// Assert that the average pixel colour is NOT approximately `excluded_color`
/// (i.e. the image is not a solid `excluded_color`).
///
/// Used to verify the mid-blend invariant: at `blend = 0.5` the output must
/// differ visibly from both texture_a and texture_b.
pub fn assert_avg_not_approx(pixels: &[u8], excluded: [f32; 4], min_diff: f32, label: &str) {
    let avg = pixel_avg(pixels);
    let max_channel_diff = avg
        .iter()
        .zip(excluded.iter())
        .map(|(a, e)| (a - e).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_channel_diff >= min_diff,
        "{label}: avg {avg:?} is too close to excluded colour {excluded:?} \
         (max channel diff {max_channel_diff:.1} < required {min_diff})"
    );
}

// ---------------------------------------------------------------------------
// Internal utilities
// ---------------------------------------------------------------------------

fn align_up(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) & !(alignment - 1)
}

// Silence the dead_code lint for `TransitionUniform` fields — the struct is
// constructed via field initialisation in `default_uniform`, which Rust
// considers "used", but some fields might only be read by the GPU.
#[allow(dead_code)]
const _: () = {
    // Compile-time size check: must match WGSL struct size (112 bytes).
    // If this fails, the Rust repr(C) layout diverged from the shader.
    const _SIZE_CHECK: [u8; 112] = [0u8; std::mem::size_of::<TransitionUniform>()];
};
