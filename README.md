# sldshow2

Simple slideshow image viewer with custom WGSL transitions, built with Bevy.

## Features

- **22 different transition effects** with custom WGSL shaders
- **Embedded assets** (shaders and fonts) for standalone distribution
- **Async image scanning** for non-blocking startup
- **Frameless window support** for clean presentation
- **TOML configuration** with flexible settings
- **Smart image preloading/caching** (configurable extent)
- **Auto-advance timer** with pause/resume
- **Keyboard/mouse controls** with hold-to-repeat navigation
- **Optional file path display** with embedded Japanese font support

## Quick Start

### 1. Generate Test Images

```bash
cargo run --example generate_test_images
```

This creates 7 CC0 test images in `assets/test_images/`.

### 2. Run with Test Configuration

```bash
cargo run -- test.sldshow
```

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
# Development build
cargo build

# Release build (optimized)
cargo build --release
```

The executable is standalone and includes all required assets (shaders and fonts) embedded at compile time. You can run `target/release/sldshow2.exe` from any location without the `assets` folder.

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
- `show_image_path` - Display current file path (with embedded M PLUS 2 font)

## Transition Effects

22 different effects (mode 0-21):

- 0-1: Crossfade variations
- 2-9: Roll (from various directions)
- 10-11: Sliding door (open/close)
- 12-15: Blind effects
- 16-17: Box (expand/contract)
- 18-19: Advanced effects (random squares, angular wipe)

## Project Structure

```txt
sldshow2/
├── src/
│   ├── main.rs              # Entry point & integration
│   ├── config.rs            # TOML configuration
│   ├── image_loader.rs      # Image loading & caching
│   ├── transition.rs        # Transition material system (embeds shader)
│   └── slideshow.rs         # Auto-advance timer
├── assets/
│   ├── shaders/
│   │   └── transition.wgsl  # 22 transition effects (embedded at compile time)
│   ├── fonts/
│   │   └── MPLUS2-VariableFont_wght.ttf  # Japanese font (embedded)
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

- ~1,100 lines of Rust
- 4 core modules
- 22 WGSL transition effects
- Bevy 0.15 integration

### Architecture

- **ECS-based** using Bevy's Entity Component System
- **Event-driven** transitions via TransitionEvent
- **Resource-based** state management
- **Plugin system** for modular organization
- **Async task pool** for non-blocking image scanning
- **Compile-time asset embedding** for standalone distribution

## Troubleshooting

**No images displayed:**

- Check that `image_paths` in config points to valid directories
- Ensure images are in supported formats (PNG, JPG, GIF, WebP, BMP, TGA, TIFF, ICO, HDR)
- Check console output for error messages

**Transitions not working:**

- Shader compilation errors will be logged to console
- Shaders are embedded in the executable; rebuild if issues persist

**Text not displaying:**

- Font is embedded at compile time; rebuild if issues occur
- Check `show_image_path` setting in config

**Performance issues:**

- Reduce `cache_extent` if using many large images
- Lower `transition.time` for faster transitions
- Use `fullscreen = false` and smaller window size

**Windows path syntax error on startup (os error 123):**

- This is a harmless Bevy 0.15 initialization warning on Windows
- The error appears once during startup and can be safely ignored
- It does not affect image loading or application functionality
- All images will load correctly despite this warning

## License

MIT

## Credits

Based on the original [sldshow](https://github.com/ugai/sldshow) by ugai.

Transition effects adapted from [GL Transitions](https://gl-transitions.com/) (MIT License).

Test images are programmatically generated (CC0/Public Domain).
