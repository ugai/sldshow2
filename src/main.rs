use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::{
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
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

    // Input state
    last_cursor_move: Instant,
    cursor_visible: bool,
    last_click_time: Option<Instant>,
    drag_start_cursor: Option<winit::dpi::PhysicalPosition<f64>>,
    is_dragging: bool,
    ignore_next_release: bool,
    cursor_pos: Option<winit::dpi::PhysicalPosition<f64>>,

    // OSD
    osd_message: Option<(String, Instant)>,
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

        let pipeline = TransitionPipeline::new(&device, config_format, &config.viewer.filter_mode);
        let mut text_renderer = TextRenderer::new(
            &device,
            &queue,
            &surface_config,
            config.style.font_family.as_deref(),
        )?;
        // Apply style config
        text_renderer.set_style(config.style.font_size, config.style.text_color);

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
            last_cursor_move: Instant::now(),
            cursor_visible: true,
            last_click_time: None,
            drag_start_cursor: None,
            is_dragging: false,
            ignore_next_release: false,
            cursor_pos: None,
            osd_message: None,
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

    fn input(&mut self, event: &WindowEvent, modifiers: &winit::keyboard::ModifiersState) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_move = Instant::now();
                // self.cursor_pos = Some(*position); // Moved to calculation below
                if !self.cursor_visible {
                    self.window.set_cursor_visible(true);
                    self.cursor_visible = true;
                }

                // Drag Logic
                // We calculate screen position to be robust against window moving/resizing (e.g. fullscreen toggle)
                let client_origin = self
                    .window
                    .inner_position()
                    .unwrap_or(winit::dpi::PhysicalPosition::default());
                let screen_pos_x = client_origin.x as f64 + position.x;
                let screen_pos_y = client_origin.y as f64 + position.y;
                let screen_pos = winit::dpi::PhysicalPosition::new(screen_pos_x, screen_pos_y);

                if let Some(start_pos) = self.drag_start_cursor {
                    let dx = screen_pos.x - start_pos.x;
                    let dy = screen_pos.y - start_pos.y;
                    let dist_sq = dx * dx + dy * dy;

                    if !self.is_dragging {
                        if dist_sq > 25.0 {
                            // 5px threshold
                            self.is_dragging = true;
                        }
                    }

                    if self.is_dragging {
                        // Check if fullscreen
                        if self.window.fullscreen().is_some() {
                            self.window.set_fullscreen(None);
                            // Update drag start to current screen pos so we don't jump if logic had lag,
                            // though with screen coords it should be fine.
                            // But exiting fullscreen might take a frame.
                            self.drag_start_cursor = Some(screen_pos);
                            return true;
                        }

                        if let Ok(outer_pos) = self.window.outer_position() {
                            let new_x = outer_pos.x + dx as i32;
                            let new_y = outer_pos.y + dy as i32;
                            self.window
                                .set_outer_position(winit::dpi::PhysicalPosition::new(
                                    new_x, new_y,
                                ));

                            // IMPORTANT: Update start pos so we don't accumulate delta from original start
                            // recursively if we keep adding dx to Window Pos?
                            // Wait, if we use `start_pos` (constant during drag) and `dx` (growing),
                            // then `new_pos = original_outer_pos + dx`.
                            // We need `original_outer_pos` stored at start of drag?
                            // OR we use incremental delta.
                            // `dx` here is "Movement since START of drag".
                            // `outer_pos` is CURRENT window pos.
                            // If we add `dx` to `current`, we fly away exponentially.
                            // We need `dx` since *last frame*?
                            // `dx = screen_pos - last_screen_pos`.
                            // We need to track `last_screen_pos`.
                            // `drag_start_cursor` is currently treated as "Start of Drag".
                            // Let's change usage: `drag_start_cursor` -> `last_drag_pos`.

                            self.drag_start_cursor = Some(screen_pos);
                        }
                    }
                } else if self.cursor_pos.is_some() {
                    // Update cursor pos for potential click/drag start
                    // We only start detailed tracking when button is pressed?
                    // Actually we need to track this *before* press to have valid start?
                    // No, press sets `cursor_pos`.
                }
                // Store current screen pos as "cursor_pos" for drag start initiation
                self.cursor_pos = Some(screen_pos);
                false
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                self.last_cursor_move = Instant::now();
                match button {
                    MouseButton::Left => {
                        // Double Click
                        let now = Instant::now();
                        if let Some(last) = self.last_click_time {
                            if now.duration_since(last).as_millis() < 300 {
                                let fullscreen = self.window.fullscreen().is_some();
                                self.window.set_fullscreen(if fullscreen {
                                    None
                                } else {
                                    Some(winit::window::Fullscreen::Borderless(None))
                                });
                                self.show_osd(
                                    if fullscreen {
                                        "Fullscreen: OFF"
                                    } else {
                                        "Fullscreen: ON"
                                    }
                                    .to_string(),
                                );
                                self.last_click_time = None;
                                self.ignore_next_release = true; // Don't trigger 'next' on this release
                                return true;
                            }
                        }
                        self.last_click_time = Some(now);

                        // Start tracking for Drag (or Click)
                        if let Some(pos) = self.cursor_pos {
                            // Initialize Drag Start with Screen Position
                            self.drag_start_cursor = Some(pos);
                        }

                        self.is_dragging = false;
                        self.ignore_next_release = false;

                        true
                    }
                    MouseButton::Right => {
                        self.prev_image();
                        true
                    }
                    _ => false,
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.drag_start_cursor = None;
                if self.is_dragging {
                    self.is_dragging = false;
                } else {
                    if !self.ignore_next_release {
                        self.next_image();
                    }
                }
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.last_cursor_move = Instant::now();
                let steps = if modifiers.shift_key() { 10 } else { 1 };
                // Simple wheel handling: any movement up/down triggers next/prev
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        if *y > 0.0 {
                            for _ in 0..steps {
                                self.prev_image();
                            }
                        } else if *y < 0.0 {
                            for _ in 0..steps {
                                self.next_image();
                            }
                        }
                        true
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        if pos.y > 0.0 {
                            for _ in 0..steps {
                                self.prev_image();
                            }
                        } else if pos.y < 0.0 {
                            for _ in 0..steps {
                                self.next_image();
                            }
                        }
                        true
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key,
                        logical_key: _,
                        ..
                    },
                ..
            } => {
                self.last_cursor_move = Instant::now(); // Typing wakes cursor

                // Check for keys that work with any modifiers or specific combinations
                match physical_key {
                    PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::Space) => {
                        let steps = if modifiers.shift_key() { 10 } else { 1 };
                        for _ in 0..steps {
                            self.next_image();
                        }
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        let steps = if modifiers.shift_key() { 10 } else { 1 };
                        for _ in 0..steps {
                            self.prev_image();
                        }
                        true
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        self.jump_to(0);
                        true
                    }
                    PhysicalKey::Code(KeyCode::End) => {
                        let last = self.texture_manager.len().saturating_sub(1);
                        self.jump_to(last);
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyP) => {
                        self.slideshow.toggle_pause();
                        info!("Slideshow paused: {}", self.slideshow.paused);
                        self.show_osd(
                            if self.slideshow.paused {
                                "Paused"
                            } else {
                                "Resumed"
                            }
                            .to_string(),
                        );
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyF) => {
                        let fullscreen = self.window.fullscreen().is_some();
                        self.window.set_fullscreen(if fullscreen {
                            None
                        } else {
                            Some(winit::window::Fullscreen::Borderless(None))
                        });
                        self.show_osd(
                            if fullscreen {
                                "Fullscreen: OFF"
                            } else {
                                "Fullscreen: ON"
                            }
                            .to_string(),
                        );
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyD) => {
                        let decorated = self.window.is_decorated();
                        self.window.set_decorations(!decorated);
                        self.show_osd(
                            if !decorated {
                                "Decorations: ON"
                            } else {
                                "Decorations: OFF"
                            }
                            .to_string(),
                        );
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        let always_on_top = !self.config.window.always_on_top;
                        self.config.window.always_on_top = always_on_top;
                        self.window.set_window_level(if always_on_top {
                            winit::window::WindowLevel::AlwaysOnTop
                        } else {
                            winit::window::WindowLevel::Normal
                        });
                        self.show_osd(
                            if always_on_top {
                                "Always On Top: ON"
                            } else {
                                "Always On Top: OFF"
                            }
                            .to_string(),
                        );
                        true
                    }
                    PhysicalKey::Code(KeyCode::BracketLeft) => {
                        // [
                        self.adjust_timer(-1.0);
                        true
                    }
                    PhysicalKey::Code(KeyCode::BracketRight) => {
                        // ]
                        self.adjust_timer(1.0);
                        true
                    }
                    _ => false,
                }
            }
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

    fn jump_to(&mut self, index: usize) {
        let old_index = self.texture_manager.current_index;
        if index < self.texture_manager.len() && index != old_index {
            self.texture_manager.jump_to(index);
            self.start_transition(old_index, self.texture_manager.current_index);
            self.slideshow.reset();
        }
    }

    fn adjust_timer(&mut self, delta: f32) {
        let new_timer = (self.slideshow.duration() + delta).max(1.0);
        self.slideshow.set_duration(new_timer);
        info!("Slideshow timer set to: {:.1}s", new_timer);
        self.show_osd(format!("Timer: {:.1}s", new_timer));
    }

    fn show_osd(&mut self, text: String) {
        self.osd_message = Some((text, Instant::now() + Duration::from_millis(1500)));
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
        // Auto-hide cursor
        if self.cursor_visible && self.last_cursor_move.elapsed().as_secs_f32() > 3.0 {
            self.window.set_cursor_visible(false);
            self.cursor_visible = false;
        }

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

        // OSD Logic
        if let Some((ref text, expiry)) = self.osd_message {
            if Instant::now() > expiry {
                self.osd_message = None;
                self.text_renderer.set_osd_text("");
            } else {
                self.text_renderer.set_osd_text(text);
            }
        } else {
            self.text_renderer.set_osd_text("");
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

    // Prevent screen saver
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Power::{
            ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        // Prevents sleep and screen saver
        SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
    }

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

    let mut modifiers = winit::keyboard::ModifiersState::default();

    event_loop
        .run(move |event, target| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                // Update modifiers state
                if let WindowEvent::ModifiersChanged(state) = event {
                    modifiers = state.state();
                }

                if !state.input(event, &modifiers) {
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
                        // Handle modifiers for our mocked helper
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Digit1),
                                    ..
                                },
                            ..
                        } => {
                            if modifiers.alt_key() {
                                let _ = state
                                    .window
                                    .request_inner_size(winit::dpi::LogicalSize::new(1280, 720));
                                state.show_osd("Resize: 1280x720".to_string());
                            }
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Digit2),
                                    ..
                                },
                            ..
                        } => {
                            if modifiers.alt_key() {
                                let _ = state
                                    .window
                                    .request_inner_size(winit::dpi::LogicalSize::new(1920, 1080));
                                state.show_osd("Resize: 1920x1080".to_string());
                            }
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Digit0),
                                    ..
                                },
                            ..
                        } => {
                            if modifiers.alt_key() {
                                let _ =
                                    state
                                        .window
                                        .request_inner_size(winit::dpi::LogicalSize::new(
                                            state.config.window.width,
                                            state.config.window.height,
                                        ));
                                state.show_osd(format!(
                                    "Resize: {}x{}",
                                    state.config.window.width, state.config.window.height
                                ));
                            }
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::KeyC),
                                    ..
                                },
                            ..
                        } => {
                            if modifiers.control_key() {
                                if let Some(path) = state.texture_manager.current_path() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        if let Err(e) = clipboard.set_text(path.as_str()) {
                                            error!("Failed to copy to clipboard: {}", e);
                                        } else {
                                            info!("Copied path to clipboard: {}", path);
                                            state.show_osd("Copied to Clipboard".to_string());
                                        }
                                    }
                                }
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
