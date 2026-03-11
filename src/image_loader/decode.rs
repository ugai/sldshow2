//! Image decoding, mipmap generation, and GPU-ready resizing.

use camino::Utf8Path;
use image::GenericImageView;

use super::MipData;
use super::exif::{apply_orientation, read_exif_orientation};

/// Converts a linear light value to sRGB using the IEC 61966-2-1 piecewise transfer function.
/// This is more accurate than the simple gamma 2.2 approximation, especially for near-black values.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// Helper to perform fast resizing using fast_image_resize
pub(crate) fn fast_resize(
    src_img: fast_image_resize::images::Image,
    dst_width: u32,
    dst_height: u32,
    filter: fast_image_resize::FilterType,
) -> anyhow::Result<image::RgbaImage> {
    // Create destination image
    let mut dst_img = fast_image_resize::images::Image::new(
        dst_width,
        dst_height,
        fast_image_resize::PixelType::U8x4,
    );

    // Create resizer
    let mut resizer = fast_image_resize::Resizer::new();
    let resize_opts = fast_image_resize::ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(filter));

    // Resize
    resizer
        .resize(&src_img, &mut dst_img, &resize_opts)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // Convert back to image::RgbaImage
    let buffer = dst_img.into_vec();
    image::RgbaImage::from_raw(dst_width, dst_height, buffer)
        .ok_or_else(|| anyhow::anyhow!("from_raw failed"))
}

pub(super) fn load_image_mips(
    path: &Utf8Path,
    max_size: (u32, u32),
    is_hdr: bool,
) -> anyhow::Result<MipData> {
    use std::fs::File;
    use std::io::{BufReader, Seek, SeekFrom};

    let file = File::open(path.as_std_path())
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;
    let mut reader = BufReader::new(file);

    // Read EXIF orientation before decoding so we open the file only once.
    let orientation = read_exif_orientation(&mut reader);
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| anyhow::anyhow!("Failed to seek image: {}", e))?;

    let img = image::ImageReader::new(&mut reader)
        .with_guessed_format()
        .map_err(|e| anyhow::anyhow!("Failed to guess image format: {}", e))?
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;

    let is_exr = path.extension().unwrap_or("").eq_ignore_ascii_case("exr");

    if is_hdr && is_exr {
        // HDR path: keep linear float data as Rgba32F, skip sRGB conversion.
        // GPU upload will convert f32 → f16 for Rgba16Float texture.
        let rgba32f = img.into_rgba32f();
        let img = apply_orientation(image::DynamicImage::ImageRgba32F(rgba32f), orientation);
        let base = resize_for_gpu_hdr(img.into_rgba32f(), max_size.0, max_size.1);

        // Generate mip chain using image::imageops (fast_image_resize is U8 only)
        let mip_count = mip_level_count(base.width(), base.height());
        let mut mips: Vec<image::Rgba32FImage> = Vec::with_capacity(mip_count as usize);
        mips.push(base);

        for _ in 1..mip_count {
            let prev = mips.last().expect("mip chain is non-empty");
            let new_w = (prev.width() / 2).max(1);
            let new_h = (prev.height() / 2).max(1);
            let resized =
                image::imageops::resize(prev, new_w, new_h, image::imageops::FilterType::Triangle);
            mips.push(resized);
        }

        Ok(MipData::Hdr(mips))
    } else {
        // SDR path: existing behavior — tonemap EXR to sRGB, upload as Rgba8UnormSrgb.
        let mut img = img;
        if is_exr {
            // Apply the IEC 61966-2-1 piecewise sRGB transfer function per channel
            let mut rgba32f = img.into_rgba32f();
            for pixel in rgba32f.pixels_mut() {
                pixel[0] = linear_to_srgb(pixel[0].max(0.0));
                pixel[1] = linear_to_srgb(pixel[1].max(0.0));
                pixel[2] = linear_to_srgb(pixel[2].max(0.0));
                // Alpha remains linear
            }
            img = image::DynamicImage::ImageRgba32F(rgba32f);
        }

        let img = apply_orientation(img, orientation);
        let base = resize_for_gpu(img, max_size.0, max_size.1)?.into_rgba8();

        // Generate mipmap chain on CPU
        let mip_count = mip_level_count(base.width(), base.height());
        let mut mips = Vec::with_capacity(mip_count as usize);
        mips.push(base);

        for _ in 1..mip_count {
            let prev = mips
                .last()
                .ok_or_else(|| anyhow::anyhow!("mip chain is empty"))?;
            let new_w = (prev.width() / 2).max(1);
            let new_h = (prev.height() / 2).max(1);

            let mut prev_clone = prev.clone();

            // Fast image resize wrapper creation
            let src_image = fast_image_resize::images::Image::from_slice_u8(
                prev.width(),
                prev.height(),
                prev_clone.as_mut(),
                fast_image_resize::PixelType::U8x4,
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            let resized = fast_resize(
                src_image,
                new_w,
                new_h,
                fast_image_resize::FilterType::Bilinear,
            )?;
            mips.push(resized);
        }

        Ok(MipData::Sdr(mips))
    }
}

