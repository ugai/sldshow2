use anyhow::Result;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    keyboard::KeyCode,
};

use crate::clipboard;
use crate::config::{self, Config};
use crate::drag_drop::DragDropHandler;
use crate::image_loader::{self, TextureManager};
use crate::input::{InputAction, InputContext, InputHandler};
use crate::osc::OscAction;
use crate::overlay::EguiOverlay;
use crate::renderer::Renderer;
use crate::screenshot::ScreenshotCapture;
use crate::thumbnail::ThumbnailManager;
use crate::timer::{SequenceTimer, SlideshowTimer};
use crate::transition::{TransitionPipeline, TransitionUniform};

pub struct ApplicationState {
    config: Config,
    size: winit::dpi::PhysicalSize<u32>,
    window: Arc<winit::window::Window>,

    // GPU renderer (surface, device, queue, pipeline, uniform buffer, bind group)
    renderer: Renderer,

    // Subsystems
    texture_manager: TextureManager,
    thumbnail_manager: ThumbnailManager,
    slideshow: SlideshowTimer,
    sequence_timer: SequenceTimer,
    input_handler: InputHandler,
    egui_overlay: EguiOverlay,

    // Transition State
    transition: Option<ActiveTransition>,
    // The texture currently being displayed (when no transition active)
    current_texture_index: Option<usize>,

    // OSD (top-right, reactive feedback)
    osd_message: Option<(String, Instant)>,

    // Display toggles
    show_filename_text: bool,
    // Temporary displays — share the same area as their persistent counterparts
    filename_bar_temp_expiry: Option<Instant>, // o temp → same bottom-left as O persistent
    info_temp_expiry: Option<Instant>,         // i temp → same top-left as I persistent

    // Color adjustments (mpv-like)
    color_brightness: f32,
    color_contrast: f32,
    color_gamma: f32,
    color_saturation: f32,

    // Screenshot
    screenshot_requested: bool,
    screenshot: ScreenshotCapture,

    // Drag & drop
    drag_drop: DragDropHandler,

    // Shuffle state
    shuffle_enabled: bool,

    // Input state
    modifiers: winit::keyboard::ModifiersState,

    // Timer reset target — stores the config's original timer value
    initial_timer: f32,

    // Cached info overlay string — invalidated on image change
    cached_info_string: Option<String>,

    // Zoom/pan state
    zoom_scale: f32,
    zoom_pan: [f32; 2],

    // Deferred resize — set on WindowEvent::Resized, applied at the start of render()
    // to avoid reconfiguring the surface on every rapid resize event.
    pending_resize: Option<winit::dpi::PhysicalSize<u32>>,

    // Async clipboard support
    clipboard_receiver: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}

struct ActiveTransition {
    start_time: Instant,
    duration: Duration,
    mode: i32,
    from_index: usize,
    to_index: usize,
}

