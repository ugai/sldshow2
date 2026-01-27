/// Generate simple test images for sldshow2
/// These are programmatically generated and therefore CC0/public domain
use image::{ImageBuffer, Rgb, RgbImage};
use std::path::Path;

fn create_gradient_image(width: u32, height: u32, color1: Rgb<u8>, color2: Rgb<u8>) -> RgbImage {
    ImageBuffer::from_fn(width, height, |_x, y| {
        let t = y as f32 / height as f32;
        Rgb([
            (color1[0] as f32 + (color2[0] as f32 - color1[0] as f32) * t) as u8,
            (color1[1] as f32 + (color2[1] as f32 - color1[1] as f32) * t) as u8,
            (color1[2] as f32 + (color2[2] as f32 - color1[2] as f32) * t) as u8,
        ])
    })
}

fn create_solid_image(width: u32, height: u32, color: Rgb<u8>) -> RgbImage {
    ImageBuffer::from_pixel(width, height, color)
}

fn create_checkered_image(width: u32, height: u32, square_size: u32) -> RgbImage {
    ImageBuffer::from_fn(width, height, |x, y| {
        let square_x = x / square_size;
        let square_y = y / square_size;

        if (square_x + square_y) % 2 == 0 {
            Rgb([255, 255, 255])
        } else {
            Rgb([200, 200, 200])
        }
    })
}

fn main() {
    let output_dir = Path::new("assets/test_images");
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let width = 1920;
    let height = 1080;

    // Red to Yellow gradient
    let img = create_gradient_image(width, height, Rgb([255, 0, 0]), Rgb([255, 255, 0]));
    img.save(output_dir.join("01_red_yellow_gradient.png"))
        .expect("Failed to save image");
    println!("Created: 01_red_yellow_gradient.png");

    // Blue to Green gradient
    let img = create_gradient_image(width, height, Rgb([0, 100, 255]), Rgb([0, 255, 100]));
    img.save(output_dir.join("02_blue_green_gradient.png"))
        .expect("Failed to save image");
    println!("Created: 02_blue_green_gradient.png");

    // Purple to Pink gradient
    let img = create_gradient_image(width, height, Rgb([128, 0, 128]), Rgb([255, 192, 203]));
    img.save(output_dir.join("03_purple_pink_gradient.png"))
        .expect("Failed to save image");
    println!("Created: 03_purple_pink_gradient.png");

    // Solid Blue
    let img = create_solid_image(width, height, Rgb([50, 50, 200]));
    img.save(output_dir.join("04_solid_blue.png"))
        .expect("Failed to save image");
    println!("Created: 04_solid_blue.png");

    // Solid Red
    let img = create_solid_image(width, height, Rgb([200, 50, 50]));
    img.save(output_dir.join("05_solid_red.png"))
        .expect("Failed to save image");
    println!("Created: 05_solid_red.png");

    // Solid Green
    let img = create_solid_image(width, height, Rgb([50, 200, 50]));
    img.save(output_dir.join("06_solid_green.png"))
        .expect("Failed to save image");
    println!("Created: 06_solid_green.png");

    // Checkered pattern
    let img = create_checkered_image(width, height, 100);
    img.save(output_dir.join("07_checkered_pattern.png"))
        .expect("Failed to save image");
    println!("Created: 07_checkered_pattern.png");

    println!("\nGenerated 7 test images in assets/test_images/");
    println!("These images are programmatically generated and are in the public domain (CC0).");
}
