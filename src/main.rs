//! sldshow2 - Simple slideshow image viewer with custom WGSL transitions
//!
//! A Bevy-based slideshow application featuring 22 different transition effects,
//! reactive rendering for power efficiency, and flexible TOML configuration.

mod config;
mod consts;
mod diagnostics;
mod error;
mod image_loader;
mod metadata;
mod slideshow;
mod transition;
mod watcher;

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{MouseButton, MouseWheel};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{InstanceFlags, RenderCreation, WgpuSettings};
use bevy::sprite_render::MeshMaterial2d;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::window::{MonitorSelection, PresentMode, WindowMode};
use bevy::winit::WinitSettings;
use camino::Utf8PathBuf;
use config::Config;
use consts::EMBEDDED_FONT_HANDLE;
use image_loader::{
    ImageLoader, ImageLoaderPlugin, load_images_system_inner, load_images_system_readonly,
    poll_loading_tasks, scan_image_paths,
};
use slideshow::{SlideshowAdvanceEvent, SlideshowPlugin, SlideshowTimer};
use transition::{
    TransitionEntity, TransitionEvent, TransitionMaterial, TransitionPlugin, TransitionState,
    TransitionUniform,
};
use watcher::{FileWatcher, poll_file_watcher_system};

/// Main entry point for sldshow2 application
fn main() {
    // Initialize structured logging with environment filter
    // Use RUST_LOG environment variable to control log level (e.g., RUST_LOG=sldshow2=debug)
    use tracing_subscriber::{EnvFilter, fmt};
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sldshow2=info")),
        )
        .init();

    // Load configuration
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(Utf8PathBuf::from);

    let config = Config::load_default(config_path).unwrap_or_else(|e| {
        error!("Failed to load config: {}", e);
        warn!("Using default configuration");
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
        // Start in Continuous mode for smooth startup loading pipeline
        // optimize_power_mode will switch to Reactive when idle
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
                        resolution: (config.window.width as u32, config.window.height as u32)
                            .into(),
                        present_mode: PresentMode::AutoVsync,
                        decorations: config.window.decorations,
                        resizable: config.window.resizable,
                        mode: window_mode,
                        ..default()
                    }),
                    ..default()
                })
                // Disable wgpu GPU validation in debug builds.
                // Bevy enables validation by default in debug mode, which adds
                // 20-30ms per frame and 200-400ms spikes on material/texture changes.
                // Release builds already have validation disabled.
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        instance_flags: InstanceFlags::empty(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(ImageLoaderPlugin)
        .add_plugins(SlideshowPlugin)
        .add_plugins(TransitionPlugin)
        .add_plugins(diagnostics::DiagnosticsPlugin)
        .add_systems(Startup, (setup, prewarm_transition_shader.after(setup)))
        // Transition pipeline - strict ordering to prevent inter-frame delays:
        // detect_image_change writes TransitionEvent →
        // handle_transition_events sets state.active →
        // trigger_transition creates/updates material (MUST be same frame!) →
        // update_transitions animates blend value
        //
        // CRITICAL: All transition systems MUST be in a single ordered chain.
        // Previously, handle_transition_events and trigger_transition were in
        // separate groups with no ordering, allowing load_images_system to run
        // between them. This caused 200-900ms delays because the render phase
        // (GPU texture upload + shader work) would complete between groups.
        .add_systems(
            Update,
            (
                start_image_scan,
                poll_image_scan.after(start_image_scan),
                keyboard_input_system,
                handle_slideshow_advance,
                detect_image_change.after(keyboard_input_system),
                transition::handle_transition_events.after(detect_image_change),
                transition::trigger_transition.after(transition::handle_transition_events),
                transition::update_transitions.after(transition::trigger_transition),
                update_transition_on_resize,
            ),
        )
        // Image loading system - runs AFTER entire transition chain to avoid
        // inserting GPU uploads between transition event handling and rendering
        .add_systems(
            Update,
            load_images_system
                .run_if(|loader: Res<ImageLoader>| !loader.is_empty())
                .after(transition::update_transitions),
        )
        .add_systems(Update, update_image_path_text)
        .add_systems(Update, poll_file_watcher_system)
        .add_systems(Update, optimize_power_mode)
        .run();
}

