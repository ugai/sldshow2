//! Screenshot capture from the rendered surface.

use log::{error, info};

pub struct ScreenshotCapture {
    counter: u32,
}

impl ScreenshotCapture {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Capture the current frame to a PNG file.
    /// Returns `Ok(filename)` on success, `Err(message)` on failure.
    /// The caller is responsible for submitting `render_encoder` — this method
    /// submits both the render encoder and an internal copy encoder together.
    pub fn capture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_encoder: wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Result<String, String> {
        let width = surface_config.width;
        let height = surface_config.height;
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        // wgpu requires 256-byte row alignment for buffer copies
        let padded_bytes_per_row = (unpadded_bytes_per_row + 255) & !255;

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot Staging Buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut copy_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screenshot Copy Encoder"),
        });

        copy_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit([render_encoder.finish(), copy_encoder.finish()]);

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = device.poll(wgpu::PollType::Wait);

        if let Ok(Ok(())) = receiver.recv() {
            let data = buffer_slice.get_mapped_range();
            let filename = self.next_filename();

            // Remove row padding and copy pixel data
            let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
            for row in 0..height {
                let start = (row * padded_bytes_per_row) as usize;
                let end = start + unpadded_bytes_per_row as usize;
                pixels.extend_from_slice(&data[start..end]);
            }
            drop(data);
            staging_buffer.unmap();

            // Handle BGRA surface formats (common on Windows)
            if matches!(
                surface_config.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) {
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }

            match image::save_buffer(&filename, &pixels, width, height, image::ColorType::Rgba8) {
                Ok(()) => {
                    info!("Screenshot saved: {}", filename);
                    Ok(filename)
                }
                Err(e) => {
                    error!("Failed to save screenshot: {}", e);
                    Err("Screenshot failed!".to_string())
                }
            }
        } else {
            error!("Failed to map screenshot buffer");
            Err("Screenshot failed!".to_string())
        }
    }

    fn next_filename(&mut self) -> String {
        loop {
            self.counter += 1;
            let filename = format!("sldshow-shot{:04}.png", self.counter);
            if !std::path::Path::new(&filename).exists() {
                return filename;
            }
        }
    }
}
