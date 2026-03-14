//! WGPU render pipeline for image transitions with 20 WGSL shader effects.

use crate::config::{FilterMode, TransitionMode};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

/// Number of available transition modes (must match TRANSITION_MAX_MODE_IDX in WGSL)
const TRANSITION_MODE_COUNT: i32 = 20; // Modes 0..=19

/// SDR reference white on an scRGB (Rgba16Float) swapchain: 203 nits / 80 nits (BT.2408).
pub const SDR_WHITE_SCALE: f32 = 203.0 / 80.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TransitionUniform {
    pub blend: f32,
    pub mode: i32,
    pub aspect_ratio: [f32; 2],
    pub bg_color: [f32; 4],
    pub window_size: [f32; 2],
    pub image_a_size: [f32; 2],
    pub image_b_size: [f32; 2],
    // Color adjustment parameters (mpv-like: keys 1-8)
    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,
    // Ambient fit: 0 = Fit (black bars), 1 = AmbientFit (blurred background)
    pub fit_mode: i32,
    pub ambient_blur: f32,
    // Zoom/pan: scale > 1.0 means zoomed in; pan is UV-space offset
    pub zoom_scale: f32,
    pub zoom_pan: [f32; 2],
    pub display_mode: i32, // 0 = SDR (clamp), 1 = HDR (pass-through)
    // SDR brightness compensation on HDR (Rgba16Float) swapchains.
    // SDR content: 203.0 / 80.0 ≈ 2.54 (BT.2408 reference white).
    // HDR content or SDR swapchain: 1.0 (no scaling).
    pub sdr_scale_a: f32,
    pub sdr_scale_b: f32,
    pub _pad: [f32; 2],
}

pub struct TransitionPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
}

impl TransitionPipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        filter_mode: FilterMode,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Transition Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../assets/shaders/transition.wgsl"
            ))),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transition Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<TransitionUniform>() as u64,
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Transition Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Transition Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let filter = filter_mode.to_wgpu();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Transition Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
        }
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        uniform_buffer: &wgpu::Buffer,
        texture_a: &wgpu::TextureView,
        texture_b: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transition Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(texture_b),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Pick a random transition mode from available effects
    pub fn random_mode() -> TransitionMode {
        use rand::RngExt;
        let mut rng = rand::rng();
        // SAFETY: TRANSITION_MODE_COUNT is 20, so range is 0..20 i.e. 0..=19 which is valid
        TransitionMode::try_from(rng.random_range(0..TRANSITION_MODE_COUNT))
            .expect("random_range(0..20) always produces a valid TransitionMode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TransitionMode;

    #[test]
    fn transition_uniform_size_is_multiple_of_16() {
        // WGSL uniform buffers must be 16-byte aligned.
        assert_eq!(
            std::mem::size_of::<TransitionUniform>() % 16,
            0,
            "TransitionUniform size must be a multiple of 16 bytes for WGSL alignment"
        );
    }

    #[test]
    fn transition_uniform_size_matches_field_layout() {
        // Freeze the exact struct size so accidental field changes are caught.
        // Update this value only when deliberately changing the struct layout.
        assert_eq!(std::mem::size_of::<TransitionUniform>(), 112);
    }

    #[test]
    fn transition_mode_count_matches_config_max() {
        // TRANSITION_MODE_COUNT must equal TransitionMode::MAX + 1 so that
        // random_mode() never produces an out-of-range index.
        assert_eq!(TRANSITION_MODE_COUNT, TransitionMode::MAX + 1);
    }
}