// Font handle is now defined in consts module

/// Resource to track initial scan state
#[derive(Resource, Default)]
struct InitialScanState {
    scanned: bool,
    frame_count: u32,
}

/// Component to track the scan task
#[derive(Component)]
struct ImageScanTask(Task<Result<Vec<Utf8PathBuf>, String>>);

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
    fonts
        .insert(
            &EMBEDDED_FONT_HANDLE,
            Font::try_from_bytes(font_data.to_vec()).expect("Failed to load embedded font"),
        )
        .expect("Failed to insert embedded font");

    // Spawn camera with default 2D setup
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Default,
            ..default()
        },
    ));

    // Set background color from config
    let bg = config.bg_color_f32();
    *clear_color = ClearColor(Color::srgba(bg[0], bg[1], bg[2], bg[3]));

    // Spawn image path text if enabled in config
    if config.style.show_image_path {
        info!("Spawning image path text UI");

        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    left: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                ZIndex(1000),
            ))
            .with_children(|parent| {
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

    // Set max texture size from config
    // [0, 0] means use window dimensions (may cause frame spikes at 4K+)
    let (max_w, max_h) = if config.viewer.max_texture_size == [0, 0] {
        (config.window.width, config.window.height)
    } else {
        (
            config.viewer.max_texture_size[0],
            config.viewer.max_texture_size[1],
        )
    };
    loader.set_max_texture_size(max_w, max_h);
    info!("Max texture size set to {}x{}", max_w, max_h);
}

/// Pre-warm the transition shader pipeline during startup.
///
/// Creates a TransitionEntity with a 1x1 placeholder texture so that the GPU
/// shader pipeline is compiled during the loading screen (where the user expects
/// a brief delay) instead of on the first image transition (where it causes a
/// 1-2 second visible freeze).
fn prewarm_transition_shader(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TransitionMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut transition_state: ResMut<TransitionState>,
    config: Res<Config>,
) {
    // Create a 1x1 black placeholder image
    let placeholder = Image::new(
        bevy::render::render_resource::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        vec![0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    let placeholder_handle = images.add(placeholder);

    let window_size = Vec2::new(config.window.width as f32, config.window.height as f32);
    let mesh = meshes.add(Rectangle::new(window_size.x, window_size.y));
    let bg = config.bg_color_f32();

    let material_handle = materials.add(TransitionMaterial {
        uniforms: TransitionUniform {
            blend: 1.0, // Show texture_b (all black = invisible on black bg)
            mode: 0,
            aspect_ratio: Vec2::ONE,
            bg_color: Vec4::new(bg[0], bg[1], bg[2], bg[3]),
            window_size,
            image_a_size: Vec2::ONE,
            image_b_size: Vec2::ONE,
        },
        texture_a: placeholder_handle.clone(),
        texture_b: placeholder_handle,
    });

    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material_handle.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
        TransitionEntity,
    ));

    // Store material handle so trigger_transition can find and reuse the entity
    transition_state.material_handle = Some(material_handle);
    info!("Transition shader pre-warmed (entity created at startup)");
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
        // Non-blocking check - only process if task is finished
        if task.0.is_finished() {
            // Task is done, extract result (block_on is instant for finished tasks)
            let result = bevy::tasks::block_on(&mut task.0);
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

                    // Initialize hot-reload file watcher
                    if config.viewer.hot_reload {
                        match FileWatcher::new(
                            config.viewer.image_paths.clone(),
                            config.viewer.scan_subfolders,
                        ) {
                            Ok(watcher) => {
                                info!(
                                    "Hot-reload enabled for {} directories",
                                    watcher.watched_paths().len()
                                );
                                commands.insert_resource(watcher);
                            }
                            Err(e) => {
                                warn!("Failed to initialize hot-reload: {}", e);
                            }
                        }
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
    /// Time of the last image change (for detecting rapid navigation)
    last_change_time: Option<f32>,
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

/// Component to mark the image path text
#[derive(Component)]
struct ImagePathText;

/// Detect when the current image changes and trigger appropriate transitions
///
/// Monitors the image loader's current index and fires transition events when
/// the active image changes. The first image displays instantly without transition,
/// subsequent images use configured transition effects.
#[allow(clippy::too_many_arguments)]
fn detect_image_change(
    loader: Res<ImageLoader>,
    mut tracker: ResMut<ImageChangeTracker>,
    mut transition_events: MessageWriter<TransitionEvent>,
    config: Res<Config>,
    images: Res<Assets<Image>>,
    mut transition_state: ResMut<TransitionState>,
    mut metrics: ResMut<diagnostics::TransitionMetrics>,
    time: Res<Time>,
) {
    // Early returns for invalid states
    if loader.is_empty() {
        return;
    }

    let current_index = loader.current_index;
    let Some(current_handle) = loader.current_handle() else {
        return;
    };

    // Ensure current image is fully loaded before triggering transition
    if images.get(current_handle).is_none() {
        return;
    }

    // No change detected
    if tracker.last_index == Some(current_index) {
        return;
    }

    // Handle rapid switching with debouncing
    if let Some(ref transition) = transition_state.active {
        let elapsed = time.elapsed_secs() - transition.start_time;
        let min_transition_time = 0.1; // Allow at least 100ms before cancelling

        // Only cancel if transition has been running for at least min_transition_time
        // This prevents excessive cancellations during normal usage
        if elapsed >= min_transition_time {
            // Rapid switching detected - cancel current transition
            diagnostics::track_transition_cancel(&mut metrics);
            info!(
                "Transition cancelled after {:.3}s due to rapid switching",
                elapsed
            );

            // Update displayed_image to current transition target immediately
            transition_state.displayed_image = Some(transition.to_image.clone());
            transition_state.active = None;
            // Fall through to start new transition below
        } else {
            // Transition just started - let it run a bit before allowing cancellation
            // Track the pending change but don't start new transition yet
            return;
        }
    }

    let mode = if config.transition.random {
        transition::random_transition_mode()
    } else {
        config.transition.mode
    };

    let current_handle = current_handle.clone();

    // Handle first image (instant display)
    let Some(prev_index) = tracker.last_index else {
        info!("First image loaded - showing instantly");
        transition_events.write(TransitionEvent {
            from_image: current_handle.clone(),
            to_image: current_handle,
            duration: 0.0,
            mode,
        });
        tracker.last_index = Some(current_index);
        return;
    };

    // Handle normal transition
    let Some(prev_handle) = loader.handles.get(&prev_index) else {
        return;
    };

    // Detect rapid navigation: if navigating faster than transition duration,
    // skip animation and show image instantly for responsive browsing
    let now = time.elapsed_secs();
    let rapid_navigation = tracker
        .last_change_time
        .is_some_and(|t| now - t < config.transition.time);
    let duration = if rapid_navigation {
        0.0
    } else {
        config.transition.time
    };

    if rapid_navigation {
        debug!(
            "Rapid navigation: instant display image {} -> image {}",
            prev_index + 1,
            current_index + 1
        );
    } else {
        info!(
            "Normal transition: image {} -> image {}",
            prev_index + 1,
            current_index + 1
        );
    }
    transition_events.write(TransitionEvent {
        from_image: prev_handle.clone(),
        to_image: current_handle,
        duration,
        mode,
    });

    tracker.last_index = Some(current_index);
    tracker.last_change_time = Some(now);
}

/// Handle keyboard and mouse input for navigation and control
#[allow(clippy::too_many_arguments)]
fn keyboard_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut exit: MessageWriter<AppExit>,
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
        exit.write(AppExit::Success);
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
        if repeat_timer.in_repeat_mode || !repeat_timer.delay_timer.is_finished() {
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

    if should_advance_next && loader.next(config.viewer.pause_at_last) {
        info!("Next image ({}/{})", loader.current_index + 1, loader.len());
        timer.reset(); // Reset timer when manually advancing
    }

    // Left arrow: previous image (supports key hold with delay)
    let should_advance_prev = keys.just_pressed(KeyCode::ArrowLeft)
        || (repeat_timer.in_repeat_mode
            && keys.pressed(KeyCode::ArrowLeft)
            && repeat_timer.repeat_timer.just_finished());

    if should_advance_prev && loader.previous() {
        info!(
            "Previous image ({}/{})",
            loader.current_index + 1,
            loader.len()
        );
        timer.reset(); // Reset timer when manually advancing
    }

    // Home: first image
    if keys.just_pressed(KeyCode::Home) {
        loader.current_index = 0;
        info!("First image");
        timer.reset();
    }

    // End: last image
    if keys.just_pressed(KeyCode::End) && !loader.is_empty() {
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
        if let Ok(mut window) = windows.single_mut() {
            match window.mode {
                WindowMode::Windowed => {
                    window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Index(
                        config.window.monitor_index,
                    ));
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
    if mouse_buttons.just_pressed(MouseButton::Left) && loader.next(config.viewer.pause_at_last) {
        info!("Next image ({}/{})", loader.current_index + 1, loader.len());
        timer.reset();
    }

    // Mouse right click: previous image
    if mouse_buttons.just_pressed(MouseButton::Right) && loader.previous() {
        info!(
            "Previous image ({}/{})",
            loader.current_index + 1,
            loader.len()
        );
        timer.reset();
    }

    // Mouse wheel: scroll through images
    for event in mouse_wheel.read() {
        if event.y > 0.0 {
            // Scroll up: previous image
            if loader.previous() {
                info!(
                    "Previous image ({}/{})",
                    loader.current_index + 1,
                    loader.len()
                );
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
///
/// Only advances if the next image is already loaded to prevent stuttering.
/// If next image isn't ready, the advance is skipped and will retry on next timer tick.
fn handle_slideshow_advance(
    mut events: MessageReader<SlideshowAdvanceEvent>,
    mut loader: ResMut<ImageLoader>,
    config: Res<Config>,
    images: Res<Assets<Image>>,
) {
    for _ in events.read() {
        // Check if next image is ready before advancing
        let next_index = if config.viewer.pause_at_last
            && loader.current_index >= loader.len().saturating_sub(1)
        {
            loader.current_index // Would stay at last
        } else {
            (loader.current_index + 1) % loader.len()
        };

        // Only advance if next image is fully loaded (prevents stutter)
        if let Some(next_handle) = loader.handles.get(&next_index) {
            if images.get(next_handle).is_none() {
                debug!(
                    "Auto-advance skipped: image {} not ready yet",
                    next_index + 1
                );
                continue;
            }
        } else {
            debug!(
                "Auto-advance skipped: image {} not loaded yet",
                next_index + 1
            );
            continue;
        }

        if loader.next(config.viewer.pause_at_last) {
            info!(
                "Auto-advance: Next image ({}/{})",
                loader.current_index + 1,
                loader.len()
            );
        }
    }
}

/// Update the image path text display
fn update_image_path_text(
    loader: Res<ImageLoader>,
    config: Res<Config>,
    mut text_query: Query<&mut Text, With<ImagePathText>>,
    mut last_displayed_index: Local<Option<usize>>,
) {
    if !config.style.show_image_path {
        return;
    }

    // Determine which image to display the path for:
    // - During transition: show the target image (to_image) path
    // - Otherwise: show the currently displayed image
    let display_index = loader.current_index;

    // Check if we need to update:
    // 1. Index changed, OR
    // 2. First time the current image handle becomes available
    let current_handle_ready = loader.handles.contains_key(&display_index);
    let should_update = match *last_displayed_index {
        None => current_handle_ready, // First update when image is ready
        Some(last) => last != display_index && current_handle_ready,
    };

    if !should_update {
        return;
    }

    *last_displayed_index = Some(display_index);

    for mut text in text_query.iter_mut() {
        if let Some(path) = loader.paths.get(display_index) {
            let path_string = path.as_str().replace('\\', "/");

            // Try to get metadata and append if available (may be empty if not loaded yet)
            let metadata = loader.current_metadata();
            let summary = metadata.and_then(|m| {
                let s = m.summary();
                if s.is_empty() { None } else { Some(s) }
            });

            let display_text = if let Some(meta_summary) = summary {
                format!("{}\n{}", path_string, meta_summary)
            } else {
                path_string
            };

            *text = Text::new(display_text);
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
    mut transition_query: Query<
        (&mut Mesh2d, &MeshMaterial2d<TransitionMaterial>),
        With<TransitionEntity>,
    >,
    images: Res<Assets<Image>>,
) {
    // Check if window size changed
    if let Ok(window) = windows.single() {
        let new_size = Vec2::new(window.width(), window.height());
        // Note: max_texture_size is NOT updated on resize - it's controlled by config
        // to prevent 4K+ windows from causing large GPU uploads

        // Update transition entity mesh and material
        if let Ok((mut mesh_handle, material_handle)) = transition_query.single_mut() {
            // Update mesh to new window size
            let new_mesh = meshes.add(Rectangle::new(new_size.x, new_size.y));
            *mesh_handle = Mesh2d(new_mesh);

            // Update material uniforms with new window size
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.uniforms.window_size = new_size;

                // Recalculate image sizes for letterboxing
                if let Some(img_a) = images.get(&material.texture_a) {
                    material.uniforms.image_a_size =
                        Vec2::new(img_a.width() as f32, img_a.height() as f32);
                }
                if let Some(img_b) = images.get(&material.texture_b) {
                    material.uniforms.image_b_size =
                        Vec2::new(img_b.width() as f32, img_b.height() as f32);
                }
            }
        }
    }
}

/// Dynamically adjust power mode based on activity
/// - Continuous: During transitions or active slideshow playback (smooth animation)
/// - Reactive: When paused and idle (save GPU/CPU power)
fn optimize_power_mode(
    mut winit_settings: ResMut<WinitSettings>,
    transition_state: Res<TransitionState>,
    slideshow_timer: Res<SlideshowTimer>,
    loader: Res<ImageLoader>,
) {
    let is_transitioning = transition_state.active.is_some();
    let is_playing = !slideshow_timer.paused && slideshow_timer.interval > 0.0;
    // Keep Continuous while preloading to avoid delaying GPU uploads and task polling
    let has_pending_work = !loader.pending_uploads.is_empty() || !loader.loading_tasks.is_empty();

    if is_transitioning || is_playing || has_pending_work {
        // Need smooth animation, accurate timer firing, or pending image work
        winit_settings.focused_mode = bevy::winit::UpdateMode::Continuous;
    } else {
        // Fully idle: no transitions, not playing, no pending loads
        // Wait for input events instead of continuous rendering
        winit_settings.focused_mode = bevy::winit::UpdateMode::reactive(std::time::Duration::MAX);
    }

    // Always save power when unfocused
    winit_settings.unfocused_mode = bevy::winit::UpdateMode::reactive(std::time::Duration::MAX);
}

/// Wrapper system for image loading that checks transition state
///
/// During active transitions, only the current image is uploaded to the GPU.
/// Preload images are deferred until the transition completes to prevent
/// frame spikes during animations.
///
/// Three-phase approach to minimize Bevy change detection:
/// 1. Poll completed tasks → pending_uploads (no Assets<Image> mutation)
/// 2. Determine if GPU uploads will actually happen
/// 3. Only trigger DerefMut on Assets<Image> when uploading
fn load_images_system(
    mut loader: ResMut<ImageLoader>,
    mut images: ResMut<Assets<Image>>,
    transition_state: Res<TransitionState>,
) {
    if loader.paths.is_empty() {
        return;
    }

    let transition_active = transition_state.active.is_some();

    // Phase 1: Poll completed tasks (only modifies loader, not images)
    poll_loading_tasks(&mut loader);

    // Phase 2: Determine if we'll actually call images.add() this frame.
    // Only trigger Assets<Image> change detection when we will upload.
    let current_index = loader.current_index;
    let current_in_pending = loader
        .pending_uploads
        .iter()
        .any(|(idx, _)| *idx == current_index);
    let current_needs_upload = !loader.handles.contains_key(&current_index) && current_in_pending;

    let will_upload = if loader.pending_uploads.is_empty() {
        false
    } else if current_needs_upload || !loader.initial_load_complete {
        true
    } else {
        // Preload uploads only when idle (no transition active)
        !transition_active
    };

    if will_upload {
        // DerefMut: triggers change detection (upload path)
        load_images_system_inner(&mut loader, &mut images, transition_active);
    } else {
        // Deref only: no change detection triggered (read-only path)
        load_images_system_readonly(&mut loader, &images, transition_active);
    }
}
