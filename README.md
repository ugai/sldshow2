# sldshow2

High-performance slideshow image viewer with custom [WGSL](https://www.w3.org/TR/WGSL/) transitions, built with Rust, [winit](https://github.com/rust-windowing/winit), [wgpu](https://github.com/gfx-rs/wgpu), and [egui](https://github.com/emilk/egui).

## Features

- **Rich transition effects** — crossfade, roll, blind, box, angular wipe, random squares, and more
- **HDR / EXR support** *(beta)* — native Rgba16Float pipeline on HDR displays; EXR sequence playback with auto-detected FPS
- **Interactive zoom & pan** — Ctrl+scroll to zoom, drag to pan
- **Color adjustments** — contrast, brightness, gamma, saturation
- **Gallery view** — thumbnail grid with scrub bar and on-screen controller
- **Settings panel** — runtime controls for playback, transitions, display, and window
- **AmbientFit** — blurred background fill instead of black bars
- **Screenshot & clipboard** — capture frame to PNG, copy image or path
- **Drag & drop** — drop files/folders to load; Shift+drop to append
- **Transparent & frameless windows**, screen saver prevention (Windows)

## Quick Start

```bash
cargo run --example generate_test_images
cargo run --release -- example.sldshow
```

Press `?` in-app for the full keyboard shortcut reference.

## Configuration

TOML files with `.sldshow` extension. Lookup: CLI arg → `~/.sldshow` → defaults.

Configuration is loaded **once at startup**. There is no hot-reload or file watching — to apply changes to the `.sldshow` file, restart the application. Runtime adjustments made through the Settings panel take effect immediately but are not saved back to disk.

See [`example.sldshow`](example.sldshow) for all options — window, viewer (playback mode, fit mode, texture limits, scan subfolders, …), transition, and style (background, font, transparency).

## Supported Formats

PNG, JPEG, GIF, BMP, TIFF, WebP, ICO, TGA, HDR, PNM, DDS, QOI, AVIF, OpenEXR

## Development

- [CONTRIBUTING.md](CONTRIBUTING.md) — setup, workflow, coding standards
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — module map and key flows
- [AGENTS.md](AGENTS.md) — AI agent automation (`agent:ready` issues)

## License

MIT — Based on the original [sldshow](https://github.com/ugai/sldshow) by ugai. Transitions adapted from [GL Transitions](https://gl-transitions.com/) (MIT).
