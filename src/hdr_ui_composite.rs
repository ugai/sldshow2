//! Intermediate-texture composite pass for egui on HDR (Rgba16Float) swapchains.
//!
//! On HDR swapchains, `egui_wgpu` outputs linear values in \[0, 1\], which maps to
//! \[0, 80 nit\] — darker than the SDR reference white of 203 nits used by the main
//! transition shader.  This module bridges the gap:
//!
//! 1. Provides an `Rgba8Unorm` intermediate render target for egui.
//! 2. Composites that target onto the `Rgba16Float` swapchain, scaling RGB by
//!    `SDR_WHITE_SCALE` so UI content appears at the correct perceived brightness.

use std::borrow::Cow;

use wgpu::{Device, TextureFormat, TextureUsages};

/// The render-target format for the egui intermediate texture in HDR mode.
///
/// `Rgba8Unorm` stores egui's premultiplied-alpha linear output with 8-bit
/// precision per channel — sufficient for UI rendering.
pub const EGUI_HDR_INTERMEDIATE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// Owns the intermediate Rgba8Unorm texture and the composite render pipeline.
pub struct HdrUiComposite {
    /// Intermediate texture that egui renders into (pass `texture_view` as the
    /// colour attachment of the egui render pass).
    pub texture: wgpu::Texture,
    /// View into `texture`.  Use as the render-pass colour attachment for egui.
    pub texture_view: wgpu::TextureView,

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

    /// Draw the intermediate texture onto the current render pass, applying
    /// `SDR_WHITE_SCALE`.  The render pass must target the Rgba16Float swapchain
    /// with `LoadOp::Load` so the main image is preserved underneath.
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
