# Design Guidelines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `docs/DESIGN_GUIDELINES.md` as the single source of truth for overlay visual conventions and link it from `CLAUDE.md` and `CONTRIBUTING.md`.

**Architecture:** Documentation-only change. No Rust code is modified. The guideline document uses Rule-first structure: Rules → Color Tokens (Radix Gray Dark scale) → Spacing Tokens → PR Checklist. Color token values follow the Radix UI Gray Dark scale; exact hex must be copied from https://www.radix-ui.com/colors at write time.

**Tech Stack:** Markdown, Radix UI Gray Dark color scale reference.

---

## File Map

| Action | Path | Purpose |
|--------|------|---------|
| Create | `docs/DESIGN_GUIDELINES.md` | The guideline document (deliverable) |
| Modify | `CLAUDE.md` | Add reference link under Conventions section |
| Modify | `CONTRIBUTING.md` | Add reference link under Code Style section |
| Modify | `.gitignore` | Add `.superpowers/` entry |

---

### Task 1: Add `.superpowers/` to `.gitignore`

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Append the entry**

Open `.gitignore` and add at the end:

```
# Brainstorming / planning session artifacts
.superpowers/
```

- [ ] **Step 2: Verify**

```bash
grep "superpowers" .gitignore
```

Expected output:
```
.superpowers/
```

- [ ] **Step 3: Commit**

```bash
git add .gitignore
git commit -m "chore: ignore .superpowers/ brainstorm artifacts"
```

---

### Task 2: Create `docs/DESIGN_GUIDELINES.md`

**Files:**
- Create: `docs/DESIGN_GUIDELINES.md`

Before writing, fetch the exact Radix UI Gray Dark hex values from
https://www.radix-ui.com/colors (Gray → Dark tab). You need steps 2, 4, 5, 6, 12.

- [ ] **Step 1: Verify Radix Gray Dark values**

Open https://www.radix-ui.com/colors in a browser, select **Gray** → **Dark**.
Record the exact hex for:
- Step 2 → `PANEL_FILL`
- Step 4 → `WIDGET_BG`
- Step 5 → `WIDGET_BG_HOVERED`
- Step 6 → `WIDGET_BG_ACTIVE` and `SEPARATOR`
- Step 12 → `TEXT_PRIMARY`, `TEXT_HEADING`, `STROKE_NONINTERACTIVE`

- [ ] **Step 2: Verify contrast ratios**

For each text token, compute the contrast ratio against `PANEL_FILL` using:

```
L(hex) = 0.2126·R_lin + 0.7152·G_lin + 0.0722·B_lin
R_lin = (R/255)^2.2  (simplified sRGB linearization)
ratio = (L_lighter + 0.05) / (L_darker + 0.05)
```

Confirm:
- `TEXT_PRIMARY` (Gray Dark 12) vs `PANEL_FILL` (Gray Dark 2) ≥ 7:1  ✓ (expect ~12–14:1)
- `TEXT_HEADING` same token, same check ✓

- [ ] **Step 3: Write the file**

Create `docs/DESIGN_GUIDELINES.md` with the following content, substituting
`<STEP_N_HEX>` with the actual values from Step 1:

