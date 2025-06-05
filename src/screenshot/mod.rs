use crate::config::ScreenshotFormat;
use crate::screenshot::capture::capture_active_window;
use crate::{anki::AnkiClient, config::Screenshot};
use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use windows_capture::window::Window;
mod capture;
mod encode;
use encode::encode;

impl Default for Screenshot {
    fn default() -> Self {
        Self {
            format: crate::config::ScreenshotFormat::Avif,
            field_name: "Picture".to_string(),
            quality: 80,
            speed: 6,
            exclude_title_bar: true,
        }
    }
}

pub struct AnkiScreenshot {
    cfg: Screenshot,
    anki: Arc<AnkiClient>,
}

impl AnkiScreenshot {
    pub fn new(cfg: Screenshot, anki: Arc<AnkiClient>) -> Self {
        Self { cfg, anki }
    }

    /// 外部调用的热键处理函数
    pub async fn on_hotkey_clicked(&self) -> Result<()> {
        let filename = self
            .generate_filename(&self.cfg.field_name, self.cfg.format.clone())
            .await;
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
        // 更新卡片字段
        self.anki
            .update_note_field(
                note_id,
                &self.cfg.field_name,
                &format!("<img src=\"{}\">", filename),
            )
            .await?;

        info!("截图已成功保存到Anki卡片 ID: {}", note_id);
        Ok(())
    }
    pub async fn generate_filename(&self, prefix: &str, ext: ScreenshotFormat) -> String {
        let window_name = Self::get_foreground_window_name();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let safe_window_name: String = window_name
            .chars()
            .map(|c| if r#"/\?%*:|"<>."#.contains(c) { '_' } else { c })
            .collect();
        format!("{}_{}_{}.{:?}", prefix, safe_window_name, timestamp, ext)
    }
    pub fn get_foreground_window_name() -> String {
        let window = Window::foreground().unwrap();
        window.title().unwrap().to_string()
    }
}
