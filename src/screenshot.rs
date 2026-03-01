//! Screenshot capture from the rendered surface.

use half::f16;
use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Maximum number of filename candidates tried before giving up.
const MAX_FILENAME_ATTEMPTS: u32 = 10;
/// Maximum duration to wait for GPU map completion during screenshot capture.
const SCREENSHOT_MAP_TIMEOUT: Duration = Duration::from_secs(2);
/// Sleep interval between non-blocking map status polls.
const SCREENSHOT_MAP_POLL_INTERVAL: Duration = Duration::from_millis(5);

pub struct ScreenshotCapture;

impl ScreenshotCapture {
    pub fn new() -> Self {
        Self
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
        let (bytes_per_pixel, is_bgra, is_rgba16f) = match surface_config.format {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                (4u32, false, false)
            }
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                (4u32, true, false)
            }
            wgpu::TextureFormat::Rgba16Float => (8u32, false, true),
            format => {
                error!("Unsupported surface format for screenshot: {format:?}");
                return Err(format!(
                    "Screenshot failed: unsupported surface format ({format:?})."
                ));
            }
        };
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
        let wait_started = Instant::now();
        let map_result = loop {
            let _ = device.poll(wgpu::PollType::Poll);
            match receiver.try_recv() {
                Ok(result) => break Ok(result),
                Err(TryRecvError::Empty) => {
                    if wait_started.elapsed() >= SCREENSHOT_MAP_TIMEOUT {
                        break Err("Timed out waiting for screenshot GPU map".to_string());
                    }
                    std::thread::sleep(SCREENSHOT_MAP_POLL_INTERVAL);
                }
                Err(TryRecvError::Disconnected) => {
                    break Err("Screenshot map callback channel disconnected".to_string());
                }
            }
        };

        match map_result {
            Ok(Ok(())) => {
                let data = buffer_slice.get_mapped_range();
                let filename = self.next_filename()?;

                // Remove row padding and convert to RGBA8 pixels.
                let mut pixels = Vec::with_capacity((width * height * 4) as usize);
                for row in 0..height {
                    let start = (row * padded_bytes_per_row) as usize;
                    let end = start + unpadded_bytes_per_row as usize;
                    let row_data = &data[start..end];

                    if is_rgba16f {
                        for pixel in row_data.chunks_exact(8) {
                            let r =
                                f16::from_bits(u16::from_ne_bytes([pixel[0], pixel[1]])).to_f32();
                            let g =
                                f16::from_bits(u16::from_ne_bytes([pixel[2], pixel[3]])).to_f32();
                            let b =
                                f16::from_bits(u16::from_ne_bytes([pixel[4], pixel[5]])).to_f32();
                            let a = f16::from_bits(u16::from_ne_bytes([pixel[6], pixel[7]]))
                                .to_f32()
                                .clamp(0.0, 1.0);

                            pixels.extend_from_slice(&[
                                linear_hdr_to_srgb_u8(r),
                                linear_hdr_to_srgb_u8(g),
                                linear_hdr_to_srgb_u8(b),
                                (a * 255.0).round() as u8,
                            ]);
                        }
                    } else {
                        for pixel in row_data.chunks_exact(4) {
                            if is_bgra {
                                pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                            } else {
                                pixels.extend_from_slice(pixel);
                            }
                        }
                    }
                }

                drop(data);
                staging_buffer.unmap();

                match image::save_buffer(&filename, &pixels, width, height, image::ColorType::Rgba8)
                {
                    Ok(()) => {
                        info!("Screenshot saved: {}", filename);
                        Ok(filename)
                    }
                    Err(e) => {
                        error!("Failed to save screenshot: {}", e);
                        Err("Screenshot failed!".to_string())
                    }
                }
            }
            Ok(Err(e)) => {
                error!("Failed to map screenshot buffer: {e}");
                Err("Screenshot failed: GPU map failed.".to_string())
            }
            Err(reason) => {
                error!("Failed to map screenshot buffer: {reason}");
                Err(format!("Screenshot failed: {reason}."))
            }
        }
    }

    /// Resolve the directory where screenshots should be saved.
    ///
    /// Priority:
    /// 1. System Pictures directory (`dirs::picture_dir()`)
    /// 2. Documents directory (`dirs::document_dir()`)
    /// 3. Home directory (`dirs::home_dir()`)
    /// 4. Temporary directory (`std::env::temp_dir()`) — always writable
    fn screenshot_dir() -> PathBuf {
        if let Some(pictures) = dirs::picture_dir() {
            return pictures;
        }
        warn!("Pictures directory unavailable; falling back to documents directory");
        if let Some(documents) = dirs::document_dir() {
            return documents;
        }
        warn!("Documents directory unavailable; falling back to home directory");
        if let Some(home) = dirs::home_dir() {
            return home;
        }
        warn!("Home directory unavailable; falling back to temporary directory");
        std::env::temp_dir()
    }

    /// Generate a unique screenshot path using a millisecond-precision
    /// timestamp.  If the timestamp-derived name already exists (e.g. two
    /// screenshots in the same millisecond), a numeric suffix is appended.
    /// Returns `Err` if no free slot is found within [`MAX_FILENAME_ATTEMPTS`]
    /// tries, or if the system clock is unavailable.
    fn next_filename(&self) -> Result<String, String> {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .map_err(|_| "Screenshot failed!".to_string())?;

        let dir = Self::screenshot_dir();

        for attempt in 0..MAX_FILENAME_ATTEMPTS {
            let name = if attempt == 0 {
                format!("sldshow-shot-{ms}.png")
            } else {
                format!("sldshow-shot-{ms}-{attempt}.png")
            };
            let path = dir.join(&name);
            if !path.exists() {
                return Ok(path.to_string_lossy().into_owned());
            }
        }

        error!("No free screenshot filename found after {MAX_FILENAME_ATTEMPTS} attempts");
        Err("Screenshot failed!".to_string())
    }
}

fn linear_hdr_to_srgb_u8(linear: f32) -> u8 {
    let mapped = linear.max(0.0) / (1.0 + linear.max(0.0));
    let srgb = if mapped <= 0.003_130_8 {
        12.92 * mapped
    } else {
        1.055 * mapped.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}
