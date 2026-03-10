//! Help overlay panel — keyboard shortcuts reference.

use egui::{Align2, Color32, Context, RichText};

pub(super) fn render_help(ctx: &Context) {
    egui::Window::new("Keyboard Shortcuts")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
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
                    ui.label("Alt+0              Half image size");
                    ui.label("Alt+1              Original image size (1:1 pixels)");
                    ui.label("Alt+2              Double image size");

                    ui.add_space(4.0);
                    ui.heading("Zoom");
                    ui.label("Ctrl+Wheel         Zoom in/out");
                    ui.label("Z                  Reset zoom/pan");
                    ui.label("Drag (when zoomed) Pan image");

                    ui.add_space(4.0);
                    ui.heading("Color Adjustments");
                    ui.label("1 / 2              Contrast -/+");
                    ui.label("3 / 4              Brightness -/+");
                    ui.label("5 / 6              Gamma -/+");
                    ui.label("7 / 8              Saturation -/+");
                    ui.label("Shift+Backspace    Reset all color adjustments");

                    ui.add_space(4.0);
                    ui.heading("Actions");
                    ui.label("S                  Take screenshot");
                    ui.label("Ctrl+Shift+C       Copy image to clipboard");
                    ui.label("Ctrl+C             Copy path to clipboard");
                    ui.label("G                  Toggle Gallery");
                    ui.label("Alt+E              Open in Explorer");
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
