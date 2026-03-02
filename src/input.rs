//! Input handling and event processing.
//!
//! This module isolates all keyboard, mouse, and cursor input logic from the main event loop.

use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::Window,
};

use crate::overlay::OverlayKind;

/// Actions that can be triggered by input events.
#[derive(Debug, Clone)]
pub enum InputAction {
    NextImage {
        steps: usize,
    },
    PrevImage {
        steps: usize,
    },
    JumpTo(usize),
    TogglePause,
    ToggleFullscreen,
    SetFullscreen(bool),
    ToggleDecorations,
    ToggleAlwaysOnTop,
    AdjustTimer(f32),
    ResetTimer,
    Screenshot,
    ColorAdjust {
        key: KeyCode,
    },
    ResetColorAdjustments,
    ToggleInfoOverlay,
    ShowInfoTemporary,
    ToggleFilenameDisplay,
    ShowFilenameTemporary,
    ToggleLoop,
    ToggleFitMode,
    SetWindowPosition {
        x: i32,
        y: i32,
    },
    CopyImageToClipboard,
    ToggleHelpOverlay,
    ToggleSettings,
    ToggleGallery,
    Exit,
    ResizeWindow {
        width: u32,
        height: u32,
    },
    CopyPathToClipboard,
    OpenInExplorer,
    /// Zoom in (delta > 0) or out (delta < 0). Delta is a multiplicative scroll step.
    Zoom {
        delta: f32,
    },
    /// Pan the image by (dx, dy) in physical pixels.
    Pan {
        dx: f32,
        dy: f32,
    },
    /// Reset zoom and pan to defaults.
    ResetZoom,
}

/// Application context passed to the input handler for context-aware keyboard actions.
pub struct InputContext {
    pub fullscreen: bool,
    pub image_count: usize,
    /// The topmost overlay by open order (z-order proxy). Used to determine
    /// which overlay Escape should close first.
    pub front_overlay: Option<OverlayKind>,
    /// Size of the currently displayed image in pixels, if any.
    pub current_image_size: Option<(u32, u32)>,
}

/// Input state tracker.
///
/// Maintains cursor position, drag state, click timing, and other input-specific state.
pub struct InputHandler {
    pub last_cursor_move: Instant,
    pub cursor_visible: bool,
    last_click_time: Option<Instant>,
    drag_start_cursor: Option<PhysicalPosition<f64>>,
    is_dragging: bool,
    ignore_next_release: bool,
    cursor_pos: Option<PhysicalPosition<f64>>,
    /// Current zoom level (1.0 = no zoom). Kept here so drag behavior can check it.
    pub zoom_scale: f32,
}

impl InputHandler {
    /// Creates a new input handler with default state.
    pub fn new() -> Self {
        Self {
            last_cursor_move: Instant::now(),
            cursor_visible: true,
            last_click_time: None,
            drag_start_cursor: None,
            is_dragging: false,
            ignore_next_release: false,
            cursor_pos: None,
            zoom_scale: 1.0,
        }
    }

    /// Reset zoom state (called on image navigation).
    pub fn reset_zoom(&mut self) {
        self.zoom_scale = 1.0;
    }

