//! egui overlay rendering for on-screen UI (filename bar, OSD, debug info)

use egui::{Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, RichText};
use egui_wgpu::Renderer;
use egui_winit::State;
use std::sync::Arc;
use wgpu::{Device, Queue, TextureFormat};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::config::{Config, FitMode};
use crate::osc::{Osc, OscAction};
use crate::thumbnail::ThumbnailManager;
use std::collections::HashMap;

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
    JumpTo(usize),
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
    osc: Osc,

    // Style settings
    font_size: f32,
    text_color: Color32,

    // Gallery state
    show_gallery: bool,
    gallery_textures: HashMap<usize, egui::TextureHandle>,
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
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        if let Some(family_name) = font_family_name {
            if family_name != "Inter" && family_name != "default" {
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
                    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                        family.insert(0, "system_font".to_owned());
                    } else {
                        log::warn!("Proportional font family missing from FontDefinitions");
                    }
                    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                        family.insert(0, "system_font".to_owned());
                    } else {
                        log::warn!("Monospace font family missing from FontDefinitions");
                    }
                } else {
                    log::warn!("Failed to load system font: {}", family_name);
                }
            }
        }

        context.set_fonts(fonts);

        // Enhance rendering crispness on scaled displays (like 1.25x or 1.5x)
        let mut options = egui::Options::default();
        options.tessellation_options.feathering = true;
        // Text should be pixel-aligned for maximum crispness
        context.options_mut(|o| *o = options);

        // Adjust widget spacing and alignments
        let mut style = egui::Style::default();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(12);

        context.set_style(style);

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
            osc: Osc::new(),
            font_size: 20.0,
            text_color: Color32::WHITE,
            show_gallery: false,
            gallery_textures: HashMap::new(),
        }
    }

    /// Set text style (font size and color)
    pub fn set_style(&mut self, font_size: f32, text_color: [u8; 4]) {
        self.font_size = font_size;
        self.text_color = Color32::from_rgba_unmultiplied(
            text_color[0],
            text_color[1],
            text_color[2],
            text_color[3],
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

    /// Toggle gallery visibility
    pub fn toggle_gallery(&mut self) {
        self.show_gallery = !self.show_gallery;
    }

    /// Returns `true` when any overlay or the OSC is currently visible,
    /// meaning redraws are needed to animate or respond to input.
    pub fn is_active(&self) -> bool {
        self.show_settings || self.show_help_overlay || self.show_gallery || self.osc.visible
    }

    fn cleanup_gallery_textures(&mut self, thumbnail_manager: &mut ThumbnailManager) {
        // Remove handles for thumbnails that are no longer in the cache (evicted).
        let cached_indices: std::collections::HashSet<_> =
            thumbnail_manager.get_cached_indices().into_iter().collect();
        self.gallery_textures
            .retain(|k, _| cached_indices.contains(k));

        // Invalidate handles for thumbnails that were re-generated since the last
        // frame. The next render will recreate them from the fresh pixel data.
        for index in thumbnail_manager.drain_newly_cached() {
            self.gallery_textures.remove(&index);
        }
    }

    /// Update OSC activity (call on mouse movement)
    pub fn update_osc_activity(&mut self) {
        self.osc.update_interaction();
    }

    /// Update OSC state (auto-hide logic)
    pub fn update_osc(&mut self) {
        self.osc.check_autohide();
    }

    /// Returns true if egui currently wants to capture pointer/mouse input.
    /// Use this to suppress pointer events from reaching the application
    /// when an egui panel (e.g. Settings) is being interacted with.
    pub fn wants_pointer_input(&self) -> bool {
        self.state.egui_ctx().wants_pointer_input()
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
        texture_manager: &crate::image_loader::TextureManager,
        thumbnail_manager: &mut ThumbnailManager,
    ) -> Option<OverlayAction> {
        let current_index = texture_manager.current_index;
        let total_images = texture_manager.len();

        // Cleanup evicted textures
        self.cleanup_gallery_textures(thumbnail_manager);

        if self.show_gallery {
            return self.render_gallery(&self.context.clone(), texture_manager, thumbnail_manager);
        }
        let font_id = FontId::proportional(self.font_size);
        // egui's screen_rect() returns logical coordinates (already DPI-scaled),
        // so no manual conversion from physical pixels is needed.
        let screen_width = self.context.screen_rect().width();
        let max_width = (screen_width - MARGIN * 2.0).max(100.0);

        // Semi-transparent dark background for readability over images
        let frame = egui::Frame::new()
            .fill(Color32::from_black_alpha(160))
            .inner_margin(egui::Margin::same(12))
            .corner_radius(8.0);

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
                .resizable(true)
                .show(&self.context, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 20.0)
                        .show(ui, |ui| {
                            ui.set_min_width(350.0);

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

                            ui.add_space(4.0);
                            ui.heading("Playback");
                            ui.label("P                  Pause/resume slideshow");
                            ui.label("[ / ]              Adjust timer (-/+ 1s)");
                            ui.label("Shift + [ / ]      Adjust timer (-/+ 60s)");
                            ui.label("Backspace          Reset timer to default");
                            ui.label("L                  Toggle loop mode");

                            ui.add_space(4.0);
                            ui.heading("Display");
                            ui.label("F                  Toggle fullscreen");
                            ui.label("D                  Toggle window decorations");
                            ui.label("T                  Toggle always on top");
                            ui.label("A                  Toggle fit mode (Normal/Ambient)");
                            ui.label("I / Shift+I        Show info temporarily / toggle");
                            ui.label("O / Shift+O        Show filename temporarily / toggle");

                            ui.add_space(4.0);
                            ui.heading("Window Resize");
                            ui.label("Alt+0              Configured default size");
                            ui.label("Alt+1              1024x768");
                            ui.label("Alt+2              1920x1080 (Full HD)");

                            ui.add_space(4.0);
                            ui.heading("Color Adjustments");
                            ui.label("1 / 2              Contrast -/+");
                            ui.label("3 / 4              Brightness -/+");
                            ui.label("5 / 6              Gamma -/+");
                            ui.label("7 / 8              Saturation -/+");

                            ui.add_space(4.0);
                            ui.heading("Actions");
                            ui.label("S                  Take screenshot");
                            ui.label("Ctrl+Shift+C       Copy image to clipboard");
                            ui.label("?                  Toggle this help");
                            ui.label("Escape             Close this help");

                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Press ? or Escape to close")
                                    .italics()
                                    .color(Color32::GRAY),
                            );
                        });
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

    fn render_gallery(
        &mut self,
        ctx: &Context,
        texture_manager: &crate::image_loader::TextureManager,
        thumbnail_manager: &mut ThumbnailManager,
    ) -> Option<OverlayAction> {
        // Reset pending queue to ensure we only prioritize currently visible items
        thumbnail_manager.clear_pending();

        let mut action = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Gallery");
            ui.separator();

            let thumbnail_size = 256.0;
            let padding = 8.0;
            let item_size = egui::vec2(thumbnail_size, thumbnail_size);
            let cell_size = item_size + egui::vec2(padding, padding);

            // Scrollbar width is usually 12.0, but let's be safe.
            // If we cant find the field, we'll estimate.
            let scroll_bar_width = 16.0;
            let width = ui.available_width() - scroll_bar_width - padding * 2.0;
            let cols = (width / cell_size.x).floor() as usize;
            let cols = cols.max(1);
            let count = texture_manager.len();
            let rows = count.div_ceil(cols);

            egui::ScrollArea::vertical().show_rows(ui, cell_size.y, rows, |ui, row_range| {
                ui.style_mut().spacing.item_spacing = egui::vec2(padding, padding);

                for row in row_range {
                    ui.horizontal(|ui| {
                        for col in 0..cols {
                            let index = row * cols + col;
                            if index >= count {
                                break;
                            }

                            // Get path for thumbnail generation
                            if let Some(path) = texture_manager.paths.get(index) {
                                // Request thumbnail if not present
                                if thumbnail_manager.get_thumbnail(index).is_none() {
                                    thumbnail_manager.request_thumbnail(index, path);
                                }
                            }

                            // Retrieve texture
                            let texture_id = if let Some(img) =
                                thumbnail_manager.get_thumbnail(index)
                            {
                                // Create or get TextureHandle
                                let handle =
                                    self.gallery_textures.entry(index).or_insert_with(|| {
                                        // Convert RgbaImage to ColorImage
                                        let size = [img.width() as usize, img.height() as usize];
                                        let pixels = img.as_flat_samples();
                                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                            size,
                                            pixels.as_slice(),
                                        );
                                        ctx.load_texture(
                                            format!("thumb_{}", index),
                                            color_image,
                                            egui::TextureOptions::LINEAR,
                                        )
                                    });
                                handle.id()
                            } else {
                                egui::TextureId::default()
                            };

                            // Determine if we have a valid texture
                            let has_texture = self.gallery_textures.contains_key(&index);

                            let btn_size = item_size;
                            let resp = if has_texture {
                                ui.add_sized(
                                    btn_size,
                                    egui::ImageButton::new((texture_id, btn_size)).frame(false),
                                )
                            } else {
                                ui.add_sized(btn_size, egui::Button::new("Loading...").frame(true))
                            };

                            if resp.clicked() {
                                action = Some(OverlayAction::JumpTo(index));
                                self.show_gallery = false;
                            }
                        }
                    });
                }
            });
        });

        // Close on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_gallery = false;
        }

        action
    }
}
