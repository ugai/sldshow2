//! Screenshot capture from the rendered surface.

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

        if let Ok(Ok(())) = map_result {
            let data = buffer_slice.get_mapped_range();
            let filename = self.next_filename()?;

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
            let message = match map_result {
                Ok(Err(e)) => {
                    error!("Failed to map screenshot buffer: {e}");
                    "Screenshot failed: GPU map failed.".to_string()
                }
                Err(reason) => {
                    error!("Failed to map screenshot buffer: {reason}");
                    format!("Screenshot failed: {reason}.")
                }
                Ok(Ok(())) => "Screenshot failed!".to_string(),
            };
            Err(message)
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