impl ApplicationState {
    pub async fn new(
        window: Arc<winit::window::Window>,
        config: Config,
        drag_drop: DragDropHandler,
    ) -> Result<Self> {
        let size = window.inner_size();

        // Initialize GPU renderer
        let renderer = Renderer::new(window.clone(), &config, size).await?;

        // Initialize Subsystems
        let cache_extent = if config.viewer.playback_mode == config::PlaybackMode::Sequence {
            config
                .viewer
                .cache_extent
                .max(config.viewer.sequence_fps as usize)
        } else {
            config.viewer.cache_extent
        };

        let mut texture_manager = TextureManager::new(
            cache_extent,
            (
                config.viewer.max_texture_size[0],
                config.viewer.max_texture_size[1],
            ),
        );
        texture_manager.is_hdr = renderer.is_hdr;

        // Scan images
        if let Err(e) =
            texture_manager.scan_paths(&config.viewer.image_paths, config.viewer.scan_subfolders)
        {
            warn!("Failed to scan paths: {}", e);
        }

        if config.viewer.shuffle {
            texture_manager.shuffle_paths();
        }

        let thumbnail_manager = ThumbnailManager::new(200);

        let slideshow = SlideshowTimer::new(config.viewer.timer);
        let mut sequence_timer = SequenceTimer::new(config.viewer.sequence_fps);

        // Auto-detect EXR framerate for sequence playback
        if config.viewer.playback_mode == config::PlaybackMode::Sequence {
            if let Some(detected_fps) = texture_manager.detect_sequence_fps() {
                info!(
                    "Detected EXR framerate: {:.2} fps (overriding config value {})",
                    detected_fps, config.viewer.sequence_fps
                );
                sequence_timer.set_fps(detected_fps);
            }
        }

        // Initialize egui overlay
        let mut egui_overlay = EguiOverlay::new(
            &renderer.device,
            renderer.format(),
            window.clone(),
            config.style.font_family.clone(),
        );
        // Apply style config
        egui_overlay.set_style(config.style.font_size, config.style.text_color);

        // Initialize state
        let show_filename_text = config.style.show_image_path;
        let current_texture_index = if texture_manager.len() > 0 {
            Some(0)
        } else {
            None
        };

        let shuffle_enabled = config.viewer.shuffle;
        let initial_timer = config.viewer.timer;

        let state = Self {
            config,
            size,
            window,
            renderer,
            texture_manager,
            thumbnail_manager,
            slideshow,
            sequence_timer,
            input_handler: InputHandler::new(),
            egui_overlay,
            transition: None,
            current_texture_index,
            osd_message: None,
            show_filename_text,
            filename_bar_temp_expiry: None,
            info_temp_expiry: None,
            color_brightness: 0.0,
            color_contrast: 1.0,
            color_gamma: 1.0,
            color_saturation: 1.0,
            screenshot_requested: false,
            screenshot: ScreenshotCapture::new(),
            drag_drop,
            shuffle_enabled,
            modifiers: winit::keyboard::ModifiersState::default(),
            initial_timer,
            cached_info_string: None,
            zoom_scale: 1.0,
            zoom_pan: [0.0, 0.0],
            pending_resize: None,
            clipboard_receiver: None,
        };

        state.update_window_title();
        Ok(state)
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.renderer.resize(new_size);
            self.egui_overlay.resize(new_size.width, new_size.height);
        }
    }

    fn input(
        &mut self,
        event: &WindowEvent,
        modifiers: &winit::keyboard::ModifiersState,
    ) -> (bool, bool) {
        let current_image_size = self
            .texture_manager
            .get_current_texture()
            .map(|t| (t.width, t.height));
        let ctx = InputContext {
            fullscreen: self.window.fullscreen().is_some(),
            image_count: self.texture_manager.len(),
            help_visible: self.egui_overlay.help_overlay_visible(),
            settings_visible: self.egui_overlay.settings_visible(),
            gallery_visible: self.egui_overlay.gallery_visible(),
            current_image_size,
        };
        let (consumed, action) =
            self.input_handler
                .handle_event(event, modifiers, &self.window, &ctx);

        // Update OSC activity on cursor movement
        if matches!(event, WindowEvent::CursorMoved { .. }) {
            self.egui_overlay.update_osc_activity();
        }

        // Sync cursor visibility state with window
        if self.input_handler.cursor_visible {
            self.window.set_cursor_visible(true);
        }

        let mut should_exit = false;
        if let Some(action) = action {
            should_exit = self.execute_input_action(action);
        }

        (consumed, should_exit)
    }

    fn execute_osc_action(&mut self, action: OscAction) {
        match action {
            OscAction::PlayPause => {
                self.execute_input_action(InputAction::TogglePause);
            }
            OscAction::Previous => self.prev_image(),
            OscAction::Next => self.next_image(),
            OscAction::ToggleShuffle => {
                self.shuffle_enabled = !self.shuffle_enabled;
                // Sync config so checkbox updates
                self.config.viewer.shuffle = self.shuffle_enabled;
                let new_index = self
                    .texture_manager
                    .set_shuffle_enabled(self.shuffle_enabled);
                self.current_texture_index = Some(new_index);
                self.transition = None;
                self.renderer.invalidate_bind_group();
                let status = if self.shuffle_enabled {
                    "Shuffle: ON"
                } else {
                    "Shuffle: OFF"
                };
                info!("{}", status);
                self.show_osd(status.to_string());
            }
            OscAction::OpenSettings => {
                self.egui_overlay.toggle_settings();
            }
            OscAction::Seek(index) => {
                self.jump_to(index);
            }
        }
    }

    /// Executes an input action. Returns `true` if the application should exit.
    fn execute_input_action(&mut self, action: InputAction) -> bool {
        match action {
            InputAction::NextImage { steps } => {
                for _ in 0..steps {
                    self.next_image();
                }
            }
            InputAction::PrevImage { steps } => {
                for _ in 0..steps {
                    self.prev_image();
                }
            }
            InputAction::JumpTo(index) => self.jump_to(index),
            InputAction::TogglePause => {
                if self.config.viewer.playback_mode == config::PlaybackMode::Sequence {
                    self.sequence_timer.toggle_pause();
                    info!("Sequence paused: {}", self.sequence_timer.paused);
                    self.show_osd(
                        if self.sequence_timer.paused {
                            "Paused"
                        } else {
                            "Resumed"
                        }
                        .to_string(),
                    );
                } else {
                    self.slideshow.toggle_pause();
                    info!("Slideshow paused: {}", self.slideshow.paused);
                    self.show_osd(
                        if self.slideshow.paused {
                            "Paused"
                        } else {
                            "Resumed"
                        }
                        .to_string(),
                    );
                }
            }
            InputAction::ToggleFullscreen => {
                let fullscreen = self.window.fullscreen().is_some();
                let new_fullscreen = !fullscreen;
                self.config.window.fullscreen = new_fullscreen;
                self.window.set_fullscreen(if new_fullscreen {
                    Some(winit::window::Fullscreen::Borderless(None))
                } else {
                    None
                });
                self.show_osd(
                    if !new_fullscreen {
                        "Fullscreen: OFF"
                    } else {
                        "Fullscreen: ON"
                    }
                    .to_string(),
                );
            }
            InputAction::SetFullscreen(fullscreen) => {
                self.config.window.fullscreen = fullscreen;
                self.window.set_fullscreen(if fullscreen {
                    Some(winit::window::Fullscreen::Borderless(None))
                } else {
                    None
                });
                self.show_osd(
                    if fullscreen {
                        "Fullscreen: ON"
                    } else {
                        "Fullscreen: OFF"
                    }
                    .to_string(),
                );
            }
            InputAction::ToggleDecorations => {
                let decorated = self.window.is_decorated();
                self.window.set_decorations(!decorated);
                self.show_osd(
                    if !decorated {
                        "Decorations: ON"
                    } else {
                        "Decorations: OFF"
                    }
                    .to_string(),
                );
            }
            InputAction::ToggleAlwaysOnTop => {
                let always_on_top = !self.config.window.always_on_top;
                self.config.window.always_on_top = always_on_top;
                self.window.set_window_level(if always_on_top {
                    winit::window::WindowLevel::AlwaysOnTop
                } else {
                    winit::window::WindowLevel::Normal
                });
                self.show_osd(
                    if always_on_top {
                        "Always On Top: ON"
                    } else {
                        "Always On Top: OFF"
                    }
                    .to_string(),
                );
            }
            InputAction::AdjustTimer(mut delta) => {
                // Recalculate delta for non-Shift keys
                if delta.abs() == 1.0 {
                    delta = if delta > 0.0 {
                        self.timer_step(true)
                    } else {
                        -self.timer_step(false)
                    };
                }
                self.adjust_timer(delta);
            }
            InputAction::ResetTimer => self.reset_timer(),
            InputAction::Screenshot => self.screenshot_requested = true,
            InputAction::ColorAdjust { key } => self.handle_color_key(key),
            InputAction::ResetColorAdjustments => self.reset_color_adjustments(),
            InputAction::ToggleInfoOverlay => {
                let visible = self.egui_overlay.toggle_info_overlay();
                self.info_temp_expiry = None;
                if !visible {
                    self.egui_overlay.set_info_text("");
                }
                self.show_osd(if visible { "Info: ON" } else { "Info: OFF" }.to_string());
            }
            InputAction::ShowInfoTemporary => {
                if !self.egui_overlay.info_overlay_visible() {
                    let info = self.build_info_string();
                    self.egui_overlay.set_info_text(&info);
                    self.info_temp_expiry = Some(Instant::now() + Duration::from_millis(1500));
                }
            }
            InputAction::ToggleFilenameDisplay => {
                self.show_filename_text = !self.show_filename_text;
                self.filename_bar_temp_expiry = None;
                self.show_osd(
                    if self.show_filename_text {
                        "Filename: ON"
                    } else {
                        "Filename: OFF"
                    }
                    .to_string(),
                );
            }
            InputAction::ShowFilenameTemporary => {
                if !self.show_filename_text {
                    self.filename_bar_temp_expiry =
                        Some(Instant::now() + Duration::from_millis(1500));
                }
            }
            InputAction::ToggleLoop => {
                self.config.viewer.pause_at_last = !self.config.viewer.pause_at_last;
                let status = if self.config.viewer.pause_at_last {
                    "Loop: OFF"
                } else {
                    "Loop: ON"
                };
                info!("{}", status);
                self.show_osd(status.to_string());
            }
            InputAction::ToggleFitMode => {
                self.config.viewer.fit_mode.toggle();
                self.show_osd(
                    match self.config.viewer.fit_mode {
                        config::FitMode::Fit => "Fit: Normal",
                        config::FitMode::AmbientFit => "Fit: Ambient",
                    }
                    .to_string(),
                );
            }
            InputAction::SetWindowPosition { x, y } => {
                self.window
                    .set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            }
            InputAction::CopyImageToClipboard => {
                if self.texture_manager.paths.is_empty() {
                    self.show_osd("No Image Loaded".to_string());
                } else if self.clipboard_receiver.is_some() {
                    self.show_osd("Copying...".to_string());
                } else {
                    let current_path =
                        self.texture_manager.paths[self.texture_manager.current_index].clone();

                    let (tx, rx) = std::sync::mpsc::channel();
                    self.clipboard_receiver = Some(rx);
                    self.show_osd("Copying to Clipboard...".to_string());

                    std::thread::spawn(move || {
                        let res = match clipboard::copy_image_to_clipboard(&current_path) {
                            Ok(_) => Ok(()),
                            Err(e) => Err(format!("{}", e)),
                        };
                        let _ = tx.send(res);
                    });
                }
            }
            InputAction::ToggleHelpOverlay => {
                self.egui_overlay.toggle_help_overlay();
            }
            InputAction::ToggleSettings => {
                self.egui_overlay.toggle_settings();
            }
            InputAction::ToggleGallery => {
                self.egui_overlay.toggle_gallery();
            }
            InputAction::Exit => return true,
            InputAction::ResizeWindow { width, height } => {
                let _ = self
                    .window
                    .request_inner_size(winit::dpi::LogicalSize::new(width, height));
                self.show_osd(format!("Resize: {}x{}", width, height));
            }
            InputAction::CopyPathToClipboard => {
                if let Some(path) = self.texture_manager.current_path() {
                    match arboard::Clipboard::new() {
                        Ok(mut clipboard) => {
                            if let Err(e) = clipboard.set_text(path.as_str()) {
                                error!("Failed to copy to clipboard: {}", e);
                            } else {
                                info!("Copied path to clipboard: {}", path);
                                self.show_osd("Copied path to clipboard".to_string());
                            }
                        }
                        Err(e) => {
                            error!("Failed to initialize clipboard: {}", e);
                            self.show_osd("Clipboard Unavailable".to_string());
                        }
                    }
                }
            }
            InputAction::OpenInExplorer => self.open_explorer(),
            InputAction::Zoom { delta } => {
                let factor = if delta > 0.0 { 1.1f32 } else { 1.0 / 1.1 };
                self.zoom_scale = (self.zoom_scale * factor).clamp(1.0, 10.0);
                // Keep input_handler in sync so drag behavior is correct
                self.input_handler.zoom_scale = self.zoom_scale;
                // Clamp pan so image stays within viewport when zooming out
                self.clamp_zoom_pan();
                self.show_osd(format!("Zoom: {:.1}x", self.zoom_scale));
            }
            InputAction::Pan { dx, dy } => {
                // Convert physical pixel delta to UV-space delta
                let uv_dx = -dx / self.size.width as f32;
                let uv_dy = -dy / self.size.height as f32;
                self.zoom_pan[0] += uv_dx;
                self.zoom_pan[1] += uv_dy;
                self.clamp_zoom_pan();
            }
            InputAction::ResetZoom => {
                self.zoom_scale = 1.0;
                self.zoom_pan = [0.0, 0.0];
                self.input_handler.zoom_scale = 1.0;
                self.show_osd("Zoom: Reset".to_string());
            }
        }
        false
    }

    /// Clamp zoom_pan so the image edge is reachable but the image stays on screen.
    fn clamp_zoom_pan(&mut self) {
        let s = self.zoom_scale;
        if s <= 1.0 {
            self.zoom_pan = [0.0, 0.0];
            return;
        }

        // Mirror the contain-fit scale logic from adjust_uv() in the WGSL shader.
        // cx/cy < 1.0 on the letterboxed/pillarboxed axis; 1.0 on the full-dimension axis.
        let (cx, cy) = if let Some(tex) = self.texture_manager.get_current_texture() {
            let img_aspect = tex.width as f32 / tex.height as f32;
            let win_aspect = self.size.width as f32 / self.size.height as f32;
            if img_aspect > win_aspect {
                (1.0f32, win_aspect / img_aspect)
            } else {
                (img_aspect / win_aspect, 1.0f32)
            }
        } else {
            (1.0, 1.0)
        };

        // Correct per-axis limit: max_offset = 0.5 * (s*c - 1) / (s - 1)
        // When c = 1.0 this simplifies to 0.5 (independent of zoom).
        // When c < 1.0 (letterboxed axis) the limit is smaller.
        let max_x = (0.5 * (s * cx - 1.0) / (s - 1.0)).max(0.0);
        let max_y = (0.5 * (s * cy - 1.0) / (s - 1.0)).max(0.0);
        self.zoom_pan[0] = self.zoom_pan[0].clamp(-max_x, max_x);
        self.zoom_pan[1] = self.zoom_pan[1].clamp(-max_y, max_y);
    }

    fn next_image(&mut self) {
        let old_index = self.texture_manager.current_index;
        if self.texture_manager.next(self.config.viewer.pause_at_last) {
            self.finish_navigation(old_index);
        }
    }

    fn prev_image(&mut self) {
        let old_index = self.texture_manager.current_index;
        if self.texture_manager.previous() {
            self.finish_navigation(old_index);
        }
    }

    fn jump_to(&mut self, index: usize) {
        let old_index = self.texture_manager.current_index;
        if index < self.texture_manager.len() && index != old_index {
            self.texture_manager.jump_to(index);
            self.finish_navigation(old_index);
        }
    }

    fn finish_navigation(&mut self, old_index: usize) {
        if self.config.viewer.playback_mode == config::PlaybackMode::Sequence {
            self.current_texture_index = Some(self.texture_manager.current_index);
            self.transition = None;
            self.renderer.invalidate_bind_group();
        } else {
            self.start_transition(old_index, self.texture_manager.current_index);
        }
        self.slideshow.reset();
        self.sequence_timer.reset();
        self.update_window_title();
        self.cached_info_string = None;
        // Reset zoom/pan on every image change
        self.zoom_scale = 1.0;
        self.zoom_pan = [0.0, 0.0];
        self.input_handler.reset_zoom();
    }

    fn timer_step(&self, increasing: bool) -> f32 {
        let current = self.slideshow.interval_secs();
        if increasing && current < 5.0 || !increasing && current <= 5.0 {
            1.0
        } else {
            5.0
        }
    }

    fn adjust_timer(&mut self, delta: f32) {
        let new_timer = (self.slideshow.interval_secs() + delta).round().max(0.0);
        self.slideshow.set_duration(new_timer);
        self.config.viewer.timer = new_timer; // Sync to config
        if new_timer <= 0.0 {
            info!("Slideshow paused (timer: 0)");
            self.show_osd("Timer: 0.0s (Paused)".to_string());
        } else {
            info!("Slideshow timer set to: {:.1}s", new_timer);
            self.show_osd(format!("Timer: {:.1}s", new_timer));
        }
    }

    fn reset_timer(&mut self) {
        let default = self.initial_timer;
        self.slideshow.set_duration(default);
        self.config.viewer.timer = default; // Sync to config
        info!("Slideshow timer reset to: {:.1}s", default);
        self.show_osd(format!("Timer Reset: {:.1}s", default));
    }

    fn handle_color_key(&mut self, key: KeyCode) {
        let (value, delta, name, fmt) = match key {
            KeyCode::Digit1 => (&mut self.color_contrast, -0.05f32, "Contrast", "{:.2}"),
            KeyCode::Digit2 => (&mut self.color_contrast, 0.05, "Contrast", "{:.2}"),
            KeyCode::Digit3 => (&mut self.color_brightness, -0.05, "Brightness", "{:.2}"),
            KeyCode::Digit4 => (&mut self.color_brightness, 0.05, "Brightness", "{:.2}"),
            KeyCode::Digit5 => (&mut self.color_gamma, -0.1, "Gamma", "{:.1}"),
            KeyCode::Digit6 => (&mut self.color_gamma, 0.1, "Gamma", "{:.1}"),
            KeyCode::Digit7 => (&mut self.color_saturation, -0.05, "Saturation", "{:.2}"),
            KeyCode::Digit8 => (&mut self.color_saturation, 0.05, "Saturation", "{:.2}"),
            _ => return,
        };
        let (min, max) = match key {
            KeyCode::Digit1 | KeyCode::Digit2 => (0.0, 3.0),
            KeyCode::Digit3 | KeyCode::Digit4 => (-1.0, 1.0),
            KeyCode::Digit5 | KeyCode::Digit6 => (0.1, 5.0),
            KeyCode::Digit7 | KeyCode::Digit8 => (0.0, 3.0),
            _ => return,
        };
        *value = (*value + delta).clamp(min, max);
        let msg = if fmt == "{:.1}" {
            format!("{}: {:.1}", name, *value)
        } else {
            format!("{}: {:.2}", name, *value)
        };
        self.show_osd(msg);
    }

    fn reset_color_adjustments(&mut self) {
        self.color_brightness = 0.0;
        self.color_contrast = 1.0;
        self.color_gamma = 1.0;
        self.color_saturation = 1.0;
        self.show_osd("Color Reset".to_string());
    }

    fn build_info_string(&self) -> String {
        let Some(path) = self.texture_manager.current_path() else {
            return "No image loaded".to_string();
        };

        let resolution = if let Some(tex) = self.texture_manager.get_current_texture() {
            format!("{}x{}", tex.width, tex.height)
        } else {
            "Loading...".to_string()
        };

        let format = path.extension().unwrap_or("unknown").to_uppercase();

        let file_size = std::fs::metadata(path.as_std_path())
            .map(|m| {
                let bytes = m.len();
                if bytes >= 1_048_576 {
                    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
                } else if bytes >= 1024 {
                    format!("{:.1} KB", bytes as f64 / 1024.0)
                } else {
                    format!("{} B", bytes)
                }
            })
            .unwrap_or_else(|_| "Unknown size".to_string());

        format!("{}\n{} {}\n{}", path, resolution, format, file_size)
    }

    fn open_explorer(&mut self) {
        let Some(path) = self.texture_manager.current_path() else {
            self.show_osd("No image loaded".to_string());
            return;
        };

        let result = Self::spawn_explorer(path.as_std_path());

        match result {
            Ok(()) => self.show_osd("Opened in Explorer".to_string()),
            Err(e) => {
                error!("Failed to open explorer: {}", e);
                self.show_osd("Failed to open explorer".to_string());
            }
        }
    }

    fn spawn_explorer(path: &std::path::Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // canonicalize gives an absolute path with backslashes;
            // strip the \\?\ prefix that Windows canonicalize adds.
            let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            let abs_lossy = abs.to_string_lossy();
            let clean = abs_lossy.strip_prefix(r"\\?\").unwrap_or(&abs_lossy);
            info!("Opening explorer for: {}", clean);
            // raw_arg avoids Rust's auto-quoting so explorer sees /select,<path> verbatim
            std::process::Command::new("explorer")
                .raw_arg(format!("/select,\"{}\"", clean))
                .spawn()?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn()?;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let dir = path.parent().unwrap_or(path);
            std::process::Command::new("xdg-open").arg(dir).spawn()?;
        }
        Ok(())
    }

    fn update_window_title(&self) {
        if let Some(path) = self.texture_manager.current_path() {
            let filename = path.file_name().unwrap_or("Unknown");
            self.window.set_title(&format!("{} - sldshow2", filename));
        } else {
            self.window.set_title("sldshow2");
        }
    }
    fn start_transition(&mut self, from_index: usize, to_index: usize) {
        // If transition time is 0, do an instant cut to avoid division by zero
        // in the render loop (elapsed / duration would produce NaN).
        if self.config.transition.time == 0.0 {
            self.current_texture_index = Some(to_index);
            self.transition = None;
            self.renderer.invalidate_bind_group();
            return;
        }

        let mode = if self.config.transition.random {
            TransitionPipeline::random_mode()
        } else {
            self.config.transition.mode
        };

        self.transition = Some(ActiveTransition {
            start_time: Instant::now(),
            duration: Duration::from_secs_f32(self.config.transition.time),
            mode,
            from_index,
            to_index,
        });

        // Force bind group recreation
        self.renderer.invalidate_bind_group();
    }

    fn update(&mut self) {
        // Update OSC auto-hide logic
        self.egui_overlay.update_osc();

        // Begin egui frame
        self.egui_overlay.begin_frame(&self.window);
        let overlay_action = self.egui_overlay.build_ui(
            &mut self.config,
            self.slideshow.paused,
            &self.texture_manager,
            &mut self.thumbnail_manager,
        );

        // Check clipboard task completion
        if let Some(rx) = &self.clipboard_receiver {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    info!("Copied image to clipboard successfully.");
                    self.show_osd("Copied Image to Clipboard".to_string());
                    self.clipboard_receiver = None;
                }
                Ok(Err(e)) => {
                    error!("Failed to copy image to clipboard: {}", e);
                    self.show_osd(format!("Copy Failed: {}", e));
                    self.clipboard_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {} // Still running
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.show_osd("Copy Failed: Task died".to_string());
                    self.clipboard_receiver = None;
                }
            }
        }

        // Handle Overlay actions (Settings & OSC)
        if let Some(action) = overlay_action {
            use crate::overlay::OverlayAction;
            match action {
                OverlayAction::Osc(osc_action) => self.execute_osc_action(osc_action),
                OverlayAction::SetTimer(timer) => {
                    self.slideshow.set_duration(timer);
                    self.show_osd(format!("Timer: {:.1}s", timer));
                }
                OverlayAction::ToggleShuffle(enabled) => {
                    self.shuffle_enabled = enabled;
                    let new_index = self
                        .texture_manager
                        .set_shuffle_enabled(self.shuffle_enabled);
                    self.current_texture_index = Some(new_index);
                    self.transition = None;
                    self.renderer.invalidate_bind_group();
                    self.cached_info_string = None;
                }
                OverlayAction::SetPauseAtLast(_) => {
                    // Config already updated, just accessed by slideshow next frame
                }
                OverlayAction::ToggleScanSubfolders(_) => {
                    // Config already updated; takes effect on next drop/load
                }
                OverlayAction::SetTransitionTime(_) => {
                    // Config already updated
                }
                OverlayAction::ToggleRandomTransition(_) => {
                    // Config already updated
                }
                OverlayAction::SetTransitionMode(_) => {
                    // Config already updated; picked up by next transition
                }
                OverlayAction::SetFitMode(_) | OverlayAction::SetAmbientBlur(_) => {
                    // Config updated, will be picked up by render uniforms next frame
                }
                OverlayAction::ToggleAlwaysOnTop(always_on_top) => {
                    self.window.set_window_level(if always_on_top {
                        winit::window::WindowLevel::AlwaysOnTop
                    } else {
                        winit::window::WindowLevel::Normal
                    });
                }
                OverlayAction::ToggleFullscreen(fullscreen) => {
                    self.window.set_fullscreen(if fullscreen {
                        Some(winit::window::Fullscreen::Borderless(None))
                    } else {
                        None
                    });
                }
                OverlayAction::JumpTo(index) => {
                    self.jump_to(index);
                }
            }
        }

        // Auto-hide cursor
        if self.input_handler.cursor_visible
            && self.input_handler.last_cursor_move.elapsed().as_secs_f32() > 3.0
        {
            self.window.set_cursor_visible(false);
            self.input_handler.cursor_visible = false;
        }

        // Process drag & drop
        if let Some(pending_drop) = self.drag_drop.take_pending() {
            let rejected_suffix = if pending_drop.rejected_non_utf8 > 0 {
                format!(
                    " ({} non-UTF-8 path(s) rejected)",
                    pending_drop.rejected_non_utf8
                )
            } else {
                String::new()
            };

            if pending_drop.rejected_non_utf8 > 0 {
                warn!(
                    "Drag & drop rejected {} non-UTF-8 path(s)",
                    pending_drop.rejected_non_utf8
                );
                if pending_drop.paths.is_empty() {
                    self.show_osd(format!(
                        "Rejected {} non-UTF-8 path(s)",
                        pending_drop.rejected_non_utf8
                    ));
                }
            }

            if !pending_drop.paths.is_empty() {
                match image_loader::scan_image_paths(
                    &pending_drop.paths,
                    self.config.viewer.scan_subfolders,
                ) {
                    Ok(mut new_paths) => {
                        if self.shuffle_enabled {
                            use rand::seq::SliceRandom;
                            new_paths.shuffle(&mut rand::rng());
                        }
                        let count = new_paths.len();

                        if self.modifiers.shift_key() {
                            self.texture_manager.append_paths(new_paths);
                            // If it was already playing a slideshow/sequence, don't reset index to 0 or interrupt.
                            self.show_osd(format!("Appended {} images{}", count, rejected_suffix));
                            info!("Drag & drop: appended {} images", count);
                        } else {
                            self.texture_manager.replace_paths(new_paths);
                            self.transition = None;
                            self.renderer.invalidate_bind_group();
                            self.current_texture_index = if count > 0 { Some(0) } else { None };
                            self.slideshow.reset();
                            self.show_osd(format!("Loaded {} images{}", count, rejected_suffix));
                            info!("Drag & drop: loaded {} images", count);
                        }

                        self.update_window_title();
                        self.cached_info_string = None;
                    }
                    Err(e) => {
                        warn!("Drag & drop scan failed: {}", e);
                        self.update_window_title();
                        self.show_osd(format!("No supported images found{}", rejected_suffix));
                    }
                }
            }
        }

        self.texture_manager
            .update(&self.renderer.device, &self.renderer.queue);
        self.thumbnail_manager.update();

        // Check if transition finished (must run before auto-advance to avoid
        // a one-frame gap where the destination is shown without a transition)
        if let Some(ref transition) = self.transition {
            if transition.start_time.elapsed() >= transition.duration {
                self.current_texture_index = Some(transition.to_index);
                self.transition = None;
                self.renderer.invalidate_bind_group();
            }
        }

        if self.transition.is_none() && !self.texture_manager.paths.is_empty() {
            if self.config.viewer.playback_mode == config::PlaybackMode::Sequence {
                let frames_to_advance = self.sequence_timer.update();
                if frames_to_advance > 0 {
                    for _ in 0..frames_to_advance {
                        self.next_image();
                    }
                }
            } else if self.slideshow.update() {
                self.next_image();
            }
        }

        // Expire temporary timers
        let now = Instant::now();
        if self.filename_bar_temp_expiry.is_some_and(|t| now >= t) {
            self.filename_bar_temp_expiry = None;
        }
        if self.info_temp_expiry.is_some_and(|t| now >= t) {
            self.info_temp_expiry = None;
        }

        // Check for load errors
        if self.texture_manager.len() > 0 {
            if let Some(error_msg) = self
                .texture_manager
                .get_error(self.texture_manager.current_index)
            {
                self.egui_overlay
                    .set_center_error(&format!("Failed to load:\n{}", error_msg));
            } else {
                self.egui_overlay.clear_center_error();
            }
        } else {
            self.egui_overlay.clear_center_error();
        }

        // Update filename bar (bottom-left) — persistent O or temporary o
        let show_bar = self.show_filename_text || self.filename_bar_temp_expiry.is_some();

        if self.texture_manager.len() == 0 {
            self.egui_overlay.set_text("No images found in path");
        } else if show_bar {
            if let Some(path) = self.texture_manager.current_path() {
                let filename = path.file_name().unwrap_or("Unknown");
                let index = self.texture_manager.current_index + 1;
                let total = self.texture_manager.len();
                self.egui_overlay
                    .set_text(&format!("{} [{}/{}]", filename, index, total));
            } else {
                // Should be unreachable if len > 0
                self.egui_overlay.set_text("");
            }
        } else {
            self.egui_overlay.set_text("");
        }

        // OSD (top-right) — reactive feedback
        if let Some((ref text, expiry)) = self.osd_message {
            if now > expiry {
                self.osd_message = None;
                self.egui_overlay.set_osd_text("");
            } else {
                self.egui_overlay.set_osd_text(text);
            }
        } else {
            self.egui_overlay.set_osd_text("");
        }

        // Info overlay (top-left) — persistent I or temporary i
        if self.egui_overlay.info_overlay_visible() {
            if self.cached_info_string.is_none() {
                self.cached_info_string = Some(self.build_info_string());
            }
            let info = self.cached_info_string.as_deref().unwrap_or("");
            self.egui_overlay.set_info_text(info);
        } else if self.info_temp_expiry.is_some() {
            // Content was already set by key handler; just keep it
        } else {
            self.egui_overlay.set_info_text("");
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Apply any deferred resize before touching the surface.
        if let Some(new_size) = self.pending_resize.take() {
            self.resize(new_size);
        }

        let output = self.renderer.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let bg = &self.config.style.bg_color;
        let clear_color = wgpu::Color {
            r: bg[0] as f64 / 255.0,
            g: bg[1] as f64 / 255.0,
            b: bg[2] as f64 / 255.0,
            a: bg[3] as f64 / 255.0,
        };

        let mut encoder =
            self.renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        // Prepare BindGroup and Uniforms
        // Determine which textures to use
        let (tex_a_idx, tex_b_idx, blend, mode) = if let Some(ref t) = self.transition {
            let progress = t.start_time.elapsed().as_secs_f32() / t.duration.as_secs_f32();
            (t.from_index, t.to_index, progress.min(1.0), t.mode)
        } else if let Some(idx) = self.current_texture_index {
            (idx, idx, 0.0, 0)
        } else {
            // No images uploaded yet
            (0, 0, 0.0, 0)
        };

        // If textures are not loaded yet, we can't create bind group.
        // We'll skip rendering contents and just clear screen.
        let tex_a = self.texture_manager.get_texture(tex_a_idx);
        let tex_b = self.texture_manager.get_texture(tex_b_idx);

        if let (Some(tex_a), Some(tex_b)) = (tex_a, tex_b) {
            // Recreate bind group when textures change (transition start/end)
            if self.renderer.bind_group.is_none() {
                self.renderer.bind_group = Some(self.renderer.pipeline.create_bind_group(
                    &self.renderer.device,
                    &self.renderer.uniform_buffer,
                    &tex_a.view,
                    &tex_b.view,
                ));
            }

            // Update Uniforms
            let uniform = TransitionUniform {
                blend,
                mode,
                aspect_ratio: [1.0, 1.0],
                bg_color: self.config.bg_color_f32(),
                window_size: [self.size.width as f32, self.size.height as f32],
                image_a_size: [tex_a.width as f32, tex_a.height as f32],
                image_b_size: [tex_b.width as f32, tex_b.height as f32],
                brightness: self.color_brightness,
                contrast: self.color_contrast,
                gamma: self.color_gamma,
                saturation: self.color_saturation,
                fit_mode: self.config.viewer.fit_mode.to_uniform_value(),
                ambient_blur: self.config.viewer.ambient_blur,
                zoom_scale: self.zoom_scale,
                zoom_pan: self.zoom_pan,
                display_mode: if self.renderer.is_hdr { 1 } else { 0 },
            };

            self.renderer.queue.write_buffer(
                &self.renderer.uniform_buffer,
                0,
                bytemuck::cast_slice(&[uniform]),
            );

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                if let Some(ref bind_group) = self.renderer.bind_group {
                    render_pass.set_pipeline(&self.renderer.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..3, 0..1); // 3 vertices for fullscreen triangle
                }
            } // End of render pass
        } else {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass (Clear)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // Render egui overlay
        let egui_output = self.egui_overlay.end_frame(&self.window);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        // Prepare egui render data (must happen before creating render pass)
        let clipped_primitives = self.egui_overlay.prepare_render(
            &self.renderer.device,
            &self.renderer.queue,
            &mut encoder,
            &screen_descriptor,
            egui_output,
        );

        // Render egui into a dedicated pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.egui_overlay
                .render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        if self.screenshot_requested {
            self.screenshot_requested = false;
            match self.screenshot.capture(
                &self.renderer.device,
                &self.renderer.queue,
                encoder,
                &output.texture,
                &self.renderer.surface_config,
            ) {
                Ok(filename) => self.show_osd(format!("Screenshot: {}", filename)),
                Err(msg) => self.show_osd(msg),
            }
        } else {
            self.renderer
                .queue
                .submit(std::iter::once(encoder.finish()));
        }

        output.present();

        Ok(())
    }

    fn show_osd(&mut self, text: String) {
        self.osd_message = Some((text, Instant::now() + Duration::from_millis(1500)));
    }

    /// Returns `true` when something on screen is actively changing and a
    /// redraw must be requested every frame.  When this returns `false` the
    /// application is visually idle and can stop polling the GPU.
    fn is_animating(&self) -> bool {
        // Active image transition
        self.transition.is_some()
            // Slideshow auto-advance is running
            || !self.slideshow.paused
            // Sequence playback is running
            || (self.config.viewer.playback_mode == config::PlaybackMode::Sequence
                && !self.sequence_timer.paused)
            // OSD message or temporary overlay still visible
            || self.osd_message.is_some()
            || self.filename_bar_temp_expiry.is_some()
            || self.info_temp_expiry.is_some()
            // Egui overlays or OSC are open/visible
            || self.egui_overlay.is_active()
            // Texture loads in flight — update() must run to receive results
            || self.texture_manager.is_loading()
            // Cursor visible — update() must run to auto-hide it
            || self.input_handler.cursor_visible
    }
}

