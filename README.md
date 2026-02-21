# sldshow2

High-performance slideshow image viewer with custom [WGSL](https://www.w3.org/TR/WGSL/) transitions, built with Rust, [winit](https://github.com/rust-windowing/winit), [wgpu](https://github.com/gfx-rs/wgpu), and [egui](https://github.com/emilk/egui).

## Features

- **Multiple transition effects** with custom WGSL shaders
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
- `mode` - Specific effect (0-19) if not random

**Style:**

- `bg_color` - Background color [R, G, B, A]
- `show_image_path` - Display current file path

## Transition Effects

Available effects:

- 0-1: Crossfade variations
- 2-9: Roll (from various directions)
- 10-11: Sliding door (open/close)
- 12-15: Blind effects
- 16-17: Box (expand/contract)
- 18-19: Advanced effects (random squares, angular wipe)

## Development

- **Contributing**: See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, workflow, and coding standards.
- **Architecture**: See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed architecture documentation.
- **AI Agent Automation**: Issues labeled `agent:ready` are picked up and slain by AI agents. See [AGENTS.md](AGENTS.md) for details.

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
