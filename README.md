# sldshow2

High-performance slideshow image viewer with custom WGSL transitions, built with Rust, winit, and wgpu.

## Features

- **22 different transition effects** with custom WGSL shaders
- **Embedded assets** (shaders) for standalone distribution
- **Async image loading** for non-blocking startup and navigation
- **Frameless window support** for clean presentation
- **TOML configuration** with flexible settings
- **Smart image preloading/caching** (configurable extent)
- **Auto-advance timer** with pause/resume
- **Keyboard/mouse controls** with hold-to-repeat navigation
- **Text rendering** with glyphon for file path display
- **Hot-reload configuration** via file watching

## Quick Start

### 1. Generate Test Images

```bash
cargo run --example generate_test_images
```

This creates 7 CC0 test images in `assets/test_images/`.

### 2. Run with Test Configuration

```bash
# Development build
cargo run -- test.sldshow

# Release build (RECOMMENDED for performance testing)
cargo run --release -- test.sldshow
```

**Note**: Use `--release` for accurate performance evaluation. Debug builds may exhibit frame stuttering due to unoptimized image decoding and GPU operations.

### 3. Test Controls

**Keyboard:**
- `→` / `Space` - Next image (hold to fast-forward)
- `←` - Previous image (hold to rewind)
- `Home` - First image
- `End` - Last image
- `P` - Toggle pause/resume
- `F` - Toggle fullscreen
- `ESC` / `Q` - Quit

**Mouse:**
- Left click - Next image
- Right click - Previous image
- Scroll wheel - Navigate images

## Building

```bash
# Development build (compile-time check)
cargo build

# Release build (optimized, recommended for distribution)
cargo build --release
```

The executable is standalone and includes all required assets (shaders) embedded at compile time. You can run the binary from any location.

## Configuration

See `example.sldshow` for all configuration options.

Default config location: `~/.sldshow`

### Key Settings

**Window:**

- `width`, `height` - Window dimensions
- `fullscreen` - Fullscreen mode
- `decorations` - Show/hide titlebar

**Viewer:**

- `image_paths` - Directories or files to display
- `timer` - Seconds per image (0 = paused)
- `shuffle` - Random order
- `cache_extent` - Number of images to preload

**Transition:**

- `time` - Transition duration in seconds
- `random` - Use random effects
- `mode` - Specific effect (0-21) if not random

**Style:**

- `bg_color` - Background color [R, G, B, A]
- `show_image_path` - Display current file path

## Transition Effects

22 different effects (mode 0-21):

- 0-1: Crossfade variations
- 2-9: Roll (from various directions)
- 10-11: Sliding door (open/close)
- 12-15: Blind effects
- 16-17: Box (expand/contract)
- 18-21: Advanced effects (random squares, angular wipe, etc.)

## Project Structure

```txt
sldshow2/
├── src/
│   ├── main.rs              # Entry point, event loop, state management
│   ├── config.rs            # TOML configuration parsing
│   ├── image_loader.rs      # Async image loading & texture cache
│   ├── transition.rs        # wgpu render pipeline & shader uniforms
│   ├── slideshow.rs         # Auto-advance timer logic
│   ├── text.rs              # Text rendering with glyphon
│   ├── diagnostics.rs       # Performance diagnostics
│   ├── metadata.rs          # Image metadata extraction
│   ├── watcher.rs           # File watching for hot-reload
│   ├── consts.rs            # Application constants
│   └── error.rs             # Custom error types
├── assets/
│   ├── shaders/
│   │   └── transition.wgsl  # 22 transition effects (embedded at compile time)
│   └── test_images/         # Generated test images
├── docs/
│   ├── AI_DEVELOPMENT_GUIDE.md  # AI collaboration guidelines
│   └── QUICK_START.md           # Quick start guide
├── examples/
│   └── generate_test_images.rs # Test image generator
├── test.sldshow             # Test configuration
└── example.sldshow          # Example configuration
```

## Development

### Code Statistics

- ~1,200 lines of Rust
- 11 core modules
- 22 WGSL transition effects

### Architecture

**Direct wgpu Control Architecture:**
- **Event-driven**: Uses `winit` event loop with `RedrawRequested` events
- **State Management**: `ApplicationState` struct holds all app state
  - `wgpu::Device`, `wgpu::Queue` for GPU operations
  - `TextureManager` for async image loading and caching
  - `TransitionPipeline` for render pipeline and bind groups
- **Async Loading**: `rayon` thread pool for non-blocking image decoding
- **Compile-time asset embedding** for standalone distribution
- **Hot-reload**: `notify` crate watches config file for changes

### Key Components

**ApplicationState** (main.rs):
- Central state management
- Event handling and input processing
- Update and render loop coordination

**TransitionPipeline** (transition.rs):
- wgpu render pipeline setup
- Bind group management
- Shader uniform updates

**TextureManager** (image_loader.rs):
- Background thread image decoding
- GPU texture upload throttling
- Rolling texture cache with configurable extent
- Automatic image resizing to fit window

**TextRenderer** (text.rs):
- glyphon-based text rendering
- File path display with custom styling

## Troubleshooting

**No images displayed:**

- Check that `image_paths` in config points to valid directories
- Ensure images are in supported formats (PNG, JPG, GIF, WebP, BMP, TGA, TIFF, ICO, HDR)
- Check console output for error messages

**Transitions not working:**

- Shader compilation errors will be logged to console
- Shaders are embedded in the executable; rebuild if issues persist

**Text not displaying:**

- Check `show_image_path` setting in config
- Verify glyphon initialization in logs

**Performance issues:**

- Reduce `cache_extent` if using many large images
- Lower `transition.time` for faster transitions
- Use `fullscreen = false` and smaller window size
- **Use release builds** (`cargo run --release`) for accurate performance

## License

MIT

## Credits

Based on the original [sldshow](https://github.com/ugai/sldshow) by ugai.

Transition effects adapted from [GL Transitions](https://gl-transitions.com/) (MIT License).

Test images are programmatically generated (CC0/Public Domain).
