use egui::{Context, FullOutput};
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use wgpu::{Device, Queue, TextureFormat};
use winit::window::Window;

pub struct UiState {
    pub context: Context,
    state: State,
    renderer: Renderer,
}

impl UiState {
    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        msaa_samples: u32,
        window: &Window,
    ) -> Self {
        let context = Context::default();
        let viewport_id = context.viewport_id();
        let state = State::new(
            context.clone(),
            viewport_id,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );
        let renderer = Renderer::new(device, output_color_format, None, msaa_samples);

        Self {
            context,
            state,
            renderer,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        render_target: &wgpu::TextureView,
        screen_descriptor: ScreenDescriptor,
        run_ui: impl FnOnce(&Context),
    ) {
        let raw_input = self.state.take_egui_input(window);
        let FullOutput {
            shapes,
            textures_delta,
            platform_output,
            ..
        } = self.context.run(raw_input, run_ui);

        self.state.handle_platform_output(window, platform_output);

        let clipped_primitives = self
            .context
            .tessellate(shapes, screen_descriptor.pixels_per_point);
        if !clipped_primitives.is_empty() {
            // log::info!("Egui generated {} primitive sets", clipped_primitives.len());
        } else {
            log::warn!("Egui generated 0 primitives");
        }

        for (id, image_delta) in &textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer
                .render(&mut rpass, &clipped_primitives, &screen_descriptor);
        }

        for id in &textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