pub(super) fn mip_level_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    max_dim.ilog2() + 1
}

fn resize_for_gpu(
    img: image::DynamicImage,
    max_width: u32,
    max_height: u32,
) -> anyhow::Result<image::DynamicImage> {
    // [0, 0] means "no limit" — upload at full resolution
    if max_width == 0 || max_height == 0 {
        return Ok(img);
    }
    let (orig_w, orig_h) = img.dimensions();
    if orig_w <= max_width && orig_h <= max_height {
        return Ok(img);
    }
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);
    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);

    let mut rgba_img = img.into_rgba8();
    let src_image = fast_image_resize::images::Image::from_slice_u8(
        orig_w,
        orig_h,
        rgba_img.as_mut(),
        fast_image_resize::PixelType::U8x4,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let resized = fast_resize(
        src_image,
        new_w,
        new_h,
        fast_image_resize::FilterType::Lanczos3,
    )?;
    Ok(image::DynamicImage::ImageRgba8(resized))
}

/// Resize an HDR (Rgba32F) image to fit within max_width×max_height, preserving aspect ratio.
///
/// Uses `fast_image_resize` with `F32x4` pixel type for SIMD-accelerated bilinear interpolation
/// directly on f32 data — no precision loss or format conversion overhead.
/// Falls back to `image::imageops::resize` if the fast path fails.
fn resize_for_gpu_hdr(
    img: image::Rgba32FImage,
    max_width: u32,
    max_height: u32,
) -> image::Rgba32FImage {
    if max_width == 0 || max_height == 0 {
        return img;
    }
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w <= max_width && orig_h <= max_height {
        return img;
    }
    let scale_w = max_width as f32 / orig_w as f32;
    let scale_h = max_height as f32 / orig_h as f32;
    let scale = scale_w.min(scale_h);
    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);

    let fallback =
        || image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle);

    // Cast the f32 pixel buffer directly to bytes — no conversion needed for F32x4.
    let f32_bytes: &[u8] = bytemuck::cast_slice(img.as_raw());

    let src = match fast_image_resize::images::ImageRef::new(
        orig_w,
        orig_h,
        f32_bytes,
        fast_image_resize::PixelType::F32x4,
    ) {
        Ok(s) => s,
        Err(_) => return fallback(),
    };

    let mut dst =
        fast_image_resize::images::Image::new(new_w, new_h, fast_image_resize::PixelType::F32x4);

    let mut resizer = fast_image_resize::Resizer::new();
    let opts = fast_image_resize::ResizeOptions::new().resize_alg(
        fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
    );
    if resizer.resize(&src, &mut dst, &opts).is_err() {
        return fallback();
    }

    let f32_pixels: Vec<f32> = bytemuck::cast_slice(dst.buffer()).to_vec();
    image::Rgba32FImage::from_raw(new_w, new_h, f32_pixels).unwrap_or_else(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- mip_level_count() ---

    #[test]
    fn mip_level_count_1x1() {
        assert_eq!(mip_level_count(1, 1), 1);
    }

    #[test]
    fn mip_level_count_1024x1024() {
        // ilog2(1024) = 10, so 11 levels
        assert_eq!(mip_level_count(1024, 1024), 11);
    }

    #[test]
    fn mip_level_count_non_square_uses_max_dim() {
        // max dim = 1920, ilog2(1920) = 10, so 11 levels
        assert_eq!(mip_level_count(1920, 1080), 11);
    }

    // --- linear_to_srgb() ---

    #[test]
    fn linear_to_srgb_zero_maps_to_zero() {
        assert_eq!(linear_to_srgb(0.0), 0.0);
    }

    #[test]
    fn linear_to_srgb_one_maps_to_one() {
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn linear_to_srgb_low_value_uses_linear_segment() {
        // Values <= 0.003_130_8 use the linear c * 12.92 branch
        let v = 0.001_f32;
        assert!((linear_to_srgb(v) - v * 12.92).abs() < 1e-6);
    }
}
