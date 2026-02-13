# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sldshow2 is a high-performance slideshow image viewer built with **Rust**, **winit**, and **wgpu**. It features 20 custom WGSL transition effects and standalone distribution with embedded assets.

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
- **TransitionPipeline** (`src/transition.rs`): Manages the wgpu render pipeline, bind groups, and shader uniforms for the 20 WGSL effects.
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

### Naming Conventions (Issues, PRs, Commits)
All titles follow [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>: <short description in lowercase>
```

- **Types**: `feat:`, `fix:`, `docs:`, `refactor:`, `perf:`, `style:`, `chore:`, etc.
- **Issue titles**: `feat: implement ambient fit shader`, `fix: transition flash on first frame`
- **PR titles**: Same format. When a PR addresses a single issue, the title can match the issue title.
- **Commit messages**: Same format. Reference issues in the body (e.g., `Closes #38`).
- **Branch names**: `feat/ambient-fit-shader`, `fix/transition-flash` (type/kebab-case-description)

### Commits and Pull Requests
- **No direct push to `main`.** All changes go through pull requests.
- **Squash merge only.** PRs are squash-merged to keep `main` history clean.
- **CI must pass** before merge. PR checks: `cargo check` (Linux + Windows), `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Release build (Windows) runs on push to `main` only.
- **Always run `cargo fmt --all` before committing.** CI enforces `cargo fmt --all -- --check` and will fail on formatting differences.
- PRs should reference related issues in the body using `Closes #N` syntax for auto-close on merge.

### Pre-Commit Hook
A pre-commit hook runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` automatically. Setup:
```bash
git config core.hooksPath .githooks
```
This is a repository-local setting (not global). Each clone must run this once. **Do not skip the hook with `--no-verify`.**

### Versioning and Releases
- **SemVer** (`MAJOR.MINOR.PATCH`). Single source of truth: `version` in `Cargo.toml`.
- **Release flow**: Update `Cargo.toml` version → merge to `main` → push tag `v0.2.0` → CI builds Windows binary and creates GitHub Release automatically.
- **Tag must match `Cargo.toml` version.** The release CI verifies this and fails on mismatch.
- Do not create tags or releases without explicit instruction from the user.

### AI Co-Authorship
Include the appropriate trailer in commit messages:
- **Claude Code**: `Co-Authored-By: Claude <noreply@anthropic.com>`
- **Gemini/Antigravity**: `Co-authored-by: Gemini Code Assist[bot] (Antigravity) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`

### Parallel Development Guidelines
Multiple AI agents may work on separate issues simultaneously. Follow these rules to minimize merge conflicts:

- **Extract new modules.** New features should live in dedicated files (e.g., `src/egui_overlay.rs`, `src/drag_drop.rs`). Keep changes to `main.rs` minimal — ideally just `mod` declarations, field additions to `ApplicationState`, and call sites.
- **Conflict-prone files**: `main.rs`, `Cargo.toml`, `config.rs` are modified by most features. Keep diffs small and localized.
- **Rebase before PR.** Always rebase your branch on latest `main` before requesting review. Resolve conflicts in your branch, not on `main`.
- **One feature per branch.** Do not bundle unrelated changes. Each branch maps to one or more related issues.

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
- `assets/shaders/transition.wgsl` - 20 transition effects (embedded).