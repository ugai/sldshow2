//! On-Screen Controller (OSC) for playback controls.
//!
//! Provides interactive buttons for play/pause, navigation, and shuffle.

use egui::{Align2, Color32, Context, RichText, Stroke, Vec2};
use std::time::{Duration, Instant};

/// Auto-hide timeout in seconds
const OSC_TIMEOUT: f32 = 2.0;

/// Minimum vertical margin from bottom edge
const OSC_BOTTOM_MARGIN: f32 = 20.0;

/// OSC state and rendering
pub struct OnScreenController {
    /// Last mouse activity time
    last_activity: Instant,
    /// Whether OSC is currently visible
    visible: bool,
    /// Whether mouse is hovering over OSC
    hovering: bool,
}

/// Actions triggered by OSC button clicks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscAction {
    PlayPause,
    Previous,
    Next,
    ToggleShuffle,
    OpenSettings,
    Seek(usize),
}

impl OnScreenController {
    pub fn new() -> Self {
        Self {
            last_activity: Instant::now(),
            visible: true,
            hovering: false,
        }
    }

    /// Update activity timer (call on mouse movement)
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
        self.visible = true;
    }

    /// Update visibility based on timeout
    pub fn update(&mut self) {
        if !self.hovering && self.last_activity.elapsed() > Duration::from_secs_f32(OSC_TIMEOUT) {
            self.visible = false;
        }
    }

    /// Render OSC controls and return any triggered action
    pub fn render(
        &mut self,
        ctx: &Context,
        paused: bool,
        shuffle: bool,
        current_index: usize,
        total_images: usize,
    ) -> Option<OscAction> {
        if !self.visible {
            return None;
        }

        let mut action = None;

        // Position at bottom-center
        let area_response = egui::Area::new("osc".into())
            .anchor(Align2::CENTER_BOTTOM, [0.0, -OSC_BOTTOM_MARGIN])
            .show(ctx, |ui| {
                // Semi-transparent dark background
                let frame = egui::Frame::new()
                    .fill(Color32::from_black_alpha(200))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 10,
                        bottom: 10,
                    })
                    .corner_radius(8.0)
                    .stroke(Stroke::new(1.0, Color32::from_white_alpha(40)));

                frame.show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Scrub Bar
                        if total_images > 1 {
                            if let Some(seek_index) =
                                self.render_scrub_bar(ui, current_index, total_images)
                            {
                                action = Some(OscAction::Seek(seek_index));
                            }
                            ui.add_space(8.0);
                        }

                        // Buttons
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;

                            // Settings button (leftmost)
                            if self.render_icon_button(ui, "⚙") {
                                action = Some(OscAction::OpenSettings);
                            }

                            ui.add_space(8.0);

                            // Previous button
                            if self.render_button(ui, "◀ Prev", false) {
                                action = Some(OscAction::Previous);
                            }

                            // Play/Pause button (highlighted)
                            let play_pause_text = if paused { "▶ Play" } else { "⏸ Pause" };
                            if self.render_button(ui, play_pause_text, true) {
                                action = Some(OscAction::PlayPause);
                            }

                            // Next button
                            if self.render_button(ui, "Next ▶", false) {
                                action = Some(OscAction::Next);
                            }

                            ui.add_space(8.0);

                            // Shuffle toggle (with visual state)
                            if self.render_toggle_button(ui, "🔀 Shuffle", shuffle) {
                                action = Some(OscAction::ToggleShuffle);
                            }
                        });
                    });
                });
            });

        // Track hover state to prevent auto-hide when mouse is over OSC
        self.hovering = area_response.response.hovered();

        action
    }

    /// Render a small icon button
    fn render_icon_button(&self, ui: &mut egui::Ui, text: &str) -> bool {
        let button = egui::Button::new(RichText::new(text).size(20.0).color(Color32::WHITE))
            .fill(Color32::TRANSPARENT)
            .min_size(Vec2::new(32.0, 32.0));

        ui.add(button).clicked()
    }

    /// Render a standard button
    fn render_button(&self, ui: &mut egui::Ui, text: &str, primary: bool) -> bool {
        let button = if primary {
            egui::Button::new(RichText::new(text).size(16.0).color(Color32::WHITE))
                .fill(Color32::from_rgb(60, 120, 200))
                .min_size(Vec2::new(90.0, 32.0))
        } else {
            egui::Button::new(RichText::new(text).size(16.0).color(Color32::WHITE))
                .fill(Color32::from_rgb(50, 50, 50))
                .min_size(Vec2::new(90.0, 32.0))
        };

        ui.add(button).clicked()
    }

    /// Render a toggle button (with active state styling)
    fn render_toggle_button(&self, ui: &mut egui::Ui, text: &str, active: bool) -> bool {
        let fill_color = if active {
            Color32::from_rgb(60, 150, 60) // Green when active
        } else {
            Color32::from_rgb(50, 50, 50) // Gray when inactive
        };

        let button = egui::Button::new(RichText::new(text).size(16.0).color(Color32::WHITE))
            .fill(fill_color)
            .min_size(Vec2::new(110.0, 32.0));

        ui.add(button).clicked()
    }

    /// Render video-style timeline scrub bar
    #[allow(deprecated)]
    fn render_scrub_bar(
        &self,
        ui: &mut egui::Ui,
        current_index: usize,
        total_images: usize,
    ) -> Option<usize> {
        let desired_size = Vec2::new(ui.available_width(), 20.0);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());

        // Visuals
        let _visuals = ui.style().interact(&response);

        let progress = if total_images > 1 {
            current_index as f32 / (total_images - 1) as f32
        } else {
            1.0
        };

        // Background track
        ui.painter()
            .rect_filled(rect, 4.0, Color32::from_white_alpha(30));

        // Progress fill
        let fill_width = rect.width() * progress;
        let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, rect.height()));
        ui.painter().rect_filled(
            fill_rect,
            4.0,
            Color32::from_rgb(100, 160, 255), // Light blue
        );

        // Hover tooltip
        if response.hovered() {
            let mouse_x = ui.input(|i| i.pointer.hover_pos().unwrap_or_default().x);
            let t = ((mouse_x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let hover_index = (t * (total_images - 1) as f32).round() as usize; // 0-based index

            // Show 1-based index for user
            egui::show_tooltip(ui.ctx(), ui.layer_id(), response.id, |ui| {
                ui.label(format!("{} / {}", hover_index + 1, total_images));
            });
        }

        // Interaction (Click or Drag)
        if response.clicked() || response.dragged() {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let t = ((mouse_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let new_index = (t * (total_images - 1) as f32).round() as usize;

                // Only return if valid change
                if new_index != current_index && new_index < total_images {
                    return Some(new_index);
                }
            }
        }

        None
    }

    /// Check if OSC is currently visible
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Force OSC to show (e.g., on window focus)
    #[allow(dead_code)]
    pub fn show(&mut self) {
        self.visible = true;
        self.last_activity = Instant::now();
    }

    /// Force OSC to hide
    #[allow(dead_code)]
    pub fn hide(&mut self) {
        self.visible = false;
    }
}
