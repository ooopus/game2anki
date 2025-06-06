use crate::config::Screenshot;
use anyhow::{Result, anyhow};
use image::DynamicImage;
use log::{debug, info};
use std::sync::{Arc, Condvar, Mutex};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
    window::Window,
};
struct Flags {
    image_data: Arc<(Mutex<Option<DynamicImage>>, Condvar)>,
    exclude_title_bar: bool,
}

pub fn capture_active_window(cfg: Screenshot) -> Result<DynamicImage> {
    let pair = Arc::new((Mutex::new(None::<DynamicImage>), Condvar::new()));
    let flags = Arc::new(Flags {
        image_data: Arc::clone(&pair),
        exclude_title_bar: cfg.exclude_title_bar,
    });

    struct Handler {
        flags: Arc<Flags>,
    }
    impl GraphicsCaptureApiHandler for Handler {
        type Flags = Arc<Flags>;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(context: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self {
                flags: context.flags.clone(),
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let color_format = frame.color_format();
            let mut frame_buffer = if self.flags.exclude_title_bar {
                frame.buffer_without_title_bar().unwrap()
            } else {
                frame.buffer().unwrap()
            };
            let width = frame_buffer.width();
            let height = frame_buffer.height();
            let rgba = frame_buffer.as_raw_buffer();
            info!("捕获到帧: {}x{}, 格式: {:?}", width, height, color_format);
            let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| anyhow!("无法创建图像对象"))?;
            let (lock, cvar) = &*self.flags.image_data;
            *lock.lock().unwrap() = Some(img);
            cvar.notify_one();
            capture_control.stop();
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    // 获取当前焦点窗口
    let focus_window = Window::foreground().unwrap();
    debug!("当前焦点窗口: {:?}", focus_window);

    // 配置截屏设置
    let settings = Settings::new(
        focus_window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::Default,
        ColorFormat::Rgba8,
        Arc::clone(&flags),
    );

    Handler::start(settings)?;

    // 用条件变量优雅等待图片生成
    let (lock, cvar) = &*pair;
    let guard = lock.lock().unwrap();
    let timeout = std::time::Duration::from_secs(3);
    let (guard, _result) = cvar
        .wait_timeout_while(guard, timeout, |img| img.is_none())
        .unwrap();
    if let Some(img) = &*guard {
        return Ok(img.clone());
    }
    Err(anyhow!("截图超时"))
}
