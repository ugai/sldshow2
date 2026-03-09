# sldshow2 Architecture

This document describes the high-level architecture and key components of `sldshow2`.

## Core Technologies

-   **Rust**: Primary language.
-   **winit**: Window creation and event loop management.
-   **wgpu**: Graphics API for rendering (WebGPU implementation).
-   **wgsl**: Shader language for transitions.
-   **egui**: Immediate mode GUI for overlays, settings panel, gallery, and on-screen controller.

## Module Responsibility

| File | Responsibility |
| :--- | :--- |
| `main.rs` | **Entry Point**. Initializes logging, configuration, and the event loop. Creates the window and hands off control to `ApplicationState`. Installs screen saver prevention (Windows). |
| `app.rs` | **Application Logic**. Contains `ApplicationState` struct. Handles the `winit` event loop (`ApplicationHandler`), input processing, update loop, rendering coordination, zoom/pan state, and color adjustments. |
| `renderer.rs` | **GPU Renderer**. Owns the wgpu surface, device, queue, transition pipeline, uniform buffer, and bind group. Detects HDR display support and selects `Rgba16Float` or sRGB swapchain format. Manages transparent window composite alpha. |
| `input.rs` | **Input Handling**. Translates raw winit events into abstract `InputAction`s. Handles keyboard shortcuts, mouse clicks, scroll wheel zoom, drag-to-pan, double-click fullscreen, and cursor auto-hide. |
| `transition.rs` | **Rendering Pipeline**. Manages the WGPU render pipeline for image transitions. `TransitionUniform` (112 bytes) carries blend, mode, image sizes, color adjustments, fit mode, ambient blur, zoom/pan, display mode, and per-texture SDR brightness scale factors to the shader. |
| `image_loader.rs` | **Asset Management**. Asynchronous image loading using `rayon` (up to 4 concurrent tasks). `MipData` enum supports both SDR (`Rgba8`) and HDR (`Rgba16Float` via `half` crate) texture paths. Full mip chain generation. Supports EXR, PNG, JPEG, GIF, WebP, BMP, TGA, TIFF, ICO, HDR, AVIF, PNM, DDS, QOI. |
| `thumbnail.rs` | **Thumbnails**. Generates and caches 256px thumbnails on background threads for the gallery view. Tracks stale textures via `drain_newly_cached()`. |
| `overlay.rs` | **UI Layer**. Manages the `egui` context. Draws the filename bar, OSD, info overlay, help overlay (keyboard shortcut reference), settings panel (playback/transition/display/window controls), and gallery view (thumbnail grid with virtual scrolling). |
| `config.rs` | **Configuration**. `serde::Deserialize` structs for parsing `.sldshow` TOML files with defaults and validation. Config is loaded once at startup; there is no hot-reload or file watching. The `save()` method exists but is not called — runtime changes are in-memory only and are not persisted to disk. |
| `timer.rs` | **Slideshow Timer**. State machine for auto-advancing slides with pause/resume. |
| `clipboard.rs` | **System Integration**. Async image-to-clipboard copy using `arboard`. Re-reads image from disk to avoid GPU readback. |
| `osc.rs` | **On-Screen Controller**. Floating auto-hide playback bar with scrub timeline, play/pause, next/prev, shuffle, and settings buttons. Uses Phosphor icon font. |
| `drag_drop.rs` | **OS Integration**. Cross-platform drag-and-drop: Windows uses `WM_DROPFILES` (Win32 API); other platforms use winit's `DroppedFile` event. Shift+drop appends to existing playlist. |
| `error.rs` | **Error Handling**. Custom error types. |
| `screenshot.rs` | **Screen Capture**. Captures the current rendered frame from the GPU staging buffer, handles BGRA channel swap, and saves as PNG to the Pictures directory. |

## Key Flows

### 1. Initialization (`main.rs` -> `app.rs`)
1.  `main()` parses command line args and loads `Config`.
2.  Installs screen saver prevention (Windows: `SetThreadExecutionState`).
3.  Initializes `winit` EventLoop with drag-and-drop hook.
4.  Creates the `winit::Window` (transparent if `bg_color` alpha < 255).
5.  Calls `ApplicationState::new()`:
    -   Creates `Renderer` (Surface, Adapter, Device, Queue, TransitionPipeline, uniform buffer). Selects `Rgba16Float` swapchain if HDR is supported.
    -   Initializes `TextureManager` (scans initial paths, sets up async loading channels).
    -   Sets up `EguiOverlay` with settings panel, gallery, and OSC.
6.  Starts the event loop with `run_app(&mut state)`.

### 2. Event Loop (`app.rs`)
The `ApplicationHandler` trait implementation in `app.rs` drives the application:
-   `window_event()`: Dispatches events to `egui`, then `InputHandler`. Processes `InputAction`s including zoom/pan, color adjustments, gallery toggle, and clipboard operations.
-   `about_to_wait()`: Requests redraw only when animating (transition in progress), overlay is active, or slideshow timer fires — otherwise idle.
-   `update()`: Calls `build_ui(&mut self.config, …)` each frame so the settings panel can mutate the in-memory `Config` directly. Changes take effect immediately (next redraw or action) but are **not written back to the `.sldshow` file**.

### 3. Rendering Frame (`app.rs` -> `transition.rs` -> `overlay.rs`)
On `WindowEvent::RedrawRequested`:
1.  `update()`: Updates timers, animations, and UI state.
2.  `render()`:
    -   Acquires next swapchain image.
    -   Builds `TransitionUniform` with current blend, color adjustments, zoom/pan, fit mode, and display mode.
    -   **Pass 1 (Transitions)**: Renders image transition using `TransitionPipeline`. Blends two mip-mapped textures based on `transition_progress`. In AmbientFit mode, samples low mip levels for blurred background.
    -   **Pass 2 (UI)**: Renders `egui` geometry (overlays, settings, gallery, OSC) over the scene.
    -   Optionally captures frame for screenshot via GPU staging buffer.
    -   Submits command buffer to queue and presents the frame.

### 4. Image Loading (`image_loader.rs`)
-   Images are loaded on a background thread pool (`rayon`, up to 4 concurrent tasks).
-   **SDR path**: Decoded to `Rgba8`, resized with `fast_image_resize` (Lanczos3), mip chain generated. Uploaded as `Rgba8UnormSrgb`.
-   **HDR path** (EXR on HDR displays): Decoded to `Rgba32F`, resized with `image` crate (bilinear), converted to `f16` via `half` crate. Uploaded as `Rgba16Float`.
-   EXIF orientation is applied during decoding.
-   Results are epoch-stamped to discard stale loads after playlist changes.
-   `TextureManager` handles rolling preload/cache of `cache_extent` images around the current index.

### 5. HDR Pipeline
When the wgpu surface supports `Rgba16Float`:
1.  `Renderer` sets `is_hdr = true` and configures the swapchain with `Rgba16Float`.
2.  `TextureManager` uses the HDR decode path for `.exr` files, producing `MipData::Hdr` with linear float pixels. Each `LoadedTexture` tracks `is_hdr_content` so the shader can apply per-texture brightness scaling.
3.  The `TransitionUniform.display_mode` is set to `1` (HDR), telling the shader to pass linear values through without clamping.
4.  SDR images on an HDR swapchain are scaled by `sdr_scale_a/b` (203/80 ≈ 2.54, BT.2408 reference white) in the shader to restore correct brightness. HDR content and SDR swapchains use a scale of 1.0.
