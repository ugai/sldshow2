//! Intermediate-texture composite pass for egui on HDR (Rgba16Float) swapchains.
//!
//! On HDR swapchains, `egui_wgpu` outputs linear values in \[0, 1\], which maps to
//! \[0, 80 nit\] — darker than the SDR reference white of 203 nits used by the main
//! transition shader.  This module bridges the gap:
//!
//! 1. Provides an `Rgba8UnormSrgb` intermediate render target for egui.
//! 2. Composites that target onto the `Rgba16Float` swapchain, scaling RGB by
//!    `SDR_WHITE_SCALE` so UI content appears at the correct perceived brightness.
//!
//! # Implementation Note — egui_wgpu Coupling
//!
//! **This is a workaround for egui_wgpu's internal gamma-handling behaviour.**
//!
//! `egui_wgpu` decides how to gamma-encode output based on `format.is_srgb()`:
//! - `is_srgb() == true`  → egui writes **linear** values; the GPU applies sRGB
//!   encoding on write (correct for this composite pipeline).
//! - `is_srgb() == false` → egui calls `linear_to_gamma()` in its fragment shader
//!   before writing (would store gamma-encoded values, making the subsequent
//!   ×SDR_WHITE_SCALE multiplication produce badly over-bright results).
//!
//! We exploit the first case by using `Rgba8UnormSrgb` as the intermediate format,
//! even though `Rgba8Unorm` would otherwise be a more obvious choice.
//!
//! **Regression risk on egui_wgpu upgrades:**
//! If egui_wgpu changes its `is_srgb()` heuristic, adds native HDR support, or
//! alters its gamma path in any way, this module will likely break silently (UI
//! will appear over-bright, washed out, or incorrect in HDR mode).
//!
//! When upgrading egui_wgpu, verify:
//! 1. Gallery thumbnails match the perceived brightness of the main slide image.
//! 2. White UI elements (panels, text backgrounds) are not blown out.
//! 3. Check `egui_wgpu` source for any changes to `format.is_srgb()` usage or
//!    gamma handling in the renderer's fragment shader.

use std::borrow::Cow;

use wgpu::{Device, TextureFormat, TextureUsages};

/// The render-target format for the egui intermediate texture in HDR mode.
///
/// # Why `Rgba8UnormSrgb` and not `Rgba8Unorm`?
///
/// `egui_wgpu` inspects `format.is_srgb()` to decide gamma handling:
/// - **sRGB format** (`is_srgb() == true`): egui writes linear values and lets
///   the GPU apply sRGB encoding on write.  This is what we need.
/// - **non-sRGB format** (`is_srgb() == false`): egui calls `linear_to_gamma()`
///   in its own fragment shader, storing gamma-encoded values.  Multiplying those
///   by `SDR_WHITE_SCALE` (≈ 2.54) would produce badly over-bright output.
///
/// With `Rgba8UnormSrgb`:
/// 1. egui writes **linear** values → GPU encodes sRGB on write.
/// 2. Composite shader samples texture → GPU decodes sRGB back to linear.
/// 3. Shader multiplies by `SDR_WHITE_SCALE` → correct HDR brightness on output.
///
/// # WARNING — egui_wgpu version coupling
///
/// This relies on egui_wgpu's `is_srgb()` heuristic (validated against
/// egui_wgpu 0.32.x).  If egui_wgpu changes its gamma-handling logic or adds
/// native HDR-format support, this constant may need to be revisited.  See the
/// module-level doc for the full regression checklist.
pub const EGUI_HDR_INTERMEDIATE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Owns the intermediate Rgba8UnormSrgb texture and the composite render pipeline.
pub struct HdrUiComposite {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,

    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,

    // Retained for bind-group recreation on resize.
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl HdrUiComposite {
    /// Create the intermediate texture and composite pipeline.
    ///
    /// * `output_format` — the swapchain format (must be `Rgba16Float`).
    /// * `width` / `height` — initial surface size in physical pixels.
    pub fn new(device: &Device, width: u32, height: u32, output_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR UI Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../assets/shaders/hdr_ui_composite.wgsl"
            ))),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR UI Composite BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR UI Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR UI Composite Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    // Premultiplied-alpha blend — matches egui_wgpu's own blend mode.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HDR UI Composite Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let (texture, texture_view) = Self::create_texture(device, width, height);
        let bind_group =
            Self::create_bind_group(device, &bind_group_layout, &texture_view, &sampler);

        Self {
            texture,
            texture_view,
            pipeline,
            bind_group,
            bind_group_layout,
            sampler,
        }
    }

    /// Recreate the intermediate texture for a new surface size.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        let (texture, view) = Self::create_texture(device, width, height);
        self.texture = texture;
        self.texture_view = view;
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &self.texture_view,
            &self.sampler,
        );
    }

    /// The colour-attachment view for the egui intermediate render pass.
    ///
    /// Pass this as `view` in `RenderPassColorAttachment` when rendering egui.
    pub fn egui_render_target(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    /// Draw the intermediate texture onto the current render pass, applying
    /// `SDR_WHITE_SCALE`.  The render pass must target the Rgba16Float swapchain
    /// with `LoadOp::Load` so the main image is preserved underneath.
    ///
    /// `forget_lifetime()` is used internally to satisfy the borrow checker while
    /// keeping a single `&self` borrow across both sub-passes.
    pub fn composite<'rp>(&'rp self, render_pass: wgpu::RenderPass<'rp>) {
        let mut rp = render_pass.forget_lifetime();
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.bind_group, &[]);
        rp.draw(0..3, 0..1);
    }

    fn create_texture(
        device: &Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR UI Intermediate Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: EGUI_HDR_INTERMEDIATE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        (texture, view)
    }

    fn create_bind_group(
        device: &Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HDR UI Composite Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
