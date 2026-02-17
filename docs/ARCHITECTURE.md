# sldshow2 Architecture

This document describes the high-level architecture and key components of `sldshow2`.

## Core Technologies

-   **Rust**: Primary language.
-   **winit**: Window creation and event loop management.
-   **wgpu**: Graphics API for rendering (WebGPU implementation).
-   **wgsl**: Shader language for transitions.
-   **egui**: Immediate mode GUI for overlays and debug info.

## Module Responsibility

| File | Responsibility |
| :--- | :--- |
| `main.rs` | **Entry Point**. Initializes logging, configuration, and the event loop. Creates the window and hands off control to `ApplicationState`. |
| `app.rs` | **Application Logic**. Contains `ApplicationState` struct. Handles the `winit` event loop (`ApplicationHandler`), input processing, update loop, and rendering coordination. |
| `input.rs` | **Input Handling**. Processes raw winit events into abstract `InputAction`s. Handles key remapping, mouse interaction, and cursor visibility. |
| `transition.rs` | **Rendering Pipeline**. Manages the WGPU render pipeline for image transitions. Loads and executes compiled WGSL shaders. |
| `image_loader.rs` | **Asset Management**. Asynchronous image loading using `rayon`. Manages texture uploads to the GPU. |
| `thumbnail.rs` | **Thumbnails**. Generates and caches thumbnails for the file list or future gallery view. |
| `overlay.rs` | **UI Layer**. Manages the `egui` context. Draws the filename bar, OSD (On-Screen Display), debug info, and settings panel. |
| `config.rs` | **Configuration**. derived `serde::Deserialize` structs for parsing `config.toml`. |
| `timer.rs` | **Slideshow Timer**. Simple state machine for auto-advancing slides. |
| `clipboard.rs` | **System Integration**. Interact with the system clipboard to copy file paths or image data. |
| `osc.rs` | **On-Screen Controller**. Logic for the interactive bottom-bar controller (play/pause, next/prev). |
| `drag_drop.rs` | **OS Integration**. Handles file drag-and-drop events (Windows specific implementations for `WM_DROPFILES`). |
| `error.rs` | **Error Handling**. Custom error types. |
| `screenshot.rs` | **Utility**. Captures the current frame and saves it to disk. |

## Key Flows

### 1. Initialization (`main.rs` -> `app.rs`)
1.  `main()` parses command line args and loads `Config`.
2.  Initializes `winit` EventLoop.
3.  Creates the `winit::Window`.
4.  Calls `ApplicationState::new()`:
    -   Initializes WGPU (Surface, Adapter, Device, Queue).
    -   Sets up the `TransitionPipeline` (loads shaders).
    -   Initializes `TextureManager` (scans initial paths).
    -   Sets up `EguiOverlay`.
5.  Starts the event loop with `run_app(&mut state)`.

### 2. Event Loop (`app.rs`)
The `ApplicationHandler` trait implementation in `app.rs` drives the application:
-   `window_event()`: Dispatches events to `egui`, then `input_handler`.
-   `about_to_wait()`: Requests a redraw (continuous rendering or on-demand).

### 3. Rendering Frame (`app.rs` -> `transition.rs` -> `overlay.rs`)
On `WindowEvent::RedrawRequested`:
1.  `update()`: Updates timers, animations, and UI state.
2.  `render()`:
    -   Acquires next swapchain image.
    -   **Pass 1 (Transitions)**: Renders the background/image transition using `TransitionPipeline`. Blends two textures based on `transition_progress`.
    -   **Pass 2 (UI)**: Renders `egui` geometry over the scene.
    -   Submits command buffer to queue.
    -   Presents the frame.

### 4. Image Loading (`image_loader.rs`)
-   Images are loaded on a background thread pool (`rayon`).
-   Decoded images are sent back to the main thread via channels.
-   Main thread uploads texture data to GPU during `update()`.
-   `TextureManager` handles preloading (caching next/prev images) to avoid stalls.
