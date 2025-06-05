use windows_capture::window::Window;

/// 获取当前前台窗口的标题
pub fn get_foreground_window_name() -> String {
    match Window::foreground() {
        Ok(window) => window.title().unwrap_or_default().to_string(),
        Err(_) => String::new(),
    }
}
