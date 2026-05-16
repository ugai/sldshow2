//! Drag & drop handler.
//!
//! On Windows, uses the `WM_DROPFILES` API directly (bypasses winit's OLE
//! drag-and-drop which can be unreliable on some configurations).
//! On other platforms, `WindowEvent::DroppedFile` events are forwarded through
//! the same channel via [`DragDropHandler::queue_dropped_file`].

use camino::Utf8PathBuf;
use std::sync::mpsc;

/// Aggregated drag-and-drop data collected since the previous frame.
pub struct PendingDrop {
    pub paths: Vec<Utf8PathBuf>,
    pub rejected_non_utf8: usize,
}

pub(crate) enum DragDropMessage {
    Paths(Vec<Utf8PathBuf>),
    RejectedNonUtf8(usize),
}

/// Receiver half lives in `ApplicationState`, sender is captured by the
/// event-loop message hook (Windows) or used directly via
/// [`DragDropHandler::queue_dropped_file`] (non-Windows).
pub struct DragDropHandler {
    rx: mpsc::Receiver<DragDropMessage>,
    /// Retained so non-Windows platforms can enqueue files from window events.
    #[cfg(not(windows))]
    tx: mpsc::Sender<DragDropMessage>,
}

impl DragDropHandler {
    /// Create a handler pair.  On Windows the returned sender must be moved
    /// into the message hook via [`build_msg_hook`].
    pub fn new() -> (Self, mpsc::Sender<DragDropMessage>) {
        let (tx, rx) = mpsc::channel();
        #[cfg(windows)]
        let handler = Self { rx };
        #[cfg(not(windows))]
        let handler = Self { rx, tx: tx.clone() };
        (handler, tx)
    }

    /// Drain all drag/drop events received since the last call.
    ///
    /// Returns `None` when no dropped paths were received and no dropped paths
    /// were rejected due to UTF-8 conversion failures.
    pub fn take_pending(&self) -> Option<PendingDrop> {
        let mut paths = Vec::new();
        let mut rejected_non_utf8 = 0usize;
        while let Ok(message) = self.rx.try_recv() {
            match message {
                DragDropMessage::Paths(batch) => paths.extend(batch),
                DragDropMessage::RejectedNonUtf8(count) => rejected_non_utf8 += count,
            }
        }
        if paths.is_empty() && rejected_non_utf8 == 0 {
            None
        } else {
            Some(PendingDrop {
                paths,
                rejected_non_utf8,
            })
        }
    }

    /// Enqueue a single dropped file path (non-Windows only).
    ///
    /// Call this from `WindowEvent::DroppedFile` on Linux/macOS so that the
    /// existing drain logic in [`take_pending`] picks it up on the next frame.
    #[cfg(not(windows))]
    pub fn queue_dropped_file(&self, path: std::path::PathBuf) {
        match Utf8PathBuf::try_from(path) {
            Ok(p) => {
                let _ = self.tx.send(DragDropMessage::Paths(vec![p]));
            }
            Err(e) => {
                log::warn!("Dropped path is not valid UTF-8: {}", e);
                let _ = self.tx.send(DragDropMessage::RejectedNonUtf8(1));
            }
        }
    }
}

// ---- Windows-specific glue ------------------------------------------------

#[cfg(windows)]
enum DroppedItem {
    Path(Utf8PathBuf),
    Skip,
    RejectedNonUtf8,
}

