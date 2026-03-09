# GEMINI.md

sldshow2: Rust slideshow viewer with WGSL transition effects (winit + wgpu). See [CONTRIBUTING.md](CONTRIBUTING.md) for full development workflow.

## Build & Test

```bash
cargo run --release -- test.sldshow   # Visual testing (ALWAYS use --release)
cargo build                           # Compile check only
```

**IMPORTANT**: Always use `--release` for visual/performance testing. Debug builds of `image` and `wgpu` are slow and cause frame stutters that don't reflect production behavior.

No unit tests — testing is manual only.

## Architecture

For the full Module Map and Architecture details, see **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

## Conventions

- **Commit/PR/issue/branch titles**: [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `refactor:`, etc.
- **Branch names**: `feat/kebab-description`, `fix/kebab-description`
- **PRs**: Squash merge only. No direct push to `main`. **Always include a detailed body** with `## Summary` (bulleted list of changes) and `## Test plan` (checklist of how it was tested), ending with `Closes #N`.
- **Pre-commit hook**: Runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Do not skip with `--no-verify`.
- Always run `cargo fmt --all` before committing.

## AI Agent Rules

- **Co-authorship trailer** — format: `Co-Authored-By: {model} ({tool}) <email>`. Use the actual model name:
  - Gemini CLI: `Co-Authored-By: {model} (Gemini CLI) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
  - Antigravity: `Co-Authored-By: {model} (Antigravity) <176961590+gemini-code-assist[bot]@users.noreply.github.com>`
- **Do not** create git tags or releases unless explicitly instructed.
- **New features**: Extract to dedicated modules (e.g., `src/drag_drop.rs`). Keep `main.rs` and `app.rs` diffs minimal.
- **Conflict-prone files**: `app.rs`, `main.rs`, `Cargo.toml`, `config.rs` — keep changes small and localized.
