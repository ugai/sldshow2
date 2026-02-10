# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sldshow2 is a high-performance slideshow image viewer built with **Rust**, **winit**, and **wgpu**. It features 22 custom WGSL transition effects and standalone distribution with embedded assets.

## Development Commands

### Building and Running
```bash
# Development build (compile-time check)
cargo build

# Run with release optimizations (RECOMMENDED for visual testing)
# Debug builds may be slower due to unoptimized wgpu/image crate code
cargo run --release -- test.sldshow

# Run example
cargo run --release --example generate_test_images
```

### Code Quality
```bash
# Format code
cargo fmt

# Lint with Clippy
cargo clippy

# Run documentation
cargo doc --open
```

### Testing
There are no unit tests in this codebase - testing is done manually by running the application.

## Architecture Overview

### Core Loop
The application uses a standard `winit` event loop in `src/main.rs`.
- **`ApplicationState`**: Holds all app state (window, wgpu device, texture manager, etc.).
- **`update()`**: Called on `RedrawRequested`. Handles non-rendering logic (slideshow timer, image loading).
- **`render()`**: Called on `RedrawRequested`. Encodes GPU commands.

### Key Components
- **TransitionPipeline** (`src/transition.rs`): Manages the wgpu render pipeline, bind groups, and shader uniforms for the 22 WGSL effects.
- **TextureManager** (`src/image_loader.rs`): Handles async image loading (using `rayon` `image` crate) and GPU texture management. Maintains a rolling cache of textures.
- **TextRenderer** (`src/text.rs`): High-quality text rendering using `glyphon` / `cosmic-text`.
- **SlideshowTimer** (`src/slideshow.rs`): Simple `std::time::Instant`-based timer for auto-advancement.
- **Config** (`src/config.rs`): TOML-based configuration system.

### State Management
All state is encapsulated in the `ApplicationState` struct in `main.rs`.
- `texture_manager`: Holds loaded textures and loading status.
- `pipeline`: Holds render pipeline state.
- `slideshow`: Holds timer state.
- `transition`: Option<ActiveTransition> tracks current transition progress.

### Asset Embedding
Assets are embedded at compile time for standalone distribution:
- **Shaders**: `include_str!("../assets/shaders/transition.wgsl")`
- **Fonts**: (Currently disabled/placeholder)

## Development Guidelines

### Build Strategy
- **Always use `--release` for visual testing**. Pure debug builds of `image` (PNG/JPG decoding) and `wgpu` can be slow, causing frame stutters that don't reflect production performance.

### Commits and Pull Requests
- Commit messages and PR titles follow [Conventional Commits](https://www.conventionalcommits.org/) format: `feat:`, `fix:`, `docs:`, `refactor:`, `perf:`, `style:`, `chore:`, etc.

### Code Style
- **English** comments and documentation.
- **Structured Logging**: Use `log` crate (`info!`, `warn!`, `error!`).
- **Explicit State**: Avoid global mutable state; pass `ApplicationState` or its fields explicitly.

### WGPU Specifics
- **Texture Upload**: Done via `queue.write_texture`. Large images are resized on CPU before upload to avoid VRAM exhaustion.
- **Bind Groups**: Recreated only when textures change (e.g., transition start/end).
- **Surface Configuration**: Handled in `resize()`.

### Performance Considerations
- **Async Loading**: Image decoding happens on background threads (`rayon`).
- **Throttling**: Main thread receives loaded images via channel to upload to GPU.
- **Texture Cache**: `cache_extent` configuration controls how many images are kept in VRAM.

## File Organization

### Core Modules
- `main.rs` - Application entry, event loop, input handling.
- `transition.rs` - WGPU pipeline and effect logic.
- `image_loader.rs` - Texture manager and threaded loading.
- `slideshow.rs` - Timer logic.
- `config.rs` - Configuration.
- `error.rs` - Custom error types.

### Assets
- `assets/shaders/transition.wgsl` - 22 transition effects (embedded).