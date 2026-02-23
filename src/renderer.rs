//! GPU renderer — owns the wgpu surface, device, queue, pipeline, and per-frame resources.

use anyhow::{Context, Result};
use log::info;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::config::Config;
use crate::transition::{TransitionPipeline, TransitionUniform};

/// Groups all wgpu rendering state that was previously spread across
/// `ApplicationState`.  Fields are `pub` so that `app.rs` can access them
/// directly — this refactor is about *grouping*, not encapsulation.
pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub pipeline: TransitionPipeline,
    pub uniform_buffer: wgpu::Buffer,
    /// Recreated when textures change (transition start/end).
    pub bind_group: Option<wgpu::BindGroup>,
}

impl Renderer {
    /// Initialise wgpu and create the rendering pipeline.
    pub async fn new(
        window: Arc<winit::window::Window>,
        config: &Config,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<Self> {
        // Initialize WGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find an appropriate adapter")?;

        info!("Using adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .context("Failed to create device")?;

        let caps = surface.get_capabilities(&adapter);
        let config_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: config_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: {
                let transparent = config.style.bg_color[3] < 255;
                info!("Available alpha modes: {:?}", caps.alpha_modes);
                if transparent {
                    let preferred = [
                        wgpu::CompositeAlphaMode::PreMultiplied,
                        wgpu::CompositeAlphaMode::PostMultiplied,
                        wgpu::CompositeAlphaMode::Auto,
                    ];
                    let selected = preferred
                        .iter()
                        .copied()
                        .find(|m| caps.alpha_modes.contains(m))
                        .unwrap_or(caps.alpha_modes[0]);
                    info!(
                        "Transparent mode enabled, selected alpha mode: {:?}",
                        selected
                    );
                    selected
                } else {
                    caps.alpha_modes[0]
                }
            },
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        let pipeline = TransitionPipeline::new(&device, config_format, config.viewer.filter_mode);

        // Create uniform buffer with initial values
        let uniform = TransitionUniform {
            blend: 0.0,
            mode: 0,
            aspect_ratio: [1.0, 1.0],
            bg_color: config.bg_color_f32(),
            window_size: [size.width as f32, size.height as f32],
            image_a_size: [1.0, 1.0],
            image_b_size: [1.0, 1.0],
            brightness: 0.0,
            contrast: 1.0,
            gamma: 1.0,
            saturation: 1.0,
            fit_mode: config.viewer.fit_mode.to_uniform_value(),
            ambient_blur: config.viewer.ambient_blur,
            zoom_scale: 1.0,
            zoom_pan: [0.0, 0.0],
            _pad: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transition Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            surface,
            device,
            queue,
            surface_config,
            pipeline,
            uniform_buffer,
            bind_group: None,
        })
    }

    /// Reconfigure the surface after a resize.
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = new_size.width;
        self.surface_config.height = new_size.height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    /// The surface texture format chosen during initialisation.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Force the bind group to be recreated on the next frame.
    pub fn invalidate_bind_group(&mut self) {
        self.bind_group = None;
    }
}
