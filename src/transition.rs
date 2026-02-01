//! Transition material system with embedded WGSL shaders
//!
//! Provides 22 different transition effects for slideshow image switching.
//! The WGSL shader is embedded at compile time for standalone distribution.

// ShaderType derive macro generates unused check functions
#![allow(dead_code)]

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};

use crate::consts::TRANSITION_SHADER_HANDLE;

/// Transition material plugin
pub struct TransitionPlugin;

impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        // Load and register the embedded shader
        let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
        shaders.insert(
            &TRANSITION_SHADER_HANDLE,
            Shader::from_wgsl(
                include_str!("../assets/shaders/transition.wgsl"),
                file!(),
            ),
        ).expect("Failed to insert transition shader");

        app.add_plugins(Material2dPlugin::<TransitionMaterial>::default())
            .add_message::<TransitionEvent>()
            .init_resource::<TransitionState>();
            // Note: update_transitions is now scheduled in main.rs for explicit ordering
    }
}

/// Uniform data for transition shader
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct TransitionUniform {
    pub blend: f32,
    pub mode: i32,
    pub aspect_ratio: Vec2,
    pub bg_color: Vec4,
    pub window_size: Vec2,
    pub image_a_size: Vec2,
    pub image_b_size: Vec2,
}

/// Transition material for custom shader effects
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TransitionMaterial {
    /// Uniform data (blend, mode, aspect_ratio, bg_color)
    #[uniform(0)]
    pub uniforms: TransitionUniform,

    /// First texture (current image)
    #[texture(1)]
    #[sampler(2)]
    pub texture_a: Handle<Image>,

    /// Second texture (next image)
    #[texture(3)]
    #[sampler(4)]
    pub texture_b: Handle<Image>,
}

impl Material2d for TransitionMaterial {
    fn fragment_shader() -> ShaderRef {
        // Use the pre-registered shader handle
        TRANSITION_SHADER_HANDLE.into()
    }
}

/// Transition state resource
#[derive(Resource, Default)]
pub struct TransitionState {
    /// The image currently displayed on screen
    pub displayed_image: Option<Handle<Image>>,
    /// Currently active transition
    pub active: Option<ActiveTransition>,
}

/// Active transition data
#[derive(Debug)]
pub struct ActiveTransition {
    /// Warmup flag - first frame resets timer after GPU texture upload
    pub warmup: bool,
    /// Start time
    pub start_time: f32,
    /// Duration in seconds
    pub duration: f32,
    /// Current progress (0.0 to 1.0) - tracked to prevent frame time jumps
    pub progress: f32,
    /// Target image handle (needed for updating displayed_image)
    pub to_image: Handle<Image>,
}

/// Event to trigger a transition
#[derive(Message)]
pub struct TransitionEvent {
    pub from_image: Handle<Image>,
    pub to_image: Handle<Image>,
    pub duration: f32,
    pub mode: i32,
}

/// Handle transition events
pub fn handle_transition_events(
    mut events: MessageReader<TransitionEvent>,
    mut state: ResMut<TransitionState>,
    time: Res<Time>,
    mut metrics: ResMut<crate::diagnostics::TransitionMetrics>,
) {
    for event in events.read() {
        // Initialize displayed_image on first transition
        if state.displayed_image.is_none() {
            state.displayed_image = Some(event.from_image.clone());
        }

        state.active = Some(ActiveTransition {
            warmup: true, // Start in warmup mode to absorb heavy first frame
            start_time: time.elapsed_secs(),
            duration: event.duration,
            progress: 0.0, // Initialize progress tracking
            to_image: event.to_image.clone(),
        });

        crate::diagnostics::track_transition_start(&mut metrics);
        info!("Starting transition: mode {} duration {}s", event.mode, event.duration);
    }
}

/// Update active transitions
pub fn update_transitions(
    mut state: ResMut<TransitionState>,
    time: Res<Time>,
    mut materials: ResMut<Assets<TransitionMaterial>>,
    mut metrics: ResMut<crate::diagnostics::TransitionMetrics>,
) {
    let Some(ref mut transition) = state.active else {
        return;
    };

    // [WARMUP] Handle first frame after transition start
    // The first frame often has heavy GPU work (texture upload)
    // We absorb this delay by resetting the timer AFTER the heavy frame
    if transition.warmup {
        transition.warmup = false;
        transition.start_time = time.elapsed_secs(); // Reset timer to NOW

        // Ensure blend starts at 0.0
        for (_id, material) in materials.iter_mut() {
            material.uniforms.blend = 0.0;
        }
        return;
    }

    // Handle instant transitions (duration = 0)
    if transition.duration == 0.0 {
        // Update all materials to blend=1.0 to show texture_b immediately
        for (_id, material) in materials.iter_mut() {
            material.uniforms.blend = 1.0;
        }

        // Clone target before modifying state
        let to_image = transition.to_image.clone();

        // Update displayed_image to target
        state.displayed_image = Some(to_image);
        // Instant transition - mark as complete immediately
        state.active = None;
        info!("Transition complete (instant)");
        return;
    }

    // Use delta-based progression with frame time clamping to prevent stuttering
    // Clamp delta to ~30fps minimum (0.033s) to prevent large jumps from frame spikes
    let delta = time.delta_secs().min(0.033);
    let progress_delta = delta / transition.duration;

    // Update progress incrementally instead of recalculating from elapsed time
    // This prevents jumps when frame time spikes occur
    let linear_progress = (transition.progress + progress_delta).clamp(0.0, 1.0);
    transition.progress = linear_progress;

    // Apply smoothstep easing for natural-looking animation
    // Formula: 3x² - 2x³
    // This makes the transition start slowly, speed up, then slow down at the end
    // Also masks small stutters by reducing visual impact of frame time variations
    let eased_progress = linear_progress * linear_progress * (3.0 - 2.0 * linear_progress);

    // Update all transition materials with eased progress
    for (_id, material) in materials.iter_mut() {
        material.uniforms.blend = eased_progress;
    }

    // Remove transition when complete
    if transition.progress >= 1.0 {
        // Clone target before modifying state
        let to_image = transition.to_image.clone();
        let actual_duration = time.elapsed_secs() - transition.start_time;

        state.displayed_image = Some(to_image);
        state.active = None;

        crate::diagnostics::track_transition_complete(&mut metrics, actual_duration);
        info!("Transition complete (actual duration: {:.3}s)", actual_duration);

        // Note: pending_target will be processed by detect_image_change
        // when it detects that active is None
    }
}

/// Helper to pick a random transition mode
pub fn random_transition_mode() -> i32 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(0..=19) // Modes 0-19 are available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_mode() {
        for _ in 0..100 {
            let mode = random_transition_mode();
            assert!(mode >= 0 && mode <= 19);
        }
    }
}