/// Classify a single file entry returned by `DragQueryFileW`.
///
/// `len` is the character count from the probe call (no null terminator).
/// `written_raw` is the count returned by the copy call.
/// `buf` is the buffer that was filled.
/// `i` is the entry index, used only for log messages.
#[cfg(windows)]
fn classify_drop_item(len: usize, written_raw: usize, buf: &[u16], i: u32) -> DroppedItem {
    if len == 0 {
        log::warn!("DragQueryFileW reported zero-length path at index {}", i);
        return DroppedItem::Skip;
    }

    if written_raw == 0 {
        log::warn!(
            "Dropped path could not be read from WM_DROPFILES at index {}",
            i
        );
        return DroppedItem::Skip;
    }

    // Guard against a misbehaving shell extension returning written > buf.len().
    let written = if written_raw > buf.len() {
        log::warn!(
            "DragQueryFileW returned written ({}) > buf.len() ({}) at index {}; clamping",
            written_raw,
            buf.len(),
            i
        );
        buf.len()
    } else {
        written_raw
    };

    let utf16 = &buf[..written];
    let path_str = match String::from_utf16(utf16) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Dropped path has invalid UTF-16 at index {}: {}", i, e);
            return DroppedItem::Skip;
        }
    };

    if path_str.is_empty() {
        log::warn!("DragQueryFileW produced empty path string at index {}", i);
        return DroppedItem::Skip;
    }

    match Utf8PathBuf::try_from(std::path::PathBuf::from(path_str)) {
        Ok(p) => DroppedItem::Path(p),
        Err(e) => {
            log::warn!("Dropped path is not valid UTF-8 at index {}: {}", i, e);
            DroppedItem::RejectedNonUtf8
        }
    }
}

/// Install a message hook that intercepts `WM_DROPFILES` and forwards the
/// paths through `tx`.  Must be called on the `EventLoopBuilder` *before*
/// `.build()`.
#[cfg(windows)]
pub fn build_msg_hook(
    tx: mpsc::Sender<DragDropMessage>,
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
        let mut rejected_non_utf8 = 0usize;

        for i in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
            let mut buf = vec![0u16; len + 1];
            let written_raw = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) } as usize;
            match classify_drop_item(len, written_raw, &buf, i) {
                DroppedItem::Path(p) => paths.push(p),
                DroppedItem::Skip => continue,
                DroppedItem::RejectedNonUtf8 => rejected_non_utf8 += 1,
            }
        }
        unsafe { DragFinish(hdrop) };

        if !paths.is_empty() {
            let _ = tx.send(DragDropMessage::Paths(paths));
        }
        if rejected_non_utf8 > 0 {
            let _ = tx.send(DragDropMessage::RejectedNonUtf8(rejected_non_utf8));
        }
        true // we handled it — don't let winit see it
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn encode_utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn zero_len_is_skipped() {
        let buf = vec![0u16; 1];
        assert!(matches!(
            classify_drop_item(0, 0, &buf, 0),
            DroppedItem::Skip
        ));
    }

    #[test]
    fn written_zero_with_nonzero_len_is_skipped() {
        let buf = vec![0u16; 6];
        assert!(matches!(
            classify_drop_item(5, 0, &buf, 1),
            DroppedItem::Skip
        ));
    }

    #[test]
    fn written_exceeds_buf_does_not_panic() {
        // buf holds "C:\x" + null; written_raw reports more than buf.len().
        let mut buf = encode_utf16("C:\\x");
        buf.push(0); // null terminator slot
        let path_len = buf.len() - 1; // length without null
        let written_raw = buf.len() + 5; // exceeds buf.len()
        // Must not panic; result may be Path or Skip depending on decoded content.
        let result = classify_drop_item(path_len, written_raw, &buf, 2);
        assert!(matches!(result, DroppedItem::Path(_) | DroppedItem::Skip));
    }

    #[test]
    fn valid_path_produces_path_item() {
        let path = "C:\\Users\\test\\image.png";
        let mut buf = encode_utf16(path);
        buf.push(0); // null terminator slot (not included in written)
        let written = path.encode_utf16().count();
        let result = classify_drop_item(written, written, &buf, 0);
        assert!(matches!(result, DroppedItem::Path(_)));
        if let DroppedItem::Path(p) = result {
            assert_eq!(p.as_str(), path);
        }
    }
}

/// Call after the window is created to enable `WM_DROPFILES` on the HWND.
#[cfg(windows)]
pub fn enable_wm_dropfiles(window: &winit::window::Window) {
    use windows::Win32::System::Ole::RevokeDragDrop;
    use windows::Win32::UI::Shell::DragAcceptFiles;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else {
        log::warn!("Could not get window handle for drag-and-drop setup");
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
