//! Settings overlay panel — playback, transition, display, and window options.

use egui::{Align2, Context};

use super::OverlayAction;
use crate::config::{Config, FitMode, TIMER_MIN, TransitionMode};

pub(super) fn render_settings(
    ctx: &Context,
    config: &mut Config,
    show: &mut bool,
) -> Option<OverlayAction> {
    let mut action = None;

    egui::Window::new("Settings")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(true)
        .open(show)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);

            ui.heading("Playback");
            ui.horizontal(|ui| {
                ui.label("Timer (sec):");
                if ui
                    .add(
                        egui::DragValue::new(&mut config.viewer.timer)
                            .speed(TIMER_MIN)
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
            if ui
                .checkbox(&mut config.viewer.scan_subfolders, "Scan Subfolders")
                .changed()
            {
                action = Some(OverlayAction::ToggleScanSubfolders(
                    config.viewer.scan_subfolders,
                ));
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
            if !config.transition.random {
                ui.horizontal(|ui| {
                    ui.label("Transition Mode:");
                    let mut mode_val: i32 = config.transition.mode.into();
                    if ui
                        .add(
                            egui::Slider::new(&mut mode_val, 0..=19)
                                .custom_formatter(|n, _| {
                                    TransitionMode::try_from(n as i32)
                                        .map(|m| format!("{} — {}", n as i32, m.name()))
                                        .unwrap_or_else(|_| format!("{}", n as i32))
                                })
                                .custom_parser(|s| s.trim().parse::<f64>().ok()),
                        )
                        .changed()
                    {
                        // Value comes from a bounded slider so try_from always succeeds.
                        if let Ok(m) = TransitionMode::try_from(mode_val) {
                            config.transition.mode = m;
                            action = Some(OverlayAction::SetTransitionMode(m));
                        }
                    }
                });
            }

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
                    .selectable_value(&mut config.viewer.fit_mode, FitMode::AmbientFit, "Ambient")
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
                        action = Some(OverlayAction::SetAmbientBlur(config.viewer.ambient_blur));
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

    action
}
