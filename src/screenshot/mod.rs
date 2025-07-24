// src/screenshot/mod.rs
use crate::{
    anki::AnkiClient,
    config::{MediaConfig, Screenshot},
    utils::file::generate_safe_filename,
};
use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;

mod capture;
mod encode;

use capture::capture_active_window;
use encode::encode_to_file;

pub struct AnkiScreenshot {
    cfg: Screenshot,
    anki: Arc<AnkiClient>,
}

impl AnkiScreenshot {
    pub fn new(cfg: Screenshot, anki: Arc<AnkiClient>) -> Self {
        Self { cfg, anki }
    }

    pub async fn on_hotkey_clicked(&self) -> Result<()> {
        // 1. 调用新的 `generate_safe_filename`
        // 它现在接收整个 self.cfg 的引用，因为 Screenshot 实现了 MediaConfig trait
        let filename = generate_safe_filename(&self.cfg);

        // 2. 获取 Anki 的媒体目录，并构建完整的文件路径
        let media_dir = self.anki.get_media_dir().await?;
        let final_path = std::path::Path::new(&media_dir).join(&filename);

        // 3. 捕获窗口
        // 注意：capture_active_window 现在接收整个 Screenshot 配置
        let screenshot = capture_active_window(self.cfg.clone())?;

        // 4. 调用新的 `encode_to_file` 函数，使用新的配置结构
        encode_to_file(&self.cfg, &screenshot, &final_path)?;

        debug!("截图已成功编码并保存到文件: {}", final_path.display());

        // 5. 获取 note_id
        let note_id = self.anki.get_latest_note_id().await?;

        // 6. 更新卡片字段
        // 文件已经存在于媒体目录中，只需更新卡片引用
        self.anki
            .update_note_field(
                note_id,
                self.cfg.field_name(),
                &format!("<img src=\"{}\">", filename),
            )
            .await?;

        info!("截图已成功保存到 Anki 卡片 ID: {}", note_id);
        Ok(())
    }
}
