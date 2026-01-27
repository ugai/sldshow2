//! Transition material system with embedded WGSL shaders
//!
//! Provides 22 different transition effects for slideshow image switching.
//! The WGSL shader is embedded at compile time for standalone distribution.

// ShaderType derive macro generates unused check functions
#![allow(dead_code)]

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Shader, ShaderRef, ShaderType};
use bevy::sprite::{Material2d, Material2dPlugin};

/// Transition material plugin
pub struct TransitionPlugin;

// Shader handle for the transition shader
pub const TRANSITION_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x1234_5678_9abc_def0);

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
        );

        app.add_plugins(Material2dPlugin::<TransitionMaterial>::default())
            .add_event::<TransitionEvent>()
            .init_resource::<TransitionState>()
            .add_systems(Update, (handle_transition_events, update_transitions));
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
#[derive(Debug, Clone)]
pub struct ActiveTransition {
    /// Start time
    pub start_time: f32,
    /// Duration in seconds
    pub duration: f32,
    /// Transition mode
    #[allow(dead_code)]
    pub mode: i32,
    /// Source image handle
    #[allow(dead_code)]
    pub from_image: Handle<Image>,
    /// Target image handle
    #[allow(dead_code)]
    pub to_image: Handle<Image>,
}

/// Event to trigger a transition
#[derive(Event)]
pub struct TransitionEvent {
    pub from_image: Handle<Image>,
    pub to_image: Handle<Image>,
    pub duration: f32,
    pub mode: i32,
}

/// Handle transition events
fn handle_transition_events(
    mut events: EventReader<TransitionEvent>,
    mut state: ResMut<TransitionState>,
    time: Res<Time>,
) {
    for event in events.read() {
        // Initialize displayed_image on first transition
        if state.displayed_image.is_none() {
            state.displayed_image = Some(event.from_image.clone());
        }

        state.active = Some(ActiveTransition {
            start_time: time.elapsed_secs(),
            duration: event.duration,
            mode: event.mode,
            from_image: event.from_image.clone(),
            to_image: event.to_image.clone(),
        });

        info!("Starting transition: mode {} duration {}s", event.mode, event.duration);
    }
}

/// Update active transitions
fn update_transitions(
    mut state: ResMut<TransitionState>,
    time: Res<Time>,
    mut materials: ResMut<Assets<TransitionMaterial>>,
) {
    let Some(ref transition) = state.active else {
        return;
    };

    // Handle instant transitions (duration = 0)
    if transition.duration == 0.0 {
        // Update displayed_image to target
        state.displayed_image = Some(transition.to_image.clone());
        // Instant transition - mark as complete immediately
        state.active = None;
        info!("Transition complete (instant)");
        return;
    }

    let elapsed = time.elapsed_secs() - transition.start_time;
    let progress = (elapsed / transition.duration).clamp(0.0, 1.0);

    // Update all transition materials
    for (_id, material) in materials.iter_mut() {
        material.uniforms.blend = progress;
    }

    // Remove transition when complete
    if progress >= 1.0 {
        state.displayed_image = Some(transition.to_image.clone());
        state.active = None;
        info!("Transition complete");
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
