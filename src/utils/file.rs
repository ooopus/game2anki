use std::time::{SystemTime, UNIX_EPOCH};

/// 生成安全的文件名，避免特殊字符
///
/// # 参数
/// - prefix: 文件名前缀
/// - ext: 文件扩展名
pub fn generate_safe_filename(prefix: &str, ext: &str) -> String {
    let window_name = crate::utils::window::get_foreground_window_name();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 替换非法字符
    let safe_window_name: String = window_name
        .chars()
        .map(|c| if r#"/\?%*:|"<>."#.contains(c) { '_' } else { c })
        .collect();

    format!("{prefix}_{safe_window_name}_{timestamp}.{ext}")
}
