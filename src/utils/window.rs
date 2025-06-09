//! Window management utilities including handle abstraction
//! and foreground window detection.

use windows::Win32::Foundation::HWND;
use windows_capture::window::Window;

/// A sendable wrapper for HWND to safely pass window handles between threads
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendableHWND(pub HWND);

unsafe impl Send for SendableHWND {}
unsafe impl Sync for SendableHWND {}

impl SendableHWND {
    /// Creates a new SendableHWND from an HWND.
    pub fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    /// Returns the underlying HWND.
    pub fn hwnd(&self) -> HWND {
        self.0
    }
}

/// Gets the name of the currently focused foreground window
pub fn get_foreground_window_name() -> String {
    match Window::foreground() {
        Ok(window) => window.title().unwrap_or_default().to_string(),
        Err(_) => String::new(),
    }
}
