//! egui overlay rendering for on-screen UI (filename bar, OSD, debug info)

use egui::{Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, RichText};
use egui_wgpu::Renderer;
use egui_winit::State;
use std::sync::Arc;
use wgpu::{Device, Queue, TextureFormat};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::config::{Config, FitMode};
use crate::osc::{OnScreenController, OscAction};

/// Vertical margin from screen edge (in pixels)
const MARGIN: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayAction {
    Osc(OscAction),
    SetTimer(f32),
    ToggleShuffle(bool),
    SetPauseAtLast(bool),
    SetTransitionTime(f32),
    ToggleRandomTransition(bool),
    SetFitMode(FitMode),
    SetAmbientBlur(f32),
    ToggleAlwaysOnTop(bool),
    ToggleFullscreen(bool),
}

pub struct EguiOverlay {
    context: Context,
    state: State,
    renderer: Renderer,

    // Text content for three display areas
    filename_text: String,
    osd_text: String,
    info_text: String,
    center_error_text: String,

    // Info overlay toggle state
    show_info_overlay: bool,

    // Help overlay toggle state
    show_help_overlay: bool,

    // Settings overlay toggle state
    show_settings: bool,

    // On-Screen Controller
    osc: OnScreenController,

    // Style settings
    font_size: f32,
    text_color: Color32,
}

