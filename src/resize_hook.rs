//! Win32 resize-drag detection hook.
//!
//! Sets an `AtomicBool` flag while the user is dragging a window border
//! (`WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE`).  The application skips
//! expensive `surface.configure()` calls and renders at the old surface
//! size while the flag is set, letting DWM stretch the frame instead.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const WM_ENTERSIZEMOVE: u32 = 0x0231;
const WM_EXITSIZEMOVE: u32 = 0x0232;

/// Build a Win32 message hook that tracks resize-drag state.
///
/// Returns the shared flag and a closure suitable for combining with other
/// hooks before passing to `EventLoopBuilderExtWindows::with_msg_hook`.
pub fn build_resize_hook() -> (Arc<AtomicBool>, impl FnMut(*const std::ffi::c_void) -> bool) {
    let resizing = Arc::new(AtomicBool::new(false));
    let flag = resizing.clone();

    let hook = move |msg_ptr: *const std::ffi::c_void| {
        #[repr(C)]
        struct Msg {
            _hwnd: isize,
            message: u32,
            _wparam: usize,
            _lparam: isize,
            _time: u32,
            _pt_x: i32,
            _pt_y: i32,
        }

        let msg = unsafe { &*(msg_ptr as *const Msg) };
        match msg.message {
            WM_ENTERSIZEMOVE => {
                flag.store(true, Ordering::Release);
                false // let winit process it too
            }
            WM_EXITSIZEMOVE => {
                flag.store(false, Ordering::Release);
                false
            }
            _ => false,
        }
    };

    (resizing, hook)
}