```markdown
# sldshow2 Design Guidelines

All overlay visual styling is centralized in `apply_theme(ctx, &config.style)`
in `src/overlay/mod.rs`. This document defines the rules, tokens, and review
checklist that govern every overlay UI change.

**Scope:** Settings, Help, Gallery, OSC overlays.  
**Out of scope:** HUD (filename bar, OSD, info bar) — uses `config.style.text_color` directly.  
**Light theme:** Not supported. All rules apply to Dark theme only.

## Rules

### Do
- Use bare `ui.label()`, `ui.heading()`, `ui.button()` in overlay modules and
  let `apply_theme()` handle all visual properties.
- Add new styling tokens to `apply_theme()` when a new visual property is needed.
- Use `RichText::new(...).color(...)` **only** for semantic emphasis:

  | Constant       | Color (approx)                     | Usage                                                          |
  |----------------|------------------------------------|----------------------------------------------------------------|
  | `COLOR_ERROR`  | `Color32::from_rgb(220, 80, 80)`   | Error messages                                                 |
  | `COLOR_WARN`   | `Color32::from_rgb(220, 180, 60)`  | Warnings                                                       |
  | `COLOR_HINT`   | `Color32::from_rgb(140, 140, 140)` | Dim hints / footers (intentionally low-contrast; decorative secondary text only) |

### Don't
- Do not call `ui.visuals_mut()`, `ui.style_mut()`, or `ctx.set_style()` inside
  overlay modules — those belong exclusively in `apply_theme()`.
- Do not use `Color32::GRAY`, `Color32::WHITE`, or any ad-hoc literal color
  outside of `apply_theme()` and the semantic constants above.
- Do not use `set_global_style()` — it writes only the current theme slot.
  Use `ctx.style_mut()` scoped to Dark inside `apply_theme()` instead.

## Color Tokens

All tokens are fixed constants defined in `apply_theme()`. Values follow the
**Radix UI Gray Dark** scale (<https://www.radix-ui.com/colors>).

| Constant                | Radix Step    | Hex         | Usage                              |
|-------------------------|---------------|-------------|------------------------------------|
| `PANEL_FILL`            | Gray Dark 2   | `<STEP_2_HEX>`  | Window / panel background      |
| `WIDGET_BG`             | Gray Dark 4   | `<STEP_4_HEX>`  | Button / input background      |
| `WIDGET_BG_HOVERED`     | Gray Dark 5   | `<STEP_5_HEX>`  | Hover state                    |
| `WIDGET_BG_ACTIVE`      | Gray Dark 6   | `<STEP_6_HEX>`  | Pressed / active state         |
| `SEPARATOR`             | Gray Dark 6   | `<STEP_6_HEX>`  | `ui.separator()` stroke        |
| `TEXT_PRIMARY`          | Gray Dark 12  | `<STEP_12_HEX>` | Body text, labels              |
| `TEXT_HEADING`          | Gray Dark 12  | `<STEP_12_HEX>` | Headings                       |
| `STROKE_NONINTERACTIVE` | Gray Dark 12  | `<STEP_12_HEX>` | Non-interactive widget text    |

**Contrast requirements (measured against `PANEL_FILL`):**
- Body text / labels: **≥ 7:1**
- Headings: **≥ 4.5:1**
- Non-text UI elements (separators, widget borders): no minimum.

**Contrast formula:**

```
ratio = (L_lighter + 0.05) / (L_darker + 0.05)
L = 0.2126·R + 0.7152·G + 0.0722·B
```

where R, G, B are linearized from sRGB:
`c_lin = (c/255 / 12.92)` if `c/255 ≤ 0.04045`, else `((c/255 + 0.055) / 1.055)^2.4`

## Spacing Tokens

Set once in `apply_theme()` via `style.spacing.*`. Overlay modules must not
override these locally.

| Field                   | Value            | Usage                                       |
|-------------------------|------------------|---------------------------------------------|
| `spacing.item_spacing`  | `(8.0, 8.0)`     | Vertical / horizontal gap between widgets   |
| `spacing.window_margin` | `12` (all sides) | Window inner padding                        |
| `spacing.button_padding`| egui default     | Do not override                             |
| `spacing.indent`        | egui default     | Do not override                             |

> **Exception:** `gallery.rs` adjusts `item_spacing` locally for the thumbnail
> grid cell layout. This is permitted because it is a layout-level override
> scoped to the scroll area, not a color or text style override.

## PR Review Checklist

Apply to every PR that touches `src/overlay/` or `src/osc.rs`.

### Required checks

- [ ] No new `.color(...)` calls outside `apply_theme()` and the three semantic
      constants (`COLOR_ERROR`, `COLOR_WARN`, `COLOR_HINT`).
- [ ] New overlays use bare `ui.label()` / `ui.heading()` — no local
      `visuals_mut()` / `style_mut()` calls.
- [ ] If `apply_theme()` was modified: manually verify Settings, Help, Gallery,
      and OSC all render correctly.

### Contrast check (required when adding or changing text)

Compute contrast ratio against `PANEL_FILL` using the formula in the Color
Tokens section. Confirm:
- Body text: ≥ 7:1
- Headings: ≥ 4.5:1

### Screenshot (required when adding or changing an overlay)

Capture the affected overlay with `--auto-screenshot` and attach the PNG to
the PR description.

> **NOTE:** `--auto-screenshot` is not yet available (tracked in #421).
> Until #421 lands, take a manual screenshot with the `S` key and attach it.

### Not required

- Light theme testing — Light is not supported (see Scope).

## Notes

### HUD colors (`config.style.text_color`)

The filename bar, OSD, and info bar use `config.style.text_color` via `RichText`
and are intentionally excluded from `apply_theme()`. Users may set this color in
their TOML config. `config.style.bg_color` is similarly user-configurable.

These fields are candidates for future cleanup once a more complete theming
system is designed, but removal is out of scope for this initiative.

### Light theme

Light theme is not supported. `apply_theme()` writes only the Dark style slot.
If Light theme support is added in the future, a separate pass updating this
document and `apply_theme()` will be required.
```

