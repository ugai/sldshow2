# CLAUDE.md

sldshow2: Rust slideshow viewer with WGSL transition effects (winit + wgpu). See [CONTRIBUTING.md](CONTRIBUTING.md) for full development workflow.

## Build & Test

```bash
cargo run --release -- example.sldshow   # Visual testing (ALWAYS use --release)
cargo build                           # Compile check only
```

**IMPORTANT**: Always use `--release` for visual/performance testing. Debug builds of `image` and `wgpu` are slow and cause frame stutters that don't reflect production behavior.

```bash
cargo test                            # Run unit tests (no GPU required)
```

## Architecture

For the full Module Map and Architecture details, see **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

## Conventions

- **Commit/PR/issue/branch titles**: [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `refactor:`, etc.
- **Branch names**: `feat/kebab-description`, `fix/kebab-description`
- **PRs**: Squash merge only. Reference issues with `Closes #N`. No direct push to `main`.
- **Pre-commit hook**: Runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Do not skip with `--no-verify`.
- Always run `cargo fmt --all` before committing.

## AI Agent Rules

- **Co-authorship trailer**: `Co-Authored-By: {model} (Claude Code) <noreply@anthropic.com>`
- **Do not** create git tags or releases unless explicitly instructed.
- **New features**: Extract to dedicated modules (e.g., `src/drag_drop.rs`). Keep `main.rs` and `app.rs` diffs minimal.
- **Conflict-prone files**: `app.rs`, `main.rs`, `Cargo.toml`, `config.rs` — keep changes small and localized.
- **Avoid hardcoding counts** in docs or comments (e.g., "20 transitions", "6 perspectives"). Counts change as features are added. Write descriptively ("multiple", "each") and let the source of truth (code, config, RECON.md) be the only place the number lives.
