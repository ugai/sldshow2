use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

mod config;
mod image_loader;
mod slideshow;
mod transition;
// mod consts; // Unused for now
mod error;
mod text;

use config::Config;
use image_loader::TextureManager;
use slideshow::SlideshowTimer;
use text::TextRenderer;
use transition::{TransitionPipeline, TransitionUniform};

struct ApplicationState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: Config,
    surface_config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Arc<winit::window::Window>,

    // Subsystems
    texture_manager: TextureManager,
    pipeline: TransitionPipeline,
    slideshow: SlideshowTimer,
    text_renderer: TextRenderer,

    // Rendering resources
    uniform_buffer: wgpu::Buffer,
    // We recreate bind group when textures change
    bind_group: Option<wgpu::BindGroup>,

    // Transition State
    transition: Option<ActiveTransition>,
    // The texture currently being displayed (when no transition active)
    current_texture_index: Option<usize>,
}

struct ActiveTransition {
    start_time: Instant,
    duration: Duration,
    mode: i32,
    from_index: usize,
    to_index: usize,
    // We bind from_texture -> Texture A, to_texture -> Texture B
}

impl ApplicationState {
    async fn new(window: Arc<winit::window::Window>, config: Config) -> Result<Self> {
        let size = window.inner_size();

        // Initialize WGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
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
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: config_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        // Initialize Subsystems
        let mut texture_manager = TextureManager::new(
            config.viewer.cache_extent,
            (
                config.viewer.max_texture_size[0],
                config.viewer.max_texture_size[1],
            ),
        );

        // Scan images
        if let Err(e) =
            texture_manager.scan_paths(&config.viewer.image_paths, config.viewer.scan_subfolders)
        {
            warn!("Failed to scan paths: {}", e);
        }

        if config.viewer.shuffle {
            texture_manager.shuffle_paths();
        }

        let pipeline = TransitionPipeline::new(&device, config_format);
        let text_renderer = TextRenderer::new(
            &device,
            &queue,
            &surface_config,
            config.style.font_family.as_deref(),
        )?;

        let slideshow = SlideshowTimer::new(config.viewer.timer);

        // Create uniform buffer
        let uniform = TransitionUniform {
            blend: 0.0,
            mode: 0,
            aspect_ratio: [1.0, 1.0], // Placeholder
            bg_color: config.bg_color_f32(),
            window_size: [size.width as f32, size.height as f32],
            image_a_size: [1.0, 1.0], // Placeholder
            image_b_size: [1.0, 1.0], // Placeholder
            _padding: [0.0; 2],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transition Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Initialize state
        let current_texture_index = if texture_manager.len() > 0 {
            Some(0)
        } else {
            None
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            surface_config,
            size,
            window,
            texture_manager,
            pipeline,
            slideshow,
            text_renderer,
            uniform_buffer,
            bind_group: None,
            transition: None,
            current_texture_index,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
            self.text_renderer.resize(new_size.width, new_size.height);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key,
                        ..
                    },
                ..
            } => match physical_key {
                PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::Space) => {
                    self.next_image();
                    true
                }
                PhysicalKey::Code(KeyCode::ArrowLeft) => {
                    self.prev_image();
                    true
                }
                PhysicalKey::Code(KeyCode::KeyP) => {
                    self.slideshow.toggle_pause();
                    info!("Slideshow paused: {}", self.slideshow.paused);
                    true
                }
                PhysicalKey::Code(KeyCode::KeyF) => {
                    let fullscreen = self.window.fullscreen().is_some();
                    self.window.set_fullscreen(if fullscreen {
                        None
                    } else {
                        Some(winit::window::Fullscreen::Borderless(None))
                    });
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn next_image(&mut self) {
        let old_index = self.texture_manager.current_index;
        if self.texture_manager.next(self.config.viewer.pause_at_last) {
            self.start_transition(old_index, self.texture_manager.current_index);
            self.slideshow.reset();
        }
    }

    fn prev_image(&mut self) {
        let old_index = self.texture_manager.current_index;
        if self.texture_manager.previous() {
            self.start_transition(old_index, self.texture_manager.current_index);
            self.slideshow.reset();
        }
    }

    fn start_transition(&mut self, from_index: usize, to_index: usize) {
        // If already transitioning, just update target or snap?
        // For simplicity, snap to target then start new transition from there?
        // Actually, if we are transitioning A->B, and user presses next, we go B->C?
        // Let's simplified: snap current transition to end, then start new.

        let mode = if self.config.transition.random {
            transition::random_transition_mode()
        } else {
            self.config.transition.mode
        };

        self.transition = Some(ActiveTransition {
            start_time: Instant::now(),
            duration: Duration::from_secs_f32(self.config.transition.time),
            mode,
            from_index,
            to_index,
        });

        // Force bind group recreation
        self.bind_group = None;
    }

    fn update(&mut self) {
        self.texture_manager.update(&self.device, &self.queue);

        if self.transition.is_none()
            && !self.texture_manager.paths.is_empty()
            && self.slideshow.update()
        {
            self.next_image();
        }

        // Check if transition finished
        if let Some(ref transition) = self.transition {
            if transition.start_time.elapsed() >= transition.duration {
                // Transition done
                self.current_texture_index = Some(transition.to_index);
                self.transition = None;
                self.bind_group = None; // Needs recreation for static state
            }
        }

        // Update text content
        if let Some(path) = self.texture_manager.current_path() {
            let filename = path.file_name().unwrap_or("Unknown");
            let index = self.texture_manager.current_index + 1;
            let total = self.texture_manager.len();
            self.text_renderer
                .set_text(&format!("{} [{}/{}]", filename, index, total));
        } else {
            self.text_renderer.set_text("No images found");
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Prepare BindGroup and Uniforms
        // Determine which textures to use
        let (tex_a_idx, tex_b_idx, blend, mode) = if let Some(ref t) = self.transition {
            let progress = t.start_time.elapsed().as_secs_f32() / t.duration.as_secs_f32();
            (t.from_index, t.to_index, progress.min(1.0), t.mode)
        } else if let Some(idx) = self.current_texture_index {
            (idx, idx, 0.0, 0)
        } else {
            // No images uploaded yet
            (0, 0, 0.0, 0)
        };

        // If textures are not loaded yet, we can't create bind group.
        // We'll skip rendering contents and just clear screen.
        let tex_a = self.texture_manager.get_texture(tex_a_idx);
        let tex_b = self.texture_manager.get_texture(tex_b_idx);

        if let (Some(tex_a), Some(tex_b)) = (tex_a, tex_b) {
            // Check if bind group needs creation
            // We recreate it every frame if transition is active? No, only on change?
            // Actually, we can assume validation error if we reuse bind group with different textures?
            // BindGroup holds references to views. If views change, we need new BindGroup.
            // Since we are creating logic where transition changes A/B, we definitely need new BindGroup when transition starts/ends.
            // AND we need it every frame? No, only when A or B changes.
            // But A and B are constant during transition A->B.

            if self.bind_group.is_none() {
                self.bind_group = Some(self.pipeline.create_bind_group(
                    &self.device,
                    &self.uniform_buffer,
                    &tex_a.view,
                    &tex_b.view,
                ));
            }

            // Update Uniforms
            let uniform = TransitionUniform {
                blend,
                mode,
                aspect_ratio: [1.0, 1.0], // TODO: Calculate this if needed by shader? Shader calculates it.
                bg_color: self.config.bg_color_f32(),
                window_size: [self.size.width as f32, self.size.height as f32],
                image_a_size: [tex_a.width as f32, tex_a.height as f32],
                image_b_size: [tex_b.width as f32, tex_b.height as f32],
                _padding: [0.0; 2],
            };

            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: self.config.style.bg_color[0] as f64 / 255.0,
                                g: self.config.style.bg_color[1] as f64 / 255.0,
                                b: self.config.style.bg_color[2] as f64 / 255.0,
                                a: self.config.style.bg_color[3] as f64 / 255.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                if let Some(ref bind_group) = self.bind_group {
                    render_pass.set_pipeline(&self.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..3, 0..1); // 3 vertices for fullscreen triangle
                }
            } // End of render pass
        } else {
            // Just clear
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass (Clear)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.style.bg_color[0] as f64 / 255.0,
                            g: self.config.style.bg_color[1] as f64 / 255.0,
                            b: self.config.style.bg_color[2] as f64 / 255.0,
                            a: self.config.style.bg_color[3] as f64 / 255.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // If we are supposed to be displaying something but it's not loaded,
            // maybe we should trigger a redraw to check next frame.
            // Handled by Event loop request_redraw.

            // Still render text even if no image
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Text Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if let Err(e) = self
                .text_renderer
                .render(&self.device, &self.queue, &mut render_pass)
            {
                error!("Text render error: {}", e);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(Utf8PathBuf::from);
    let config = Config::load_default(config_path).unwrap_or_else(|e| {
        error!("Failed to load config: {}", e);
        warn!("Using default configuration");
        Config::default()
    });

    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("sldshow2")
            .with_inner_size(winit::dpi::LogicalSize::new(
                config.window.width,
                config.window.height,
            ))
            .with_decorations(config.window.decorations)
            .with_resizable(config.window.resizable)
            .build(&event_loop)
            .unwrap(),
    );

    // Initialize WGPU state
    let mut state = pollster::block_on(ApplicationState::new(window.clone(), config.clone()))?;

    event_loop
        .run(move |event, target| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key:
                                        PhysicalKey::Code(KeyCode::Escape)
                                        | PhysicalKey::Code(KeyCode::KeyQ),
                                    ..
                                },
                            ..
                        } => target.exit(),
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                                Err(e) => error!("Render error: {:?}", e),
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        })
        .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
}
