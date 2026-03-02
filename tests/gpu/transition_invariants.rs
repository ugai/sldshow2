//! GPU integration tests for transition shader invariants.
//!
//! These tests render the transition WGSL shader to an offscreen texture using
//! wgpu and verify three fundamental invariants that must hold for **every**
//! transition mode (0–19):
//!
//! | `blend` value | Expected output |
//! |:---:|:---|
//! | `0.0` | All pixels ≈ **texture_a** colour (red) |
//! | `1.0` | All pixels ≈ **texture_b** colour (blue) |
//! | `0.5` | Average pixel ≠ red **and** ≠ blue (mid-blend visible) |
//!
//! The two source textures are 1×1 solid colours:
//! - **texture_a** — red  `[255, 0, 0, 255]`
//! - **texture_b** — blue `[0, 0, 255, 255]`
//!
//! # Skipping in headless CI
//!
//! If no GPU adapter is available (e.g. a GitHub Actions runner without a
//! GPU), [`helpers::try_setup_gpu`] returns `None` and each test returns
//! early — it is **not** counted as a failure.

mod helpers;

use helpers::{
    PIXEL_EPSILON, RENDER_SIZE, assert_avg_approx, assert_avg_not_approx,
    create_solid_color_texture, render_transition, try_setup_gpu,
};

// ── Solid-colour definitions (0-255 scale, as f32 for comparison) ──────────

/// texture_a: solid red
const RED: [u8; 4] = [255, 0, 0, 255];
/// texture_b: solid blue
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Expected average colour when the output should be ≈ red (f32, 0–255).
const RED_F: [f32; 4] = [255.0, 0.0, 0.0, 255.0];
/// Expected average colour when the output should be ≈ blue (f32, 0–255).
const BLUE_F: [f32; 4] = [0.0, 0.0, 255.0, 255.0];

/// Minimum per-channel difference required for the "mid-blend" assertion.
///
/// At `blend = 0.5`, discrete transitions show ~half the pixels as red and
/// ~half as blue, giving an average ≈ (128, 0, 128, 255).  The maximum
/// channel difference from pure red is ≥ 100 (blue channel 0→128), and from
/// pure blue is ≥ 100 (red channel 0→128).  A threshold of 50 is conservative
/// enough to pass all modes while still catching regressions.
const MID_BLEND_MIN_DIFF: f32 = 50.0;

// ---------------------------------------------------------------------------
// Helper: run all three invariant checks for one transition mode
// ---------------------------------------------------------------------------

fn check_mode_invariants(mode: i32) {
    let Some(ctx) = try_setup_gpu() else {
        eprintln!("No GPU adapter available — skipping GPU tests (mode {mode})");
        return;
    };

    let tex_a = create_solid_color_texture(&ctx, RED);
    let tex_b = create_solid_color_texture(&ctx, BLUE);

    // ── Invariant 1: blend = 0.0 → output ≈ texture_a (red) ────────────────
    {
        let pixels = render_transition(&ctx, &tex_a, &tex_b, mode, 0.0);
        assert_eq!(
            pixels.len() as u32,
            RENDER_SIZE * RENDER_SIZE * 4,
            "mode {mode}: unexpected pixel buffer length at blend=0.0"
        );
        assert_avg_approx(
            &pixels,
            RED_F,
            PIXEL_EPSILON as f32,
            &format!("mode {mode} blend=0.0"),
        );
    }

    // ── Invariant 2: blend = 1.0 → output ≈ texture_b (blue) ───────────────
    {
        let pixels = render_transition(&ctx, &tex_a, &tex_b, mode, 1.0);
        assert_avg_approx(
            &pixels,
            BLUE_F,
            PIXEL_EPSILON as f32,
            &format!("mode {mode} blend=1.0"),
        );
    }

    // ── Invariant 3: blend = 0.5 → output is in mid-blend state ─────────────
    // The average must differ from both pure red and pure blue by at least
    // MID_BLEND_MIN_DIFF in at least one channel.
    {
        let pixels = render_transition(&ctx, &tex_a, &tex_b, mode, 0.5);
        assert_avg_not_approx(
            &pixels,
            RED_F,
            MID_BLEND_MIN_DIFF,
            &format!("mode {mode} blend=0.5 (not pure red)"),
        );
        assert_avg_not_approx(
            &pixels,
            BLUE_F,
            MID_BLEND_MIN_DIFF,
            &format!("mode {mode} blend=0.5 (not pure blue)"),
        );
    }
}