impl ApplicationHandler for ApplicationState {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Window is already created before event loop starts, so nothing to do here
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if window_id != self.window.id() {
            return;
        }

        // Update modifiers state
        if let WindowEvent::ModifiersChanged(new_state) = event {
            self.modifiers = new_state.state();
            return;
        }

        // Forward event to egui first
        let egui_consumed = self.egui_overlay.handle_event(&self.window, &event);

        // Try input handler only if egui didn't consume the event
        let modifiers = self.modifiers;
        if !egui_consumed {
            let (consumed, should_exit) = self.input(&event, &modifiers);
            if should_exit {
                event_loop.exit();
                return;
            }
            if !consumed {
                match event {
                    WindowEvent::CloseRequested => event_loop.exit(),
                    WindowEvent::Resized(physical_size) => {
                        // Defer surface reconfiguration to the next render() call.
                        // During a live window drag the OS fires many Resized events
                        // per frame; reconfiguring the surface on each one causes
                        // visible stuttering. Storing only the latest size here
                        // collapses all intermediate events into a single resize that
                        // is applied once, right before the next frame is drawn.
                        self.pending_resize = Some(physical_size);
                        self.window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        inner_size_writer: _,
                    } => {
                        info!("Scale factor changed to: {}", scale_factor);
                        // winit will automatically resize the window according to the new scale factor.
                        // We don't need to use inner_size_writer unless we want to override the OS default.
                        // The automatic resize will trigger a WindowEvent::Resized, which handles the actual resize.
                    }
                    WindowEvent::RedrawRequested => {
                        self.update();
                        match self.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                self.resize(self.size)
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                error!("GPU out of memory — exiting");
                                event_loop.exit();
                            }
                            Err(e) => error!("Render error: {:?}", e),
                        }
                    }
                    // Handle drag-and-drop on non-Windows platforms via winit's
                    // DroppedFile event. Windows uses WM_DROPFILES (see drag_drop.rs).
                    #[cfg(not(windows))]
                    WindowEvent::DroppedFile(path) => {
                        self.drag_drop.queue_dropped_file(path);
                    }
                    _ => {}
                }
            }
        }

        // Ensure the result of any window event is reflected on screen.
        self.window.request_redraw();
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.is_animating() {
            self.window.request_redraw();
        }
    }
}
