# CLAUDE.md

sldshow2: Rust slideshow viewer with 20 WGSL transition effects (winit + wgpu). See [CONTRIBUTING.md](CONTRIBUTING.md) for full development workflow.

## Build & Test

```bash
cargo run --release -- test.sldshow   # Visual testing (ALWAYS use --release)
cargo build                           # Compile check only
```

**IMPORTANT**: Always use `--release` for visual/performance testing. Debug builds of `image` and `wgpu` are slow and cause frame stutters that don't reflect production behavior.

No unit tests — testing is manual only.

## Module Map

| File | Responsibility |
|---|---|
| `main.rs` | Event loop, `ApplicationState`, action execution |
| `input.rs` | Input event handling (keyboard, mouse, cursor) |
| `transition.rs` | wgpu render pipeline, 20 WGSL transition effects |
| `image_loader.rs` | Async texture loading (rayon + channels) |
| `text.rs` | glyphon text rendering |
| `config.rs` | TOML configuration (serde) |
| `slideshow.rs` | Auto-advance timer |
| `clipboard.rs` | Clipboard operations (copy image data) |

## Conventions

- **Commit/PR/issue/branch titles**: [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `refactor:`, etc.
- **Branch names**: `feat/kebab-description`, `fix/kebab-description`
- **PRs**: Squash merge only. Reference issues with `Closes #N`. No direct push to `main`.
- **Pre-commit hook**: Runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Do not skip with `--no-verify`.
- Always run `cargo fmt --all` before committing.

## AI Agent Rules

- **Co-authorship trailer** — format: `Co-Authored-By: {model} ({tool}) <email>`. Use the actual model name:
  - Claude Code: `Co-Authored-By: {model} (Claude Code) <noreply@anthropic.com>` (e.g., `Claude Opus 4.6`, `gpt-oss:20b`)
  - Gemini CLI: `Co-Authored-By: {model} (Gemini CLI) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
  - Antigravity: `Co-Authored-By: {model} (Antigravity) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
- **Do not** create git tags or releases unless explicitly instructed.
- **New features**: Extract to dedicated modules (e.g., `src/drag_drop.rs`). Keep `main.rs` diffs minimal.
- **Conflict-prone files**: `main.rs`, `Cargo.toml`, `config.rs` — keep changes small and localized.
