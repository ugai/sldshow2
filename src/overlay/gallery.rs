//! Gallery overlay panel — thumbnail grid for quick image navigation.

use egui::Context;
use std::collections::HashMap;

use super::{OverlayAction, OverlayKind};
use crate::image_loader::TextureManager;
use crate::thumbnail::ThumbnailManager;

pub(super) fn render_gallery(
    ctx: &Context,
    texture_manager: &TextureManager,
    thumbnail_manager: &mut ThumbnailManager,
    gallery_textures: &mut HashMap<usize, egui::TextureHandle>,
    show_gallery: &mut bool,
    overlay_stack: &mut Vec<OverlayKind>,
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

        let scroll_bar_width = ui.style().spacing.scroll.bar_width;
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
                        let texture_id = if let Some(img) = thumbnail_manager.get_thumbnail(index) {
                            // Create or get TextureHandle
                            let handle = gallery_textures.entry(index).or_insert_with(|| {
                                // Convert RgbaImage to ColorImage.
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
                        let has_texture = gallery_textures.contains_key(&index);

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
                            *show_gallery = false;
                            overlay_stack.retain(|k| *k != OverlayKind::Gallery);
                        }
                    }
                });
            }
        });
    });

    // Close on Escape
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *show_gallery = false;
        overlay_stack.retain(|k| *k != OverlayKind::Gallery);
    }

    action
}