    /// Handles a window event and returns (consumed, optional_action).
    pub fn handle_event(
        &mut self,
        event: &WindowEvent,
        modifiers: &ModifiersState,
        window: &Window,
        ctx: &InputContext,
    ) -> (bool, Option<InputAction>) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position, window, ctx.fullscreen)
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => self.handle_mouse_pressed(*button, ctx.fullscreen),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.handle_mouse_released(),
            WindowEvent::MouseWheel { delta, .. } => self.handle_mouse_wheel(delta, modifiers),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => {
                self.handle_keyboard_pressed(&key_event.physical_key, modifiers, ctx)
            }
            _ => (false, None),
        }
    }

    fn handle_cursor_moved(
        &mut self,
        position: &PhysicalPosition<f64>,
        window: &Window,
        fullscreen: bool,
    ) -> (bool, Option<InputAction>) {
        self.last_cursor_move = Instant::now();

        // Note: cursor visibility is set in the handler, caller should check cursor_visible field
        if !self.cursor_visible {
            self.cursor_visible = true;
            // Window.set_cursor_visible will be called by ApplicationState
        }

        // Calculate screen position for drag tracking
        let Some(client_origin) = window.inner_position().ok() else {
            self.cursor_pos = Some(PhysicalPosition::new(position.x, position.y));
            return (false, None);
        };

        let screen_pos_x = client_origin.x as f64 + position.x;
        let screen_pos_y = client_origin.y as f64 + position.y;
        let screen_pos = PhysicalPosition::new(screen_pos_x, screen_pos_y);

        if let Some(start_pos) = self.drag_start_cursor {
            let dx = screen_pos.x - start_pos.x;
            let dy = screen_pos.y - start_pos.y;
            let dist_sq = dx * dx + dy * dy;

            if !self.is_dragging && dist_sq > 25.0 {
                self.is_dragging = true;
            }

            if self.is_dragging {
                // When zoomed, drag pans the image instead of moving the window
                if self.zoom_scale > 1.0 {
                    self.drag_start_cursor = Some(screen_pos);
                    return (
                        true,
                        Some(InputAction::Pan {
                            dx: dx as f32,
                            dy: dy as f32,
                        }),
                    );
                }

                if fullscreen {
                    self.drag_start_cursor = Some(screen_pos);
                    return (true, Some(InputAction::SetFullscreen(false)));
                }

                if let Ok(outer_pos) = window.outer_position() {
                    let new_x = outer_pos.x + dx as i32;
                    let new_y = outer_pos.y + dy as i32;
                    self.drag_start_cursor = Some(screen_pos);
                    return (
                        false,
                        Some(InputAction::SetWindowPosition { x: new_x, y: new_y }),
                    );
                }
            }
        }

        self.cursor_pos = Some(screen_pos);
        (false, None)
    }

    fn handle_mouse_pressed(
        &mut self,
        button: MouseButton,
        _fullscreen: bool,
    ) -> (bool, Option<InputAction>) {
        self.last_cursor_move = Instant::now();

        match button {
            MouseButton::Left => {
                let now = Instant::now();
                if let Some(last) = self.last_click_time {
                    if now.duration_since(last).as_millis() < 300 {
                        self.last_click_time = None;
                        self.ignore_next_release = true;
                        return (true, Some(InputAction::ToggleFullscreen));
                    }
                }
                self.last_click_time = Some(now);

                if let Some(pos) = self.cursor_pos {
                    self.drag_start_cursor = Some(pos);
                }
                self.is_dragging = false;
                self.ignore_next_release = false;

                (true, None)
            }
            MouseButton::Right => (true, Some(InputAction::PrevImage { steps: 1 })),
            _ => (false, None),
        }
    }

    fn handle_mouse_released(&mut self) -> (bool, Option<InputAction>) {
        self.drag_start_cursor = None;
        if self.is_dragging {
            self.is_dragging = false;
            (true, None)
        } else if !self.ignore_next_release {
            (true, Some(InputAction::NextImage { steps: 1 }))
        } else {
            (true, None)
        }
    }

    fn handle_mouse_wheel(
        &mut self,
        delta: &winit::event::MouseScrollDelta,
        modifiers: &ModifiersState,
    ) -> (bool, Option<InputAction>) {
        self.last_cursor_move = Instant::now();

        // Ctrl+Scroll → zoom; plain Scroll → image navigation
        if modifiers.control_key() {
            let y = match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
            };
            if y != 0.0 {
                return (true, Some(InputAction::Zoom { delta: y }));
            }
            return (false, None);
        }

        let steps = if modifiers.shift_key() { 10 } else { 1 };

        match delta {
            winit::event::MouseScrollDelta::LineDelta(_, y) => {
                if *y > 0.0 {
                    (true, Some(InputAction::PrevImage { steps }))
                } else if *y < 0.0 {
                    (true, Some(InputAction::NextImage { steps }))
                } else {
                    (false, None)
                }
            }
            winit::event::MouseScrollDelta::PixelDelta(pos) => {
                if pos.y > 0.0 {
                    (true, Some(InputAction::PrevImage { steps }))
                } else if pos.y < 0.0 {
                    (true, Some(InputAction::NextImage { steps }))
                } else {
                    (false, None)
                }
            }
        }
    }

    fn handle_keyboard_pressed(
        &mut self,
        physical_key: &PhysicalKey,
        modifiers: &ModifiersState,
        ctx: &InputContext,
    ) -> (bool, Option<InputAction>) {
        self.last_cursor_move = Instant::now();

        let action = match physical_key {
            PhysicalKey::Code(KeyCode::Escape) => match ctx.front_overlay {
                Some(OverlayKind::Gallery) => Some(InputAction::ToggleGallery),
                Some(OverlayKind::Help) => Some(InputAction::ToggleHelpOverlay),
                Some(OverlayKind::Settings) => Some(InputAction::ToggleSettings),
                None => Some(InputAction::Exit),
            },
            PhysicalKey::Code(KeyCode::KeyQ) => Some(InputAction::Exit),
            PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::Space) => {
                let steps = if modifiers.shift_key() { 10 } else { 1 };
                Some(InputAction::NextImage { steps })
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                let steps = if modifiers.shift_key() { 10 } else { 1 };
                Some(InputAction::PrevImage { steps })
            }
            PhysicalKey::Code(KeyCode::Home) => Some(InputAction::JumpTo(0)),
            PhysicalKey::Code(KeyCode::End) => {
                Some(InputAction::JumpTo(ctx.image_count.saturating_sub(1)))
            }
            PhysicalKey::Code(KeyCode::KeyP) => Some(InputAction::TogglePause),
            PhysicalKey::Code(KeyCode::KeyF) => Some(InputAction::ToggleFullscreen),
            PhysicalKey::Code(KeyCode::KeyD) => Some(InputAction::ToggleDecorations),
            PhysicalKey::Code(KeyCode::KeyT) => Some(InputAction::ToggleAlwaysOnTop),
            PhysicalKey::Code(KeyCode::BracketLeft) => {
                let delta = if modifiers.shift_key() {
                    -60.0
                } else {
                    // Timer step calculation deferred to ApplicationState
                    -1.0 // Placeholder, will be recalculated
                };
                Some(InputAction::AdjustTimer(delta))
            }
            PhysicalKey::Code(KeyCode::BracketRight) => {
                let delta = if modifiers.shift_key() {
                    60.0
                } else {
                    1.0 // Placeholder
                };
                Some(InputAction::AdjustTimer(delta))
            }
            PhysicalKey::Code(KeyCode::Backspace) => {
                if modifiers.shift_key() {
                    Some(InputAction::ResetColorAdjustments)
                } else {
                    Some(InputAction::ResetTimer)
                }
            }
            PhysicalKey::Code(KeyCode::KeyS) => Some(InputAction::Screenshot),
            PhysicalKey::Code(
                key @ (KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Digit6
                | KeyCode::Digit7
                | KeyCode::Digit8),
            ) if !modifiers.alt_key() && !modifiers.shift_key() && !modifiers.control_key() => {
                Some(InputAction::ColorAdjust { key: *key })
            }
            PhysicalKey::Code(KeyCode::Digit0) if modifiers.alt_key() => ctx
                .current_image_size
                .map(|(w, h)| InputAction::ResizeWindow {
                    width: (w / 2).max(1),
                    height: (h / 2).max(1),
                }),
            PhysicalKey::Code(KeyCode::Digit1) if modifiers.alt_key() => ctx
                .current_image_size
                .map(|(w, h)| InputAction::ResizeWindow {
                    width: w,
                    height: h,
                }),
            PhysicalKey::Code(KeyCode::Digit2) if modifiers.alt_key() => ctx
                .current_image_size
                .map(|(w, h)| InputAction::ResizeWindow {
                    width: w * 2,
                    height: h * 2,
                }),
            PhysicalKey::Code(KeyCode::KeyI) => {
                if modifiers.shift_key() {
                    Some(InputAction::ToggleInfoOverlay)
                } else {
                    Some(InputAction::ShowInfoTemporary)
                }
            }
            PhysicalKey::Code(KeyCode::KeyO) => {
                if modifiers.shift_key() {
                    Some(InputAction::ToggleFilenameDisplay)
                } else {
                    Some(InputAction::ShowFilenameTemporary)
                }
            }
            PhysicalKey::Code(KeyCode::KeyL) => Some(InputAction::ToggleLoop),
            PhysicalKey::Code(KeyCode::KeyA) => Some(InputAction::ToggleFitMode),
            PhysicalKey::Code(KeyCode::KeyC)
                if modifiers.control_key() && modifiers.shift_key() =>
            {
                Some(InputAction::CopyImageToClipboard)
            }
            PhysicalKey::Code(KeyCode::KeyC)
                if modifiers.control_key() && !modifiers.shift_key() =>
            {
                Some(InputAction::CopyPathToClipboard)
            }
            PhysicalKey::Code(KeyCode::KeyE) if modifiers.alt_key() => {
                Some(InputAction::OpenInExplorer)
            }
            PhysicalKey::Code(KeyCode::Slash) if modifiers.shift_key() => {
                Some(InputAction::ToggleHelpOverlay)
            }
            PhysicalKey::Code(KeyCode::KeyG) => Some(InputAction::ToggleGallery),
            PhysicalKey::Code(KeyCode::KeyZ) => Some(InputAction::ResetZoom),
            _ => None,
        };

        (action.is_some(), action)
    }
}