- [ ] **Step 4: Verify required sections exist**

```bash
grep -c "^## " docs/DESIGN_GUIDELINES.md
```

Expected: `4` (Rules, Color Tokens, Spacing Tokens, PR Review Checklist)

```bash
grep "PANEL_FILL\|TEXT_PRIMARY\|WIDGET_BG\|SEPARATOR\|STROKE_NONINTERACTIVE" docs/DESIGN_GUIDELINES.md | wc -l
```

Expected: at least `8` lines (one per token).

- [ ] **Step 5: Commit**

```bash
git add docs/DESIGN_GUIDELINES.md
git commit -m "docs: add DESIGN_GUIDELINES.md for overlay theming and contrast (#419)"
```

---

### Task 3: Link from `CLAUDE.md`

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add reference under Conventions**

In `CLAUDE.md`, find the `## Conventions` section. Add a line at the end of
the bullet list:

```markdown
- **Overlay UI**: See **[docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md)** for color tokens, contrast rules, and the PR checklist for overlay-touching changes.
```

The section should look like:

```markdown
## Conventions

- **Commit/PR/issue/branch titles**: [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `refactor:`, etc.
- **Branch names**: `feat/kebab-description`, `fix/kebab-description`
- **PRs**: Squash merge only. Reference issues with `Closes #N`. No direct push to `main`.
- **Pre-commit hook**: Runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Do not skip with `--no-verify`.
- Always run `cargo fmt --all` before committing.
- **Overlay UI**: See **[docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md)** for color tokens, contrast rules, and the PR checklist for overlay-touching changes.
```

- [ ] **Step 2: Verify**

```bash
grep "DESIGN_GUIDELINES" CLAUDE.md
```

Expected:
```
- **Overlay UI**: See **[docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md)** for color tokens...
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: link DESIGN_GUIDELINES.md from CLAUDE.md"
```

---

### Task 4: Link from `CONTRIBUTING.md`

**Files:**
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Add reference under Code Style**

In `CONTRIBUTING.md`, find the `## Code Style` section. Add at the end of
that section (before `## Architecture`):

```markdown
- **Overlay UI styling**: Follow [docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md) — centralized theming via `apply_theme()`, color tokens from Radix Gray Dark, WCAG contrast rules, and PR checklist.
```

- [ ] **Step 2: Verify**

```bash
grep "DESIGN_GUIDELINES" CONTRIBUTING.md
```

Expected:
```
- **Overlay UI styling**: Follow [docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md)...
```

- [ ] **Step 3: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs: link DESIGN_GUIDELINES.md from CONTRIBUTING.md"
```

---

## Acceptance Criteria Checklist

After all tasks are complete, verify the three acceptance criteria from issue #419:

```bash
# 1. File exists and is linked
ls docs/DESIGN_GUIDELINES.md
grep "DESIGN_GUIDELINES" CLAUDE.md CONTRIBUTING.md

# 2. Contrast rule is concrete (contains formula and numeric thresholds)
grep "7:1\|4.5:1\|0.2126" docs/DESIGN_GUIDELINES.md

# 3. Token table exists (tokens will map 1:1 to apply_theme() fields in #420)
grep "PANEL_FILL\|TEXT_PRIMARY\|WIDGET_BG" docs/DESIGN_GUIDELINES.md
```

All three commands must produce non-empty output.
