# Reconnaissance Guide

Scout each perspective in priority order. Stop when you have 15 candidates.

**When referencing code**: use file path + function/struct name, not line
numbers. Line numbers drift between the time you scout and the time a Slayer
picks up the issue.

## Priority 1 — Robustness & Error Handling

- `unwrap` / `expect` in production paths that should be `?` with context
- Edge cases: zero images, corrupt files, very large images, non-image files
- Resource cleanup on exit (GPU resources, file handles)
- Unusual aspect ratios or resolutions

## Priority 2 — Code Quality & Architecture

- Dead code, unused imports, unnecessary clones
- Functions that are too long or do too many things
- Stringly-typed values that should be enums
- Module boundaries that could be cleaner

## Priority 3 — Performance

- Unnecessary allocations in hot paths (per-frame code)
- Texture upload / GPU pipeline inefficiencies
- Image decoding bottlenecks
- Large textures kept alive unnecessarily

## Priority 4 — User Experience

- Missing keyboard shortcuts that similar viewers provide (feh, sxiv, IrfanView)
- Better feedback for loading states or errors
- Window behavior quirks (resize, multi-monitor, taskbar)
- Smoother transitions or animation curves

## Priority 5 — Cross-Platform & Compatibility

- Windows-specific assumptions that break on Linux/macOS
- GPU compatibility (older hardware, integrated GPUs)
- File path edge cases (Unicode, long paths, symlinks)

## Priority 6 — New Features (Small Scope Only)

Scope limit: achievable by adding or modifying at most 2–3 files, no new
subsystem required. If larger, break into sub-issues or reject.

- Configuration options users would expect
- Small quality-of-life additions
- New transition effects (one issue per effect)
