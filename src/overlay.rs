//! egui overlay rendering for on-screen UI

use egui::{Context, FontDefinitions};
use egui_wgpu::Renderer;
use egui_winit::State;
use std::sync::Arc;
use wgpu::{Device, Queue, TextureFormat};
use winit::event::WindowEvent;
use winit::window::Window;

pub struct EguiOverlay {
    context: Context,
    state: State,
    renderer: Renderer,
    /// Show demo overlay window
    show_demo: bool,
}

impl EguiOverlay {
    pub fn new(device: &Device, surface_format: TextureFormat, window: Arc<Window>) -> Self {
        let context = Context::default();

        // Configure fonts (use default for now)
        context.set_fonts(FontDefinitions::default());

        // Create egui_winit state
        let state = State::new(
            context.clone(),
            context.viewport_id(),
            &window,
            None, // No custom pixels_per_point
            None, // No custom theme preference
            None, // No custom max_texture_side
        );

        // Create egui_wgpu renderer
        let renderer = Renderer::new(device, surface_format, None, 1, false);

        Self {
            context,
            state,
            renderer,
            show_demo: true, // Visible by default for testing
        }
    }

    /// Forward winit events to egui
    /// Returns true if egui consumed the event
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    /// Begin frame - call at start of each frame in update()
    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.context.begin_pass(raw_input);
    }

    /// Build UI - call after begin_frame()
    pub fn build_ui(&mut self) {
        if self.show_demo {
            egui::Window::new("egui Integration")
                .default_pos([10.0, 10.0])
                .default_size([200.0, 100.0])
                .show(&self.context, |ui| {
                    ui.label("egui Integration Active");
                    ui.separator();
                    ui.label("Test overlay window");

                    if ui.button("Close Overlay").clicked() {
                        self.show_demo = false;
                    }
                });
        }
    }

    /// End frame and prepare render data
    /// Returns egui primitives ready for rendering
    pub fn end_frame(&mut self, window: &Window) -> egui::FullOutput {
        let output = self.context.end_pass();
        self.state
            .handle_platform_output(window, output.platform_output.clone());
        output
    }

    /// Prepare egui render data (textures and buffers)
    /// Call this before creating the render pass
    pub fn prepare_render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        output: egui::FullOutput,
    ) -> Vec<egui::ClippedPrimitive> {
        let clipped_primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);

        // Upload resources
        for (id, image_delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        // Update buffers
        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            screen_descriptor,
        );

        // Cleanup textures
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        clipped_primitives
    }

    /// Render egui primitives into a render pass
    /// Must call prepare_render() first to get clipped_primitives
    pub fn render<'rp>(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'rp>,
        clipped_primitives: &[egui::ClippedPrimitive],
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
    ) {
        // SAFETY: The egui_wgpu::Renderer::render signature uses RenderPass<'static>
        // for API simplicity, but it doesn't actually require a 'static lifetime.
        // The render pass is only used during this function call and doesn't escape.
        // We transmute the lifetime to match the expected signature.
        let render_pass_static: &mut wgpu::RenderPass<'static> =
            unsafe { std::mem::transmute(render_pass) };

        self.renderer
            .render(render_pass_static, clipped_primitives, screen_descriptor);
    }

    /// Handle window resize
    pub fn resize(&mut self, _width: u32, _height: u32) {
        // egui_winit handles DPI scaling automatically
        // Nothing specific needed here unless we store viewport size
    }

    /// Toggle demo overlay visibility
    pub fn toggle_demo(&mut self) -> bool {
        self.show_demo = !self.show_demo;
        self.show_demo
    }
}
