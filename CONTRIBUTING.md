# Contributing to sldshow2

## Getting Started

### Prerequisites
- Rust (stable toolchain)
- On Linux: `libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev`

### Setup
```bash
git clone https://github.com/ugai/sldshow2.git
cd sldshow2
git config core.hooksPath .githooks
```

The last command enables the pre-commit hook that runs `cargo fmt`, `cargo clippy`, and `cargo test` automatically before each commit.

## Development Workflow

### Branch Naming
```
<type>/<kebab-case-description>
```
Examples: `feat/ambient-fit-shader`, `fix/transition-flash`, `refactor/filter-mode-enum`

### Making Changes
1. Create a feature branch from `main`
2. Implement your changes
3. Commit with [Conventional Commits](https://www.conventionalcommits.org/) format: `feat: add ambient fit shader`
4. The pre-commit hook will check formatting, linting, and tests automatically
5. Push and open a pull request

### Pull Requests
- **All changes go through PRs** — no direct push to `main`
- **Squash merge only** — PRs are squash-merged to keep history clean
- **CI must pass** before merge (check, fmt, clippy, test, Windows build)
- Reference related issues in the body: `Closes #38`
- PR titles follow Conventional Commits format

### Pre-Commit Hook
The repository includes a pre-commit hook at `.githooks/pre-commit` that runs:
1. `cargo fmt --all -- --check`
2. `cargo clippy --all-features -- -D warnings`
3. `cargo test --all-features`

**Setup** (required once per clone):
```bash
git config core.hooksPath .githooks
```

Do not skip the hook with `--no-verify`.

### Code Quality Commands
```bash
cargo fmt --all          # Format code
cargo clippy --all-features -- -D warnings  # Lint
cargo test --all-features                   # Run tests
cargo build --release    # Build (use --release for visual testing)
```

## Code Style
- **English** for all comments and documentation
- **Structured logging** via `log` crate (`info!`, `warn!`, `error!`)
- Avoid global mutable state

## Architecture
For detailed architecture documentation, see [CLAUDE.md](CLAUDE.md).

### Parallel Development
Multiple contributors may work on separate issues simultaneously. To minimize merge conflicts:
- **New features should live in dedicated modules** (e.g., `src/egui_overlay.rs`). Keep changes to `main.rs` minimal.
- **Rebase on latest `main`** before opening a PR.
- **One feature per branch** — do not bundle unrelated changes.

## Versioning and Releases
- Uses **SemVer** (`MAJOR.MINOR.PATCH`). Version is managed in `Cargo.toml`.
- Pushing a tag like `v0.2.0` triggers the release workflow, which builds a Windows binary and creates a GitHub Release.
- The tag must match the version in `Cargo.toml` — the CI verifies this.

## Project Board
Active issues and roadmap are tracked on the [GitHub Project](https://github.com/users/ugai/projects/1).
