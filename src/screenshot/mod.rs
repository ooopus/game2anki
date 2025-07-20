use crate::screenshot::capture::capture_active_window;
use crate::{anki::AnkiClient, config::Screenshot};
use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;
mod capture;
mod encode;
use crate::utils::file::generate_safe_filename;
use encode::encode;

pub struct AnkiScreenshot {
    cfg: Screenshot,
    anki: Arc<AnkiClient>,
}

impl AnkiScreenshot {
    pub fn new(cfg: Screenshot, anki: Arc<AnkiClient>) -> Self {
        Self { cfg, anki }
    }

    pub async fn on_hotkey_clicked(&self) -> Result<()> {
        let filename = generate_safe_filename(&self.cfg.field_name, &self.cfg.format.to_string());

        let screenshot = capture_active_window(self.cfg.clone())?;

        let _data = encode(
            self.cfg.format.clone(),
            self.cfg.quality,
            self.cfg.speed,
            &screenshot,
        )?; // 耗时操作，要放在获取窗口名之类的后面
        debug!(
            "截图格式：{:?}, 质量：{}, 速度：{}",
            self.cfg.format, self.cfg.quality, self.cfg.speed
        );

        let note_id = self.anki.get_latest_note_id().await?;

        let media_dir = self.anki.get_media_dir().await?;
        let file_path = std::path::Path::new(&media_dir).join(&filename);
        std::fs::write(&file_path, &_data)?;
        debug!("截图已保存到文件: {}", file_path.display());
        // 更新卡片字段
        self.anki
            .update_note_field(
                note_id,
                &self.cfg.field_name,
                &format!("<img src=\"{filename}\">"),
            )
            .await?;

        info!("截图已成功保存到Anki卡片 ID: {note_id}");
        Ok(())
    }
}
