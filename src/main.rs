//! sldshow2 - Simple slideshow image viewer with custom WGSL transitions
//!
//! A Bevy-based slideshow application featuring 22 different transition effects,
//! reactive rendering for power efficiency, and flexible TOML configuration.

mod config;
mod image_loader;
mod slideshow;
mod transition;

use bevy::prelude::*;
use bevy::input::mouse::{MouseButton, MouseWheel};
use bevy::render::mesh::Mesh2d;
use bevy::sprite::MeshMaterial2d;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::window::{WindowMode, PresentMode, MonitorSelection};
use bevy::winit::WinitSettings;
use config::Config;
use futures_lite::future;
use image_loader::{ImageLoader, ImageLoaderPlugin, scan_image_paths};
use slideshow::{SlideshowAdvanceEvent, SlideshowPlugin, SlideshowTimer};
use std::path::PathBuf;
use transition::{TransitionEvent, TransitionMaterial, TransitionPlugin, TransitionState, TransitionUniform};

/// Main entry point for sldshow2 application
fn main() {
    // Load configuration
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(std::path::PathBuf::from);

    let config = Config::load_default(config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        eprintln!("Using default configuration");
        Config::default()
    });

    // Determine window mode
    let window_mode = if config.window.fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Index(config.window.monitor_index))
    } else {
        WindowMode::Windowed
    };

    let mut app = App::new();
    app
        // Continuous mode for smooth transitions (will be toggled to reactive when idle)
        .insert_resource(WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        })
        .insert_resource(config.clone())
        .insert_resource(SlideshowTimer::new(config.viewer.timer))
        .init_resource::<ImageChangeTracker>()
        .init_resource::<KeyRepeatTimer>()
        .init_resource::<InitialScanState>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "sldshow2".to_string(),
                        resolution: (config.window.width as f32, config.window.height as f32).into(),
                        present_mode: PresentMode::AutoVsync,
                        decorations: config.window.decorations,
                        resizable: config.window.resizable,
                        mode: window_mode,
                        ..default()
                    }),
                    ..default()
                })
        )
        .add_plugins(ImageLoaderPlugin)
        .add_plugins(SlideshowPlugin)
        .add_plugins(TransitionPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            start_image_scan,
            poll_image_scan,
            keyboard_input_system,
            handle_slideshow_advance,
            detect_image_change,
            trigger_transition.after(detect_image_change),
            update_transition_on_resize,
        ).chain())
        .add_systems(Update, update_image_path_text)
        .run();
}

/// Font handle for the embedded M PLUS 2 font
const EMBEDDED_FONT_HANDLE: Handle<Font> = Handle::weak_from_u128(0xabcd_1234_5678_90ef);

/// Resource to track initial scan state
#[derive(Resource, Default)]
struct InitialScanState {
    scanned: bool,
    frame_count: u32,
}


/// Component to track the scan task
#[derive(Component)]
struct ImageScanTask(Task<Result<Vec<PathBuf>, String>>);

/// Initialize the application
fn setup(
    mut commands: Commands,
    mut loader: ResMut<ImageLoader>,
    config: Res<Config>,
    mut clear_color: ResMut<ClearColor>,
    mut fonts: ResMut<Assets<Font>>,
) {
    // Embed font at startup
    let font_data = include_bytes!("../assets/fonts/MPLUS2-VariableFont_wght.ttf");
    fonts.insert(
        &EMBEDDED_FONT_HANDLE,
        Font::try_from_bytes(font_data.to_vec()).expect("Failed to load embedded font"),
    );

    // Spawn camera with default 2D setup
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: bevy::render::camera::ClearColorConfig::Default,
            ..default()
        },
    ));

    // Set background color from config
    let bg = config.bg_color_f32();
    *clear_color = ClearColor(Color::srgba(bg[0], bg[1], bg[2], bg[3]));

    // Spawn image path text if enabled in config
    if config.style.show_image_path {
        info!("Spawning image path text UI");

        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ZIndex(1000),
        )).with_children(|parent| {
            parent.spawn((
                Text::new("Waiting for image..."),
                TextFont {
                    font: EMBEDDED_FONT_HANDLE,
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ImagePathText,
            ));
        });
    }

    // Configure image loader
    loader.cache_extent = config.viewer.cache_extent;
    loader.shuffle = config.viewer.shuffle;
}

