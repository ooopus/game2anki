//! Window management utilities

use windows_capture::window::Window;

/// Gets the name of the currently focused foreground window
pub fn get_foreground_window_name() -> String {
    match Window::foreground() {
        Ok(window) => window.title().unwrap_or_default().to_string(),
        Err(_) => String::new(),
    }
}
