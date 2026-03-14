//! On-Screen Controller (OSC) for playback controls.
//!
//! Provides interactive buttons for play/pause, navigation, and shuffle.

use egui::{Align2, Color32, Context, FontId, Stroke, Vec2};
use std::time::{Duration, Instant};

// Import Phosphor icons
use egui_phosphor::regular as Icon;

/// Auto-hide timeout in seconds
const OSC_TIMEOUT: f32 = 2.0;

/// Minimum vertical margin from bottom edge
const OSC_BOTTOM_MARGIN: f32 = 50.0;

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

/// OSC state and rendering
pub struct Osc {
    /// Last mouse activity time
    pub last_interaction: Instant,
    /// Whether OSC is currently visible
    pub visible: bool,
    /// Whether mouse is hovering over OSC
    pub hovering: bool,
}

impl Osc {
    pub fn new() -> Self {
        Self {
            last_interaction: Instant::now(),
            visible: true,
            hovering: false,
        }
    }

    /// Update activity timer (call on mouse movement)
    pub fn update_interaction(&mut self) {
        self.last_interaction = Instant::now();
        self.visible = true;
    }

    /// Update visibility based on timeout
    pub fn check_autohide(&mut self) {
        if !self.hovering && self.last_interaction.elapsed() > Duration::from_secs_f32(OSC_TIMEOUT)
        {
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
                    .fill(Color32::from_black_alpha(220)) // Slightly darker for contrast
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 12,
                        bottom: 12,
                    })
                    .corner_radius(12.0)
                    .stroke(Stroke::new(1.0, Color32::from_white_alpha(30)));

                frame.show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Scrub Bar
                        if total_images > 1 {
                            if let Some(seek_index) =
                                self.render_scrub_bar(ui, current_index, total_images)
                            {
                                action = Some(OscAction::Seek(seek_index));
                            }
                            ui.add_space(10.0);
                        }

                        // Buttons
                        ui.horizontal(|ui| {
                            let item_height = 32.0;
                            ui.spacing_mut().item_spacing.x = 16.0;

                            // Settings button (leftmost)
                            if self.icon_button(ui, item_height, Icon::GEAR, false) {
                                action = Some(OscAction::OpenSettings);
                            }

                            ui.add_space(8.0);

                            // Previous button
                            if self.icon_button(ui, item_height, Icon::CARET_LEFT, false) {
                                action = Some(OscAction::Previous);
                            }

                            // Play/Pause button (highlighted)
                            let play_icon = if paused { Icon::PLAY } else { Icon::PAUSE };
                            if self.icon_button(ui, item_height, play_icon, true) {
                                action = Some(OscAction::PlayPause);
                            }

                            // Next button
                            if self.icon_button(ui, item_height, Icon::CARET_RIGHT, false) {
                                action = Some(OscAction::Next);
                            }

                            ui.add_space(8.0);

                            // Shuffle toggle (with visual state)
                            if self.icon_button(ui, item_height, Icon::SHUFFLE, shuffle) {
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

    /// Render a flexible icon button using custom painting
    fn icon_button(&self, ui: &mut egui::Ui, height: f32, icon: &str, active: bool) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(height), egui::Sense::click());

        // Hover animation
        let visuals = ui.style().interact(&response);
        let bg_color = if active {
            Color32::from_rgb(60, 160, 100)
        } else if response.hovered() {
            Color32::from_white_alpha(40)
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 8.0, bg_color);
        }

        // Icon painting
        let size = height * 0.7; // Icon size relative to button
        let color = if active {
            Color32::WHITE
        } else {
            visuals.text_color()
        };

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(size),
            color,
        );

        response.clicked()
    }

    /// Render video-style timeline scrub bar
    // `egui::show_tooltip` is deprecated in egui 0.29+ in favour of
    // `Response::on_hover_ui`, but our usage requires a non-`Response`-owned
    // tooltip positioned relative to the scrub bar area.
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

        // Hover tooltip (only when pointer position is available)
        if response.hovered()
            && let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos())
        {
            let mouse_x = hover_pos.x;
            let t = ((mouse_x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let hover_index = (t * (total_images - 1) as f32).round() as usize; // 0-based index

            // Show 1-based index for user
            egui::show_tooltip(ui.ctx(), ui.layer_id(), response.id, |ui| {
                ui.label(format!("{} / {}", hover_index + 1, total_images));
            });
        }

        // Interaction (Click or Drag)
        if (response.clicked() || response.dragged())
            && let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos())
        {
            let t = ((mouse_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let new_index = (t * (total_images - 1) as f32).round() as usize;

            // Only return if valid change
            if new_index != current_index && new_index < total_images {
                return Some(new_index);
            }
        }

        None
    }
}