/// Start the async image scan task on a background thread
fn start_image_scan(
    mut commands: Commands,
    config: Res<Config>,
    mut scan_state: ResMut<InitialScanState>,
    existing_task: Query<&ImageScanTask>,
) {
    // Only start if not already scanning and not scanned yet
    if scan_state.scanned || !existing_task.is_empty() {
        return;
    }

    // Delay scan by a few frames to ensuring window is rendered white/black first
    if scan_state.frame_count < 2 {
        scan_state.frame_count += 1;
        return;
    }

    info!("Starting async image scan...");

    let paths = config.viewer.image_paths.clone();
    let scan_subfolders = config.viewer.scan_subfolders;

    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        // This runs on a background thread!
        scan_image_paths(&paths, scan_subfolders).map_err(|e| e.to_string())
    });

    commands.spawn(ImageScanTask(task));
}

/// Poll the async scan task and update loader when complete
fn poll_image_scan(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut ImageScanTask)>,
    mut loader: ResMut<ImageLoader>,
    mut scan_state: ResMut<InitialScanState>,
    config: Res<Config>,
) {
    for (entity, mut task) in &mut tasks {
        // Non-blocking check
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            match result {
                Ok(paths) => {
                    info!("Image scan complete: {} images found", paths.len());
                    // Directly set paths instead of scanning again
                    loader.paths = paths; 

                    if config.viewer.shuffle {
                        loader.shuffle_paths();
                    }
                    
                    if loader.is_empty() {
                        warn!("No images found in configured paths.");
                    } else {
                        info!("Loaded {} images", loader.len());
                    }

                    scan_state.scanned = true;
                    commands.entity(entity).despawn();
                }
                Err(e) => {
                    error!("Image scan failed: {}", e);
                    scan_state.scanned = true;
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
/// Resource to track image changes
#[derive(Resource, Default)]
struct ImageChangeTracker {
    last_index: Option<usize>,
    previous_handle: Option<Handle<Image>>,
}

/// Resource to track keyboard repeat timing
#[derive(Resource)]
struct KeyRepeatTimer {
    /// Timer for key repeat interval
    repeat_timer: Timer,
    /// Timer for initial delay before repeat starts
    delay_timer: Timer,
    /// Whether we're currently in repeat mode
    in_repeat_mode: bool,
}

impl Default for KeyRepeatTimer {
    fn default() -> Self {
        Self {
            // 60ms interval = ~16 images per second (fast but controlled)
            repeat_timer: Timer::from_seconds(0.06, TimerMode::Repeating),
            // 1000ms delay before repeat starts
            delay_timer: Timer::from_seconds(1.0, TimerMode::Once),
            in_repeat_mode: false,
        }
    }
}

/// Component to mark transition entities
#[derive(Component)]
struct TransitionEntity;

/// Component to mark the image path text
#[derive(Component)]
struct ImagePathText;

/// Detect when the current image changes and trigger appropriate transitions
///
/// Monitors the image loader's current index and fires transition events when
/// the active image changes. The first image displays instantly without transition,
/// subsequent images use configured transition effects.
fn detect_image_change(
    loader: Res<ImageLoader>,
    mut tracker: ResMut<ImageChangeTracker>,
    mut transition_events: EventWriter<TransitionEvent>,
    config: Res<Config>,
    images: Res<Assets<Image>>,
    transition_state: Res<TransitionState>,
) {
    if loader.is_empty() {
        return;
    }

    let current_index = loader.current_index;
    let Some(current_handle) = loader.current_handle() else {
        return; // No handle yet, skip silently
    };

    // Check if image is loaded
    if images.get(current_handle).is_none() {
        return; // Image not loaded yet, skip silently
    }

    // Detect change
    if tracker.last_index != Some(current_index) {
        if let Some(prev_handle) = tracker.previous_handle.clone() {
            // Image changed, trigger transition
            let mode = if config.transition.random {
                transition::random_transition_mode()
            } else {
                config.transition.mode
            };

            // If there's an active transition, show new image instantly
            // to avoid visual artifacts when rapidly switching
            if transition_state.active.is_some() {
                // Transition is active - show new image instantly
                info!("Interrupting transition - showing image {} instantly", current_index + 1);
                transition_events.send(TransitionEvent {
                    from_image: current_handle.clone(),
                    to_image: current_handle.clone(),
                    duration: 0.0, // Instant display
                    mode,
                });
            } else {
                // No active transition - normal transition
                info!("Normal transition: image {} -> image {}",
                      tracker.last_index.unwrap_or(0) + 1, current_index + 1);
                transition_events.send(TransitionEvent {
                    from_image: prev_handle,
                    to_image: current_handle.clone(),
                    duration: config.transition.time,
                    mode,
                });
            }
        } else {
            // First image - show it instantly with zero-duration transition
            info!("First image loaded - showing instantly");
            let mode = if config.transition.random {
                transition::random_transition_mode()
            } else {
                config.transition.mode
            };

            transition_events.send(TransitionEvent {
                from_image: current_handle.clone(),
                to_image: current_handle.clone(),
                duration: 0.0, // Instant display
                mode,
            });
        }

        tracker.last_index = Some(current_index);
        tracker.previous_handle = Some(current_handle.clone());
    }
}

/// Create and spawn transition entities in response to transition events
///
/// Removes old transition entities and creates new fullscreen quads with
/// transition materials that blend between source and target images.
#[allow(clippy::too_many_arguments)]
fn trigger_transition(
    mut commands: Commands,
    mut transition_events: EventReader<TransitionEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TransitionMaterial>>,
    config: Res<Config>,
    windows: Query<&Window>,
    existing_entity: Query<(Entity, &MeshMaterial2d<TransitionMaterial>), With<TransitionEntity>>,
    images: Res<Assets<Image>>,
) {
    for event in transition_events.read() {
        // Check if both images are loaded before creating/updating entity
        if images.get(&event.from_image).is_none() || images.get(&event.to_image).is_none() {
            debug!("Skipping transition - images not loaded yet");
            continue;
        }

        // Get window size
        let window_size = if let Ok(window) = windows.get_single() {
            Vec2::new(window.width(), window.height())
        } else {
            Vec2::new(config.window.width as f32, config.window.height as f32)
        };

        // Get image dimensions for both textures
        let image_a_size = if let Some(img) = images.get(&event.from_image) {
            Vec2::new(img.width() as f32, img.height() as f32)
        } else {
            window_size // Fallback
        };

        let image_b_size = if let Some(img) = images.get(&event.to_image) {
            Vec2::new(img.width() as f32, img.height() as f32)
        } else {
            window_size // Fallback
        };

        let bg = config.bg_color_f32();

        // Try to reuse existing entity to avoid spawn/despawn overhead
        if let Ok((entity, material_handle)) = existing_entity.get_single() {
            // Reuse existing entity - just update the material
            if let Some(material) = materials.get_mut(&material_handle.0) {
                // Update textures
                material.texture_a = event.from_image.clone();
                material.texture_b = event.to_image.clone();

                // Set blend based on duration
                // For instant display (duration=0), use blend=1.0 to show texture_b immediately
                // For normal transition, use blend=0.0 to start from texture_a
                material.uniforms.blend = if event.duration == 0.0 { 1.0 } else { 0.0 };

                material.uniforms.mode = event.mode;
                material.uniforms.bg_color = Vec4::new(bg[0], bg[1], bg[2], bg[3]);
                material.uniforms.window_size = window_size;
                material.uniforms.image_a_size = image_a_size;
                material.uniforms.image_b_size = image_b_size;

                info!("Transition started (reused): mode {} duration {}s, entity: {:?}",
                      event.mode, event.duration, entity);
            }
        } else {
            // No existing entity - create new one
            // This is the first transition entity, created when the second image loads
            info!("Creating first transition entity - window_size: {:?}, image_a: {:?}, image_b: {:?}",
                  window_size, image_a_size, image_b_size);

            let mesh = meshes.add(Rectangle::new(window_size.x, window_size.y));

            // Check if this is a zero-duration transition (instant display)
            // For instant display: use blend=1.0 to show texture_b immediately
            // For normal transition: use blend=0.0 to start from texture_a
            let initial_blend = if event.duration == 0.0 { 1.0 } else { 0.0 };

            let material = materials.add(TransitionMaterial {
                uniforms: TransitionUniform {
                    blend: initial_blend,
                    mode: event.mode,
                    aspect_ratio: Vec2::new(1.0, 1.0),
                    bg_color: Vec4::new(bg[0], bg[1], bg[2], bg[3]),
                    window_size,
                    image_a_size,
                    image_b_size,
                },
                texture_a: event.from_image.clone(),
                texture_b: event.to_image.clone(),
            });

            let entity = commands.spawn((
                Mesh2d(mesh),
                MeshMaterial2d(material),
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::default(),
                Visibility::Visible, // Show immediately
                InheritedVisibility::default(),
                ViewVisibility::default(),
                TransitionEntity,
            ));


            info!("Transition started (new): mode {} duration {}s, entity: {:?}",
                  event.mode, event.duration, entity.id());
        }
    }
}

/// Handle keyboard and mouse input for navigation and control
#[allow(clippy::too_many_arguments)]
fn keyboard_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<MouseWheel>,
    mut exit: EventWriter<AppExit>,
    mut loader: ResMut<ImageLoader>,
    mut timer: ResMut<SlideshowTimer>,
    mut repeat_timer: ResMut<KeyRepeatTimer>,
    config: Res<Config>,
    time: Res<Time>,
    mut windows: Query<&mut Window>,
) {
    // ESC or Q to quit
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyQ) {
        info!("Exiting application");
        exit.send(AppExit::Success);
    }

    // Check if any navigation keys are being held
    let nav_keys_held = keys.pressed(KeyCode::ArrowRight)
        || keys.pressed(KeyCode::Space)
        || keys.pressed(KeyCode::ArrowLeft);

    // Update repeat timers based on key state
    if nav_keys_held {
        // Key is held - update timers
        if !repeat_timer.in_repeat_mode {
            // Not in repeat mode yet - tick delay timer
            repeat_timer.delay_timer.tick(time.delta());
            if repeat_timer.delay_timer.just_finished() {
                // Delay finished - enter repeat mode
                repeat_timer.in_repeat_mode = true;
                repeat_timer.repeat_timer.reset();
            }
        } else {
            // In repeat mode - tick repeat timer
            repeat_timer.repeat_timer.tick(time.delta());
        }
    } else {
        // No key held - reset timers
        if repeat_timer.in_repeat_mode || !repeat_timer.delay_timer.finished() {
            repeat_timer.delay_timer.reset();
            repeat_timer.repeat_timer.reset();
            repeat_timer.in_repeat_mode = false;
        }
    }

    // Right arrow or Space: next image (supports key hold with delay)
    let should_advance_next = keys.just_pressed(KeyCode::ArrowRight)
        || keys.just_pressed(KeyCode::Space)
        || (repeat_timer.in_repeat_mode
            && (keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::Space))
            && repeat_timer.repeat_timer.just_finished());

    if should_advance_next
        && loader.next(config.viewer.pause_at_last) {
            info!("Next image ({}/{})", loader.current_index + 1, loader.len());
            timer.reset(); // Reset timer when manually advancing
        }

    // Left arrow: previous image (supports key hold with delay)
    let should_advance_prev = keys.just_pressed(KeyCode::ArrowLeft)
        || (repeat_timer.in_repeat_mode
            && keys.pressed(KeyCode::ArrowLeft)
            && repeat_timer.repeat_timer.just_finished());

    if should_advance_prev
        && loader.previous() {
            info!("Previous image ({}/{})", loader.current_index + 1, loader.len());
            timer.reset(); // Reset timer when manually advancing
        }

    // Home: first image
    if keys.just_pressed(KeyCode::Home) {
        loader.current_index = 0;
        info!("First image");
        timer.reset();
    }

    // End: last image
    if keys.just_pressed(KeyCode::End)
        && !loader.is_empty() {
            loader.current_index = loader.len() - 1;
            info!("Last image");
            timer.reset();
        }

    // P: toggle pause
    if keys.just_pressed(KeyCode::KeyP) {
        timer.toggle_pause();
        if timer.paused {
            info!("Slideshow paused");
        } else {
            info!("Slideshow resumed");
        }
    }

    // F: toggle fullscreen
    if keys.just_pressed(KeyCode::KeyF) {
        if let Ok(mut window) = windows.get_single_mut() {
            match window.mode {
                WindowMode::Windowed => {
                    window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Index(config.window.monitor_index));
                    info!("Fullscreen enabled");
                }
                _ => {
                    window.mode = WindowMode::Windowed;
                    info!("Fullscreen disabled");
                }
            }
        }
    }

    // Mouse left click: next image
    if mouse_buttons.just_pressed(MouseButton::Left)
        && loader.next(config.viewer.pause_at_last) {
            info!("Next image ({}/{})", loader.current_index + 1, loader.len());
            timer.reset();
        }

    // Mouse right click: previous image
    if mouse_buttons.just_pressed(MouseButton::Right)
        && loader.previous() {
            info!("Previous image ({}/{})", loader.current_index + 1, loader.len());
            timer.reset();
        }

    // Mouse wheel: scroll through images
    for event in mouse_wheel.read() {
        if event.y > 0.0 {
            // Scroll up: previous image
            if loader.previous() {
                info!("Previous image ({}/{})", loader.current_index + 1, loader.len());
                timer.reset();
            }
        } else if event.y < 0.0 {
            // Scroll down: next image
            if loader.next(config.viewer.pause_at_last) {
                info!("Next image ({}/{})", loader.current_index + 1, loader.len());
                timer.reset();
            }
        }
    }
}

