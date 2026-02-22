#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Application entry point.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use log::{error, warn};
use std::sync::Arc;
use winit::event_loop::EventLoop;

mod app;
mod clipboard;
mod config;
mod drag_drop;
mod error;
mod image_loader;
mod input;
mod osc;
mod overlay;
mod screenshot;
mod thumbnail;
mod timer;
mod transition;

use app::ApplicationState;
use config::Config;
use drag_drop::DragDropHandler;

fn main() -> Result<()> {
    env_logger::init();

    // Prevent screen saver
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Power::{
            ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        // Prevents sleep and screen saver
        SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
    }

    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(Utf8PathBuf::from);
    let config = Config::load_default(config_path).unwrap_or_else(|e| {
        error!("Failed to load config: {}", e);
        warn!("Using default configuration");
        Config::default()
    });

    // Set up drag-and-drop channel and event loop with message hook
    let (drag_drop, drag_drop_tx) = DragDropHandler::new();

    let event_loop = {
        #[cfg(windows)]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            EventLoop::builder()
                .with_msg_hook(drag_drop::build_msg_hook(drag_drop_tx))
                .build()
                .context("Failed to create event loop")?
        }
        #[cfg(not(windows))]
        {
            drop(drag_drop_tx); // suppress unused-variable on non-Windows
            EventLoop::new()
                .context("Failed to create event loop — is a display server running?")?
        }
    };

    let transparent = config.style.bg_color[3] < 255;
    let fullscreen = config
        .window
        .fullscreen
        .then_some(winit::window::Fullscreen::Borderless(None));
    let window_attributes = winit::window::Window::default_attributes()
        .with_title("sldshow2")
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.window.width,
            config.window.height,
        ))
        .with_decorations(config.window.decorations)
        .with_resizable(config.window.resizable)
        .with_transparent(transparent)
        .with_window_level(if config.window.always_on_top {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        })
        .with_fullscreen(fullscreen);

    #[allow(deprecated)]
    let window = Arc::new(
        event_loop
            .create_window(window_attributes)
            .context("Failed to create window")?,
    );

    // Replace winit's OLE drag-and-drop with WM_DROPFILES
    #[cfg(windows)]
    drag_drop::enable_wm_dropfiles(&window);

    // Initialize WGPU state
    let mut state = pollster::block_on(ApplicationState::new(
        window.clone(),
        config.clone(),
        drag_drop,
    ))?;

    event_loop
        .run_app(&mut state)
        .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
}