impl EguiOverlay {
    pub fn new(
        device: &Device,
        surface_format: TextureFormat,
        window: Arc<Window>,
        font_family_name: Option<String>,
    ) -> Self {
        let context = Context::default();

        // Configure fonts
        let mut fonts = FontDefinitions::default();

        if let Some(family_name) = font_family_name {
            use font_loader::system_fonts;
            let property = system_fonts::FontPropertyBuilder::new()
                .family(&family_name)
                .build();
            if let Some((font_data, _)) = system_fonts::get(&property) {
                log::info!("Loaded system font: {}", family_name);
                fonts.font_data.insert(
                    "system_font".to_owned(),
                    FontData::from_owned(font_data).into(),
                );
                // Set as highest priority for Proportional and Monospace
                fonts
                    .families
                    .get_mut(&FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "system_font".to_owned());
                fonts
                    .families
                    .get_mut(&FontFamily::Monospace)
                    .unwrap()
                    .insert(0, "system_font".to_owned());
            } else {
                log::warn!("Failed to load system font: {}", family_name);
            }
        }

        context.set_fonts(fonts);

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
            filename_text: String::new(),
            osd_text: String::new(),
            info_text: String::new(),
            center_error_text: String::new(),
            show_info_overlay: false,
            show_help_overlay: false,
            show_settings: false,
            osc: OnScreenController::new(),
            font_size: 20.0,
            text_color: Color32::WHITE,
        }
    }

    /// Set text style (font size and color)
    pub fn set_style(&mut self, font_size: f32, color_rgba: [u8; 4]) {
        self.font_size = font_size;
        self.text_color = Color32::from_rgba_unmultiplied(
            color_rgba[0],
            color_rgba[1],
            color_rgba[2],
            color_rgba[3],
        );
    }

    /// Set filename bar text (bottom-left)
    pub fn set_text(&mut self, text: &str) {
        self.filename_text = text.to_string();
    }

    /// Set OSD text (top-right, reactive feedback)
    pub fn set_osd_text(&mut self, text: &str) {
        self.osd_text = text.to_string();
    }

    /// Set info overlay text (top-left, debug info)
    pub fn set_info_text(&mut self, text: &str) {
        self.info_text = text.to_string();
    }

    /// Set center error text
    pub fn set_center_error(&mut self, text: &str) {
        self.center_error_text = text.to_string();
    }

    /// Clear center error text
    pub fn clear_center_error(&mut self) {
        self.center_error_text.clear();
    }

    /// Toggle info overlay visibility
    pub fn toggle_info_overlay(&mut self) -> bool {
        self.show_info_overlay = !self.show_info_overlay;
        self.show_info_overlay
    }

    /// Check if info overlay is visible
    pub fn info_overlay_visible(&self) -> bool {
        self.show_info_overlay
    }

    /// Toggle help overlay visibility
    pub fn toggle_help_overlay(&mut self) -> bool {
        self.show_help_overlay = !self.show_help_overlay;
        self.show_help_overlay
    }

    /// Check if help overlay is visible
    pub fn help_overlay_visible(&self) -> bool {
        self.show_help_overlay
    }

    /// Toggle settings overlay visibility
    pub fn toggle_settings(&mut self) -> bool {
        self.show_settings = !self.show_settings;
        self.show_settings
    }

    /// Check if settings overlay is visible
    #[allow(dead_code)]
    pub fn settings_visible(&self) -> bool {
        self.show_settings
    }

    /// Update OSC activity (call on mouse movement)
    pub fn update_osc_activity(&mut self) {
        self.osc.update_activity();
    }

    /// Update OSC state (auto-hide logic)
    pub fn update_osc(&mut self) {
        self.osc.update();
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
    /// Returns any action triggered by UI interaction
    pub fn build_ui(
        &mut self,
        config: &mut Config,
        paused: bool,
        current_index: usize,
        total_images: usize,
    ) -> Option<OverlayAction> {
        let font_id = FontId::proportional(self.font_size);
        // egui's screen_rect() returns logical coordinates (already DPI-scaled),
        // so no manual conversion from physical pixels is needed.
        let screen_width = self.context.screen_rect().width();
        let max_width = (screen_width - MARGIN * 2.0).max(100.0);

        // Semi-transparent dark background for readability over images
        let frame = egui::Frame::new()
            .fill(Color32::from_black_alpha(180))
            .inner_margin(egui::Margin::same(6))
            .corner_radius(4.0);

        // Use a local variable to collect actions, though we typically only have one per frame
        let mut action = None;

        // Center error text (highest priority)
        if !self.center_error_text.is_empty() {
            egui::Area::new("center_error".into())
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&self.context, |ui| {
                    ui.set_max_width(max_width * 0.8);
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(&self.center_error_text)
                                .font(FontId::proportional(self.font_size * 1.5))
                                .color(Color32::from_rgb(255, 100, 100)),
                        );
                    });
                });
        }

        // Left-side overlays: stack top-down in priority order (filename, then info)
        let mut next_y = MARGIN;
        let gap = 4.0; // gap between stacked overlays

        if !self.filename_text.is_empty() {
            let resp = egui::Area::new("filename_bar".into())
                .fixed_pos([MARGIN, next_y])
                .show(&self.context, |ui| {
                    ui.set_max_width(max_width);
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(&self.filename_text)
                                .font(font_id.clone())
                                .color(self.text_color),
                        );
                    });
                });
            next_y = resp.response.rect.bottom() + gap;
        }

        if !self.info_text.is_empty() {
            egui::Area::new("info".into())
                .fixed_pos([MARGIN, next_y])
                .show(&self.context, |ui| {
                    ui.set_max_width(max_width * 0.5);
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(&self.info_text)
                                .font(font_id.clone())
                                .color(self.text_color),
                        );
                    });
                });
        }

        // OSD (top-right, independent position)
        if !self.osd_text.is_empty() {
            egui::Area::new("osd".into())
                .anchor(Align2::RIGHT_TOP, [-MARGIN, MARGIN])
                .show(&self.context, |ui| {
                    ui.set_max_width(max_width * 0.5);
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(&self.osd_text)
                                .font(font_id.clone())
                                .color(self.text_color),
                        );
                    });
                });
        }

        // Help overlay (centered window)
        if self.show_help_overlay {
            egui::Window::new("Keyboard Shortcuts")
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(&self.context, |ui| {
                    ui.set_min_width(500.0);

                    ui.heading("Navigation");
                    ui.label("Space / Right       Next image");
                    ui.label("Left               Previous image");
                    ui.label("Shift+Left/Right   Skip 10 images");
                    ui.label("Home / End         Jump to first/last image");
                    ui.label("Mouse Wheel        Next/previous image");
                    ui.label("Shift + Wheel      Skip 10 images");
                    ui.label("Left Click         Next image");
                    ui.label("Right Click        Previous image");
                    ui.label("Double Click       Toggle fullscreen");
                    ui.label("Drag Window        Move window position");

                    ui.add_space(10.0);
                    ui.heading("Playback");
                    ui.label("P                  Pause/resume slideshow");
                    ui.label("[ / ]              Adjust timer (-/+ 1s)");
                    ui.label("Shift + [ / ]      Adjust timer (-/+ 60s)");
                    ui.label("Backspace          Reset timer to default");
                    ui.label("L                  Toggle loop mode");

                    ui.add_space(10.0);
                    ui.heading("Display");
                    ui.label("F                  Toggle fullscreen");
                    ui.label("D                  Toggle window decorations");
                    ui.label("T                  Toggle always on top");
                    ui.label("A                  Toggle fit mode (Normal/Ambient)");
                    ui.label("I / Shift+I        Show info temporarily / toggle");
                    ui.label("O / Shift+O        Show filename temporarily / toggle");

                    ui.add_space(10.0);
                    ui.heading("Color Adjustments");
                    ui.label("1 / 2              Brightness -/+");
                    ui.label("3 / 4              Contrast -/+");
                    ui.label("5 / 6              Gamma -/+");
                    ui.label("7 / 8              Saturation -/+");

                    ui.add_space(10.0);
                    ui.heading("Actions");
                    ui.label("S                  Take screenshot");
                    ui.label("Ctrl+Shift+C       Copy image to clipboard");
                    ui.label("?                  Toggle this help");
                    ui.label("Escape             Close this help");

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Press ? or Escape to close")
                            .italics()
                            .color(Color32::GRAY),
                    );
                });
        }

        // Settings overlay
        if self.show_settings {
            egui::Window::new("Settings")
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(true)
                .open(&mut self.show_settings)
                .show(&self.context, |ui| {
                    ui.set_min_width(300.0);

                    ui.heading("Playback");
                    ui.horizontal(|ui| {
                        ui.label("Timer (sec):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut config.viewer.timer)
                                    .speed(0.1)
                                    .range(0.0..=3600.0),
                            )
                            .changed()
                        {
                            action = Some(OverlayAction::SetTimer(config.viewer.timer));
                        }
                    });
                    if ui.checkbox(&mut config.viewer.shuffle, "Shuffle").changed() {
                        action = Some(OverlayAction::ToggleShuffle(config.viewer.shuffle));
                    }
                    if ui
                        .checkbox(&mut config.viewer.pause_at_last, "Stop at end (No Loop)")
                        .changed()
                    {
                        action = Some(OverlayAction::SetPauseAtLast(config.viewer.pause_at_last));
                    }

                    ui.separator();
                    ui.heading("Transition");
                    ui.horizontal(|ui| {
                        ui.label("Duration (sec):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut config.transition.time)
                                    .speed(0.05)
                                    .range(0.0..=5.0),
                            )
                            .changed()
                        {
                            action = Some(OverlayAction::SetTransitionTime(config.transition.time));
                        }
                    });
                    if ui
                        .checkbox(&mut config.transition.random, "Random Transitions")
                        .changed()
                    {
                        action = Some(OverlayAction::ToggleRandomTransition(
                            config.transition.random,
                        ));
                    }
                    // TODO: Mode dropdown if not random

                    ui.separator();
                    ui.heading("Display");
                    ui.horizontal(|ui| {
                        ui.label("Fit Mode:");
                        if ui
                            .selectable_value(&mut config.viewer.fit_mode, FitMode::Fit, "Fit")
                            .changed()
                        {
                            action = Some(OverlayAction::SetFitMode(FitMode::Fit));
                        }
                        if ui
                            .selectable_value(
                                &mut config.viewer.fit_mode,
                                FitMode::AmbientFit,
                                "Ambient",
                            )
                            .changed()
                        {
                            action = Some(OverlayAction::SetFitMode(FitMode::AmbientFit));
                        }
                    });
                    if config.viewer.fit_mode == FitMode::AmbientFit {
                        ui.horizontal(|ui| {
                            ui.label("Ambient Blur:");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut config.viewer.ambient_blur)
                                        .speed(0.1)
                                        .range(0.0..=10.0),
                                )
                                .changed()
                            {
                                action =
                                    Some(OverlayAction::SetAmbientBlur(config.viewer.ambient_blur));
                            }
                        });
                    }

                    ui.separator();
                    ui.heading("Window");
                    if ui
                        .checkbox(&mut config.window.always_on_top, "Always on Top")
                        .changed()
                    {
                        action = Some(OverlayAction::ToggleAlwaysOnTop(
                            config.window.always_on_top,
                        ));
                    }
                    if ui
                        .checkbox(&mut config.window.fullscreen, "Fullscreen")
                        .changed()
                    {
                        action = Some(OverlayAction::ToggleFullscreen(config.window.fullscreen));
                    }
                });
        }

        // Render OSC (On-Screen Controller) and capture any action
        if let Some(osc_action) = self.osc.render(
            &self.context,
            paused,
            config.viewer.shuffle,
            current_index,
            total_images,
        ) {
            action = Some(OverlayAction::Osc(osc_action));
        }

        action
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
        // egui_winit handles DPI scaling automatically via State::take_egui_input()
        // which queries the window's scale_factor() on each frame.
        // ScaleFactorChanged events trigger a window resize, which updates the surface,
        // and egui automatically adapts to the new scale factor on the next frame.
        // No manual intervention needed here.
    }
}