// ---------------------------------------------------------------------------
// One test per transition mode
// ---------------------------------------------------------------------------

#[test]
fn mode_00_crossfade() {
    check_mode_invariants(0);
}

#[test]
fn mode_01_smooth_crossfade() {
    check_mode_invariants(1);
}

#[test]
fn mode_02_roll_from_top() {
    check_mode_invariants(2);
}

#[test]
fn mode_03_roll_from_bottom() {
    check_mode_invariants(3);
}

#[test]
fn mode_04_roll_from_left() {
    check_mode_invariants(4);
}

#[test]
fn mode_05_roll_from_right() {
    check_mode_invariants(5);
}

#[test]
fn mode_06_roll_from_top_left() {
    check_mode_invariants(6);
}

#[test]
fn mode_07_roll_from_top_right() {
    check_mode_invariants(7);
}

#[test]
fn mode_08_roll_from_bottom_left() {
    check_mode_invariants(8);
}

#[test]
fn mode_09_roll_from_bottom_right() {
    check_mode_invariants(9);
}

#[test]
fn mode_10_sliding_door_open() {
    check_mode_invariants(10);
}

#[test]
fn mode_11_sliding_door_close() {
    check_mode_invariants(11);
}

#[test]
fn mode_12_blind_horizontal_a() {
    check_mode_invariants(12);
}

#[test]
fn mode_13_blind_horizontal_b() {
    check_mode_invariants(13);
}

#[test]
fn mode_14_blind_vertical_a() {
    check_mode_invariants(14);
}

#[test]
fn mode_15_blind_vertical_b() {
    check_mode_invariants(15);
}

#[test]
fn mode_16_box_expand() {
    check_mode_invariants(16);
}

#[test]
fn mode_17_box_contract() {
    check_mode_invariants(17);
}

#[test]
fn mode_18_random_squares() {
    check_mode_invariants(18);
}

#[test]
fn mode_19_angular_wipe() {
    check_mode_invariants(19);
}

// ---------------------------------------------------------------------------
// Sanity test: verify that the test setup itself works correctly
// ---------------------------------------------------------------------------

/// Confirms that the GPU helper infrastructure returns sensible pixel data
/// independently of any particular transition mode.
#[test]
fn gpu_setup_sanity() {
    let Some(ctx) = try_setup_gpu() else {
        eprintln!("No GPU adapter available — skipping GPU tests");
        return;
    };

    // A 4×4 red texture rendered with blend=0.0 must produce all-red pixels.
    let red = create_solid_color_texture(&ctx, RED);
    let blue = create_solid_color_texture(&ctx, BLUE);
    let pixels = render_transition(&ctx, &red, &blue, 0, 0.0);

    assert_eq!(
        pixels.len() as u32,
        RENDER_SIZE * RENDER_SIZE * 4,
        "pixel buffer must be RENDER_SIZE² × 4 bytes"
    );

    // Every pixel must be approximately red.
    for (i, chunk) in pixels.chunks_exact(4).enumerate() {
        assert!(
            (chunk[0] as i32 - 255).abs() <= PIXEL_EPSILON as i32,
            "pixel {i}: R channel {}, expected ≈255",
            chunk[0]
        );
        assert!(
            chunk[1] <= PIXEL_EPSILON,
            "pixel {i}: G channel {}, expected ≈0",
            chunk[1]
        );
        assert!(
            chunk[2] <= PIXEL_EPSILON,
            "pixel {i}: B channel {}, expected ≈0",
            chunk[2]
        );
    }
}
