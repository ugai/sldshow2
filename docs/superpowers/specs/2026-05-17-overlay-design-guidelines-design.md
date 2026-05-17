# Design Spec: DESIGN_GUIDELINES.md for Overlay Theming

**Issue:** #419  
**Date:** 2026-05-17  
**Status:** Approved

## Context

Overlay UI styling is fragmented across three implicit conventions in sldshow2:
1. Bare egui defaults — `settings.rs`, `help.rs` (root cause of visibility complaint)
2. `config.style.text_color` via RichText — filename / OSD / info bar in `mod.rs`
3. Ad-hoc per-widget tweaks — gallery spacing, OSC hand-drawn background, `Color32::GRAY` in `help.rs`

This spec defines what `docs/DESIGN_GUIDELINES.md` must contain and the design decisions agreed upon.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Contrast standard | Body ≥ 7:1, headings ≥ 4.5:1 (fixed values) | Visible improvement over egui default ~4.5:1; numeric enough for agent checks |
| Theme scope | Dark only; Light unsupported | sldshow2 is a dark-first image viewer |
| Color tokens | Fixed constants; not user-configurable | Simplicity; avoids user breaking contrast |
| HUD colors | Excluded from apply_theme(); stays on `config.style.text_color` | HUD is user-configurable by design |
| Color reference | Radix UI Gray Dark scale | Principled, well-documented, semantic step names |
| apply_theme() scope | Settings, Help, Gallery, OSC | HUD excluded |
| PR checklist | Medium (6 items); --auto-screenshot step gated on #421 | Balances rigor with tooling availability |
| Document structure | Rule-first | Rules before tokens; most important info first |

## Deliverable

The agent implementing #419 must create `docs/DESIGN_GUIDELINES.md` with the
exact structure and content defined below.

---

## Draft: docs/DESIGN_GUIDELINES.md

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

  | Constant       | Color (approx)             | Usage               |
  |----------------|----------------------------|---------------------|
  | `COLOR_ERROR`  | `Color32::from_rgb(220, 80, 80)`  | Error messages |
  | `COLOR_WARN`   | `Color32::from_rgb(220, 180, 60)` | Warnings       |
  | `COLOR_HINT`   | `Color32::from_rgb(140, 140, 140)`| Dim hints / footers (intentionally low-contrast; for decorative secondary text only) |

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

| Constant                | Radix Step    | Approx hex  | Usage                              |
|-------------------------|---------------|-------------|------------------------------------|
| `PANEL_FILL`            | Gray Dark 2   | `#191919`   | Window / panel background          |
| `WIDGET_BG`             | Gray Dark 4   | `#2b2b2b`   | Button / input background          |
| `WIDGET_BG_HOVERED`     | Gray Dark 5   | `#333333`   | Hover state                        |
| `WIDGET_BG_ACTIVE`      | Gray Dark 6   | `#3c3c3c`   | Pressed / active state             |
| `SEPARATOR`             | Gray Dark 6   | `#3c3c3c`   | `ui.separator()` stroke            |
| `TEXT_PRIMARY`          | Gray Dark 12  | `#eeeeee`   | Body text, labels  (≈ 14:1 ✓)     |
| `TEXT_HEADING`          | Gray Dark 12  | `#eeeeee`   | Headings                           |
| `STROKE_NONINTERACTIVE` | Gray Dark 12  | `#eeeeee`   | Non-interactive widget text        |

> **Implementation note:** Confirm exact hex values from <https://www.radix-ui.com/colors>
> at implementation time. Copy values directly from the Radix source; do not approximate.

**Contrast requirements (measured against `PANEL_FILL`):**
- Body text / labels: **≥ 7:1**
- Headings: **≥ 4.5:1**
- Non-text UI elements (separators, widget borders): no minimum.

**Contrast formula:**

```
ratio = (L_lighter + 0.05) / (L_darker + 0.05)
L = 0.2126·R + 0.7152·G + 0.0722·B   (R, G, B linearized from sRGB: x/255 if ≤ 0.04045, else ((x/255 + 0.055)/1.055)^2.4)
```

## Spacing Tokens

Set once in `apply_theme()` via `style.spacing.*`. Overlay modules must not
override these locally.

| Field                   | Value            | Usage                                   |
|-------------------------|------------------|-----------------------------------------|
| `spacing.item_spacing`  | `(8.0, 8.0)`     | Vertical / horizontal gap between widgets |
| `spacing.window_margin` | `12` (all sides) | Window inner padding                    |
| `spacing.button_padding`| egui default     | Do not override                         |
| `spacing.indent`        | egui default     | Do not override                         |

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

### HUD colors (config.style.text_color)

The filename bar, OSD, and info bar use `config.style.text_color` via `RichText`
and are intentionally excluded from `apply_theme()`. Users may set this color in
their TOML config. `config.style.bg_color` is similarly user-configurable and
excluded from centralized theming.

These fields are candidates for future cleanup once a more complete theming
system is designed, but removal is out of scope for this initiative.

### Light theme

Light theme is not supported. `apply_theme()` writes only the Dark style slot.
If Light theme support is added in the future, a separate pass updating this
document and `apply_theme()` will be required.
```

## References

- Issue #420: implement `apply_theme()` (depends on this spec)
- Issue #421: `--auto-screenshot` CLI flag (enables checklist screenshot step)
- Issue #422: vision-model reviewer agent (depends on #419 + #421)
- Issue #423: parent epic
- Radix UI Colors: https://www.radix-ui.com/colors
