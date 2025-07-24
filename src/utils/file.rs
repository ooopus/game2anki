// src/utils/file.rs

use crate::config::MediaConfig;
use std::time::{SystemTime, UNIX_EPOCH};

/// 生成安全的文件名，避免特殊字符。
///
/// # 参数
/// - cfg: 一个实现了 MediaConfig trait 的配置引用，用于提供文件名前缀和扩展名。
pub fn generate_safe_filename(cfg: &impl MediaConfig) -> String {
    // 使用您提供的确切函数名
    let window_title = crate::utils::window::get_foreground_window_name();

    // 如果窗口标题为空，提供一个默认值
    let window_title = if window_title.is_empty() {
        "NoWindowTitle".to_string()
    } else {
        window_title
    };

    // 使用您原来的时间戳逻辑 (秒级)
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 使用您原来的非法字符替换逻辑
    let safe_window_title: String = window_title
        .chars()
        .map(|c| if r#"/\?%*:|"<>."#.contains(c) { '_' } else { c })
        .collect();

    // 从 cfg 中获取前缀和扩展名
    let prefix = cfg.field_name();
    let ext = cfg.format();

    format!("{prefix}_{safe_window_title}_{timestamp}.{ext}")
}
