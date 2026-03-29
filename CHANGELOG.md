# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.4.0] — 2026-03-29

### Added
- CHANGELOG.md, SECURITY.md, CODE_OF_CONDUCT.md
- M PLUS 2 font license (OFL.txt)
- Dependabot configuration for Cargo and GitHub Actions
- Cargo.toml keywords and categories for crates.io discoverability

### Fixed
- Move drag-and-drop directory scan off main thread (#368)
- Prioritize current_index in texture preload queue (#366)
- Guard transition textures from LRU eviction during animation (#365)
- Reduce resize stutter by draining GPU queue and coalescing configures (#350)
- Prevent egui panel drag from moving main window (#349)

### Changed
- Upgrade wgpu v25→v27, egui v0.32→v0.33, windows v0.58→v0.62 (#361, #362)
- Upgrade rand, validator, pollster, toml to latest versions (#363)

### Removed
- GEMINI.md (outdated internal documentation)

## [0.3.0] — 2026-03-04

### Added
- HDR output support with Rgba16Float swapchain (#231)
- Interactive zoom and pan with Ctrl+scroll (#198)
- Image sequence playback mode for PNG/EXR (#112)
- Gallery view with virtual scrolling and thumbnails (#107)
- Settings panel with runtime controls (#103)
- Timeline scrub bar and playback controls (#92, #106)
- Keyboard shortcut help overlay (#90)
- AmbientFit shader for edge-filled display (#73)
- Drag & drop for files and folders; Shift+drop to append (#68, #230)
- Screenshot capture and clipboard copy (#85, #229)
- EXIF rotation support (#65)
- DPI scaling / HiDPI support (#91)
- File reveal in OS file explorer (#83, #284)
- EXR FPS metadata parsing for sequence mode (#186)
- Color adjustment reset shortcut (#195)
- Scan Subfolders option and Transition Mode slider in Settings (#227)
- Transition mode name display in Settings (#295)

### Fixed
- Auto-scale SDR images on HDR swapchain (#291)
- Correct inverted sliding_door transition (#234)
- Prevent HDR screenshot crash (#249)
- Cap sequence timer catch-up frames after stalls (#248)
- Reject invalid UTF-16 drag-drop paths (#267)
- Guard against empty surface caps and zero-dimension resize (#265)
- Avoid unconditional redraws on every window event (#271)
- Defer surface reconfigure to eliminate resize stutter (#216)
- Skip transition when duration is zero (#192)
- Discard stale texture uploads via epoch counter (#170)
- Correct EXR tone mapping with proper sRGB transfer function (#166)

### Performance
- Rayon thread pool for image loading (#191)
- Skip redraws when application is visually idle (#196)
- Accelerate image resizing with fast_image_resize (#162)
- Cache info overlay string to avoid per-frame fs::metadata() calls (#171)

## [0.2.0] — 2026-02-13

### Added
- Initial public release
- Slideshow viewer with WGSL transition effects (winit + wgpu)
- TOML-based configuration (`.sldshow` files)
- Multiple transition modes (crossfade, roll, blind, box, wipe, and more)
- OSD text overlay with customizable font
- Window management (fullscreen, always-on-top, frameless, transparency)
- Screen saver prevention (Windows)

[Unreleased]: https://github.com/ugai/sldshow2/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ugai/sldshow2/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/ugai/sldshow2/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ugai/sldshow2/releases/tag/v0.2.0
