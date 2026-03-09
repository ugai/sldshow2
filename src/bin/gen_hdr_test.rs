//! Generate a half-float EXR test image for HDR swapchain verification.
//!
//! Produces `tools/test_hdr_gradient.exr` — a 1920×1080 linear-float image:
//!   - Horizontal luminance gradient from 0.0 to 4.0 across the full width
//!   - Reference patches at 1.0 (SDR white), 2.0×, and 4.0× peak brightness
//!
//! Usage:
//!   cargo run --bin gen_hdr_test
//!   $env:RUST_LOG="info"; cargo run --release -- tools/test_hdr_gradient.exr

use std::path::PathBuf;

use image::{Rgba, Rgba32FImage};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn main() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("test_hdr_gradient.exr");

    let mut img = Rgba32FImage::new(WIDTH, HEIGHT);

    // Horizontal gradient: column 0 → 0.0, column WIDTH-1 → 4.0
    for (x, _y, px) in img.enumerate_pixels_mut() {
        let v = x as f32 / (WIDTH - 1) as f32 * 4.0;
        *px = Rgba([v, v, v, 1.0]);
    }

    // Reference patches at SDR white (1.0), 2× and 4× peak
    let patch_w = WIDTH / 10;
    let patch_h = HEIGHT / 6;
    let patch_top = HEIGHT / 2 - patch_h / 2;

    for &value in &[1.0_f32, 2.0, 4.0] {
        let cx = ((value / 4.0) * (WIDTH - 1) as f32) as u32;
        let patch_left = cx.saturating_sub(patch_w / 2).min(WIDTH - patch_w);

        let outline_v = (value * 1.5).min(4.0);
        let outline_top = patch_top.saturating_sub(2);
        let outline_left = patch_left.saturating_sub(2);
        let outline_bottom = (patch_top + patch_h + 2).min(HEIGHT);
        let outline_right = (patch_left + patch_w + 2).min(WIDTH);

        for y in outline_top..outline_bottom {
            for x in outline_left..outline_right {
                img.put_pixel(x, y, Rgba([outline_v, outline_v, outline_v, 1.0]));
            }
        }
        for y in patch_top..patch_top + patch_h {
            for x in patch_left..patch_left + patch_w {
                img.put_pixel(x, y, Rgba([value, value, value, 1.0]));
            }
        }
    }

    std::fs::create_dir_all(output.parent().unwrap()).expect("Failed to create tools/ dir");
    img.save(&output).expect("Failed to write EXR");

    println!(
        "Written: {}  ({}×{}, linear float32 RGBA EXR)",
        output.display(),
        WIDTH,
        HEIGHT
    );
    println!("Test with:  cargo run --release -- tools/test_hdr_gradient.exr");
}