/// Handle automatic slideshow advancement based on timer events
fn handle_slideshow_advance(
    mut events: EventReader<SlideshowAdvanceEvent>,
    mut loader: ResMut<ImageLoader>,
    config: Res<Config>,
) {
    for _ in events.read() {
        if loader.next(config.viewer.pause_at_last) {
            info!("Auto-advance: Next image ({}/{})", loader.current_index + 1, loader.len());
        }
    }
}

/// Update the image path text display
fn update_image_path_text(
    loader: Res<ImageLoader>,
    config: Res<Config>,
    mut text_query: Query<&mut Text, With<ImagePathText>>,
) {
    if !config.style.show_image_path {
        return;
    }

    for mut text in text_query.iter_mut() {
        if let Some(path) = loader.current_path() {
            let path_string = path.display().to_string().replace('\\', "/");
            *text = Text::new(path_string);
        } else {
            *text = Text::new("Waiting for image...");
        }
    }
}

/// Update transition mesh and material when window is resized
fn update_transition_on_resize(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TransitionMaterial>>,
    windows: Query<&Window, Changed<Window>>,
    mut transition_query: Query<(&mut Mesh2d, &MeshMaterial2d<TransitionMaterial>), With<TransitionEntity>>,
    images: Res<Assets<Image>>,
) {
    // Check if window size changed
    if let Ok(window) = windows.get_single() {
        let new_size = Vec2::new(window.width(), window.height());

        // Update transition entity mesh and material
        if let Ok((mut mesh_handle, material_handle)) = transition_query.get_single_mut() {
            // Update mesh to new window size
            let new_mesh = meshes.add(Rectangle::new(new_size.x, new_size.y));
            *mesh_handle = Mesh2d(new_mesh);

            // Update material uniforms with new window size
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.uniforms.window_size = new_size;

                // Recalculate image sizes for letterboxing
                if let Some(img_a) = images.get(&material.texture_a) {
                    material.uniforms.image_a_size = Vec2::new(img_a.width() as f32, img_a.height() as f32);
                }
                if let Some(img_b) = images.get(&material.texture_b) {
                    material.uniforms.image_b_size = Vec2::new(img_b.width() as f32, img_b.height() as f32);
                }
            }
        }
    }
}
