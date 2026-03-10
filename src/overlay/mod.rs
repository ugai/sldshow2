//! egui overlay rendering for on-screen UI (filename bar, OSD, debug info)

mod gallery;
mod help;
mod settings;

use egui::{Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, RichText};
use egui_wgpu::Renderer;
use egui_winit::State;
use std::sync::Arc;
use wgpu::{Device, Queue, TextureFormat};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::config::{Config, FitMode, TransitionMode};
use crate::hdr_ui_composite::{EGUI_HDR_INTERMEDIATE_FORMAT, HdrUiComposite};
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
    ToggleScanSubfolders(bool),
    SetTransitionTime(f32),
    ToggleRandomTransition(bool),
    SetTransitionMode(TransitionMode),
    SetFitMode(FitMode),
    SetAmbientBlur(f32),
    ToggleAlwaysOnTop(bool),
    ToggleFullscreen(bool),
    JumpTo(usize),
}

/// Identifies which overlay panel is currently on top (z-order proxy).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverlayKind {
    Gallery,
    Help,
    Settings,
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
    /// HDR composite pass.  `Some` only when the swapchain is `Rgba16Float`.
    hdr_composite: Option<HdrUiComposite>,
    /// Open-order stack for z-order tracking. Last element is the frontmost overlay.
    overlay_stack: Vec<OverlayKind>,
}

/// Toggle an overlay's visibility, updating the overlay stack accordingly.
/// Returns the new visibility state.
fn toggle_overlay(stack: &mut Vec<OverlayKind>, kind: OverlayKind, currently_shown: bool) -> bool {
    let new_state = !currently_shown;
    stack.retain(|k| *k != kind);
    if new_state {
        stack.push(kind);
    }
    new_state
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

        // On HDR swapchains (Rgba16Float), configure the egui renderer to write
        // to an Rgba8Unorm intermediate texture.  A composite pass then scales
        // the result up to SDR reference white before writing to the swapchain.
        let is_hdr = surface_format == TextureFormat::Rgba16Float;
        let egui_render_format = if is_hdr {
            EGUI_HDR_INTERMEDIATE_FORMAT
        } else {
            surface_format
        };
        let renderer = Renderer::new(device, egui_render_format, None, 1, false);

        let hdr_composite = if is_hdr {
            let size = window.inner_size();
            Some(HdrUiComposite::new(
                device,
                size.width.max(1),
                size.height.max(1),
                surface_format,
            ))
        } else {
            None
        };

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
            hdr_composite,
            overlay_stack: Vec::new(),
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

    fn pop_overlay(&mut self, kind: OverlayKind) {
        self.overlay_stack.retain(|k| *k != kind);
    }

    /// Returns the topmost visible overlay based on open order (z-order proxy).
    pub fn front_overlay(&self) -> Option<OverlayKind> {
        self.overlay_stack.last().copied()
    }

    /// Toggle help overlay visibility
    pub fn toggle_help_overlay(&mut self) -> bool {
        self.show_help_overlay = toggle_overlay(
            &mut self.overlay_stack,
            OverlayKind::Help,
            self.show_help_overlay,
        );
        self.show_help_overlay
    }

    /// Toggle settings overlay visibility
    pub fn toggle_settings(&mut self) -> bool {
        self.show_settings = toggle_overlay(
            &mut self.overlay_stack,
            OverlayKind::Settings,
            self.show_settings,
        );
        self.show_settings
    }

    /// Toggle gallery visibility
    pub fn toggle_gallery(&mut self) -> bool {
        self.show_gallery = toggle_overlay(
            &mut self.overlay_stack,
            OverlayKind::Gallery,
            self.show_gallery,
        );
        self.show_gallery
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
            return gallery::render_gallery(
                &self.context.clone(),
                texture_manager,
                thumbnail_manager,
                &mut self.gallery_textures,
                &mut self.show_gallery,
                &mut self.overlay_stack,
            );
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
            help::render_help(&self.context);
        }

        // Settings overlay
        if self.show_settings {
            if let Some(a) =
                settings::render_settings(&self.context, config, &mut self.show_settings)
            {
                action = Some(a);
            }
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

        // Sync stack for egui-driven closes (e.g., X-button on Settings window).
        if !self.show_settings {
            self.pop_overlay(OverlayKind::Settings);
        }
        if !self.show_help_overlay {
            self.pop_overlay(OverlayKind::Help);
        }

        action
    }
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

    /// Render egui onto the swapchain, handling HDR mode transparently.
    ///
    /// In SDR mode this is a single render pass directly to `swapchain_view`.
    /// In HDR mode (Rgba16Float swapchain) it:
    /// 1. Renders egui into an Rgba8Unorm intermediate texture.
    /// 2. Composites that texture onto `swapchain_view` scaled by `SDR_WHITE_SCALE`,
    ///    so UI elements appear at SDR reference-white brightness (203 nits).
    pub fn render_overlay(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        clipped_primitives: &[egui::ClippedPrimitive],
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        swapchain_view: &wgpu::TextureView,
    ) {
        if let Some(ref hdr) = self.hdr_composite {
            // Pass 1: render egui into the intermediate Rgba8Unorm texture.
            {
                let rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Egui HDR Intermediate Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: hdr.egui_render_target(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let mut rp = rp.forget_lifetime();
                self.renderer
                    .render(&mut rp, clipped_primitives, screen_descriptor);
            }
            // Pass 2: composite the intermediate texture onto the HDR swapchain.
            {
                let rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Egui HDR Composite Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: swapchain_view,
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
                hdr.composite(rp);
            }
        } else {
            // SDR: render egui directly to the swapchain.
            let rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: swapchain_view,
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
            let mut rp = rp.forget_lifetime();
            self.renderer
                .render(&mut rp, clipped_primitives, screen_descriptor);
        }
    }

    /// Handle window resize.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        // egui_winit handles DPI scaling automatically via State::take_egui_input()
        // which queries the window's scale_factor() on each frame.
        // ScaleFactorChanged events trigger a window resize, which updates the surface,
        // and egui automatically adapts to the new scale factor on the next frame.
        if let Some(ref mut hdr) = self.hdr_composite {
            hdr.resize(device, width.max(1), height.max(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_overlay_enables_and_pushes() {
        let mut stack = Vec::new();
        let shown = toggle_overlay(&mut stack, OverlayKind::Help, false);
        assert!(shown);
        assert_eq!(stack, vec![OverlayKind::Help]);
    }

    #[test]
    fn toggle_overlay_disables_and_removes() {
        let mut stack = vec![OverlayKind::Help];
        let shown = toggle_overlay(&mut stack, OverlayKind::Help, true);
        assert!(!shown);
        assert!(stack.is_empty());
    }

    #[test]
    fn toggle_overlay_pushes_to_top_of_stack() {
        let mut stack = vec![OverlayKind::Settings];
        let shown = toggle_overlay(&mut stack, OverlayKind::Help, false);
        assert!(shown);
        assert_eq!(stack, vec![OverlayKind::Settings, OverlayKind::Help]);
    }

    #[test]
    fn toggle_overlay_removes_duplicates_before_push() {
        let mut stack = vec![OverlayKind::Help, OverlayKind::Settings];
        // Re-enable Help (already in stack) — should move to top
        let shown = toggle_overlay(&mut stack, OverlayKind::Help, false);
        assert!(shown);
        assert_eq!(stack, vec![OverlayKind::Settings, OverlayKind::Help]);
    }
}
