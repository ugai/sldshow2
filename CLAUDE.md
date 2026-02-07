# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sldshow2 is a Bevy-based slideshow image viewer with 22 custom WGSL transition effects. Built with Rust using Bevy 0.18 and featuring standalone distribution with embedded assets.

## Development Commands

### Building and Running
```bash
# Development build (compile-time check only, NOT for visual testing)
cargo build

# Dev-release build (use for visual/performance testing during development)
# Release-level optimization with debug symbols, faster compile than full release
cargo build --profile dev-release
.\target\dev-release\sldshow2.exe .\example.sldshow

# Release build (for distribution)
cargo build --release
.\target\release\sldshow2.exe .\example.sldshow

# Generate test images
cargo run --example generate_test_images

# Run with debug logging
RUST_LOG=sldshow2=debug cargo run -- test.sldshow
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
There are no unit tests in this codebase - testing is done manually by running the application with various configurations.

## Architecture Overview

### Core Systems (Execution Order)
The main application uses Bevy's ECS with explicitly chained systems:
1. `keyboard_input_system` - Handles keyboard/mouse input
2. `handle_slideshow_advance` - Auto-advance timer logic
3. `detect_image_change` - Detects image changes and emits TransitionEvent
4. `trigger_transition` - Creates/updates TransitionEntity
5. `update_transitions` - Updates shader blend values
6. `update_transition_on_resize` - Handles window resize

### Key Components
- **TransitionPlugin**: Material2d-based shader system with 22 WGSL effects
- **ImageLoader**: Async image scanning with smart preloading/caching
- **SlideshowTimer**: Auto-advance functionality with pause/resume
- **Config**: TOML-based configuration system
- **Diagnostics**: Performance monitoring (frame time, transition metrics)

### State Management
Uses Bevy Resources for global state:
- `TransitionState` - Tracks current/displayed images with explicit double-buffering
- `ImageLoader` - Contains image cache and handles
- `Config` - Application configuration
- `SlideshowTimer` - Timer state

### Asset Embedding
All assets (shaders, fonts) are embedded at compile time for standalone distribution:
- Shaders: `include_str!("../assets/shaders/transition.wgsl")`
- Fonts: M PLUS 2 Japanese font for path display

## Development Guidelines

### Build Strategy
- Use `cargo build` for compile-time checks only (fastest iteration)
- Use `cargo build --profile dev-release` for visual/performance testing
- Use `cargo build --release` for distribution
- IMPORTANT: Debug builds have significant Bevy ECS + wgpu overhead (200-400ms frame spikes)
  that do NOT exist in release builds. Always use dev-release for visual testing.

### Code Style
- Comments and documentation in English
- Use structured logging with tracing (info!, debug!, warn!, error!)
- System execution order must be explicit with `.chain()`
- Avoid unnecessary Handle<Image> cloning

### Development Practice
- Use `cargo build` for quick compile checks during development
- Use `cargo build --profile dev-release` for visual/performance testing (release optimization + debug symbols)
- Debug builds have wgpu validation disabled (InstanceFlags::empty()) but still have ECS overhead
- Dependencies are optimized (opt-level = 3) but Bevy's ECS scheduler overhead remains in debug

### Bevy 0.18 Specifics
- UI Text requires parent-child structure (parent: Node, child: Text)
- `commands.spawn()` is deferred until stage completion
- GPU texture uploads take several frames
- Material2d requires specific feature flags in Cargo.toml

### Performance Considerations
- Image cache uses HashMap for O(1) lookups (not Vec for O(n))
- Async image scanning prevents main thread blocking
- Power mode switches between Continuous (transitions) and Reactive (idle)
- Transition debouncing prevents rapid-fire transitions

## File Organization

### Core Modules
- `main.rs` - Application entry, system setup, and UI
- `transition.rs` - WGSL shader material system with 22 effects
- `image_loader.rs` - Async image loading with smart caching
- `slideshow.rs` - Auto-advance timer functionality
- `config.rs` - TOML configuration handling
- `diagnostics.rs` - Performance monitoring
- `metadata.rs` - EXIF data extraction for rotation
- `watcher.rs` - File watching (currently disabled on Windows)

### Assets
- `assets/shaders/transition.wgsl` - 22 transition effects (embedded)
- `assets/fonts/MPLUS2-VariableFont_wght.ttf` - Japanese font (embedded)
- `assets/test_images/` - Generated test images

## Common Debugging

### Performance Issues
- Use `RUST_LOG=sldshow2=debug` for detailed logging
- Monitor with diagnostics system (frame time, transition metrics)
- Check cache_extent setting for memory usage

### Transition Problems
- Shader compilation errors logged to console
- Shaders are embedded - rebuild if issues persist
- Transition state tracking uses explicit displayed_image field

### Windows-Specific Notes
- File watching is disabled due to path handling issues
- Harmless "os error 123" warning appears once during Bevy 0.18 startup
- Use absolute paths for reliability