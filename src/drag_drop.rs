//! Drag & drop handler using Windows WM_DROPFILES API.
//!
//! winit 0.29 registers OLE drag-and-drop by default, but on some Windows
//! configurations the events never fire. This module replaces that with the
//! simpler, more reliable `DragAcceptFiles` / `WM_DROPFILES` mechanism.

use camino::Utf8PathBuf;
use log::warn;
use std::sync::mpsc;

/// Receiver half lives in `ApplicationState`, sender is captured by the
/// event-loop message hook.
pub struct DragDropHandler {
    rx: mpsc::Receiver<Vec<Utf8PathBuf>>,
}

impl DragDropHandler {
    /// Create a handler pair.  Returns `(handler, sender)` — the sender must
    /// be moved into the message hook via [`install_msg_hook`].
    pub fn new() -> (Self, mpsc::Sender<Vec<Utf8PathBuf>>) {
        let (tx, rx) = mpsc::channel();
        (Self { rx }, tx)
    }

    /// Drain all batches received since the last call. Returns `None` when
    /// nothing was dropped.
    pub fn take_pending(&self) -> Option<Vec<Utf8PathBuf>> {
        let mut all = Vec::new();
        while let Ok(batch) = self.rx.try_recv() {
            all.extend(batch);
        }
        if all.is_empty() { None } else { Some(all) }
    }
}

// ---- Windows-specific glue ------------------------------------------------

/// Install a message hook that intercepts `WM_DROPFILES` and forwards the
/// paths through `tx`.  Must be called on the `EventLoopBuilder` *before*
/// `.build()`.
#[cfg(windows)]
pub fn build_msg_hook(
    tx: mpsc::Sender<Vec<Utf8PathBuf>>,
) -> impl FnMut(*const std::ffi::c_void) -> bool {
    use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};

    const WM_DROPFILES: u32 = 0x0233;

    move |msg_ptr: *const std::ffi::c_void| {
        #[repr(C)]
        struct Msg {
            hwnd: isize,
            message: u32,
            wparam: usize,
            lparam: isize,
            time: u32,
            pt_x: i32,
            pt_y: i32,
        }

        let msg = unsafe { &*(msg_ptr as *const Msg) };
        if msg.message != WM_DROPFILES {
            return false; // let winit handle it
        }

        let hdrop = HDROP(msg.wparam as *mut std::ffi::c_void);
        let count = unsafe { DragQueryFileW(hdrop, 0xFFFF_FFFF, None) };
        let mut paths = Vec::with_capacity(count as usize);

        for i in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
            let mut buf = vec![0u16; len + 1];
            unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) };
            let os_str = String::from_utf16_lossy(&buf[..len]);
            match Utf8PathBuf::try_from(std::path::PathBuf::from(os_str)) {
                Ok(p) => paths.push(p),
                Err(e) => warn!("Dropped path is not valid UTF-8: {}", e),
            }
        }
        unsafe { DragFinish(hdrop) };

        if !paths.is_empty() {
            let _ = tx.send(paths);
        }
        true // we handled it — don't let winit see it
    }
}

/// Call after the window is created to enable `WM_DROPFILES` on the HWND.
#[cfg(windows)]
pub fn enable_wm_dropfiles(window: &winit::window::Window) {
    use windows::Win32::System::Ole::RevokeDragDrop;
    use windows::Win32::UI::Shell::DragAcceptFiles;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else {
        warn!("Could not get window handle for drag-and-drop setup");
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(win32.hwnd.get() as *mut _);

    // Remove the OLE drop-target that winit registered (if any) so that
    // WM_DROPFILES messages are delivered instead.
    let _ = unsafe { RevokeDragDrop(hwnd) };

    // Enable the classic WM_DROPFILES mechanism.
    unsafe { DragAcceptFiles(hwnd, true) };
    log::info!("Enabled WM_DROPFILES drag-and-drop");
}
