use anyhow::{Context, Result};
use image::DynamicImage;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

use ez_ffmpeg::core::context::ffmpeg_context::FfmpegContext;
use ez_ffmpeg::core::context::input::Input;
use ez_ffmpeg::core::context::output::Output;

use crate::config::ScreenshotFormat;

/// 编码器配置结构体，用于封装不同格式的参数
#[derive(Debug)]
struct EncoderConfig {
    format: &'static str,
    codec: &'static str,
    options: Vec<(&'static str, String)>,
}

impl EncoderConfig {
    /// 根据格式和质量参数创建编码器配置
    fn new(format: ScreenshotFormat, quality: u8, speed: u8) -> Self {
        match format {
            ScreenshotFormat::Avif => Self {
                format: "avif",
                codec: "libaom-av1",
                options: vec![
                    ("crf", Self::clamp_quality(quality, 0, 63).to_string()),
                    ("still-picture", "1".to_string()),
                    ("cpu-used", Self::clamp_speed(speed, 0, 8).to_string()),
                ],
            },
            ScreenshotFormat::Webp => Self {
                format: "webp",
                codec: "libwebp",
                options: vec![
                    ("qscale", Self::clamp_quality(quality, 0, 100).to_string()),
                    ("preset", Self::webp_preset(speed)),
                ],
            },
            ScreenshotFormat::Png => Self {
                format: "png",
                codec: "png",
                options: vec![(
                    "compression_level",
                    Self::clamp_speed(speed, 0, 9).to_string(),
                )],
            },
        }
    }

    /// 将质量值限制在指定范围内
    fn clamp_quality(quality: u8, min: u8, max: u8) -> u8 {
        quality.clamp(min, max)
    }

    /// 将速度值限制在指定范围内  
    fn clamp_speed(speed: u8, min: u8, max: u8) -> u8 {
        speed.clamp(min, max)
    }

    /// 根据速度参数选择 WebP 预设
    fn webp_preset(speed: u8) -> String {
        match speed {
            0..=2 => "picture".to_string(),
            3..=5 => "photo".to_string(),
            6..=7 => "drawing".to_string(),
            _ => "default".to_string(),
        }
    }
}

/// 线程安全的输入数据源
struct InputDataSource {
    cursor: Arc<Mutex<Cursor<Vec<u8>>>>,
}

impl InputDataSource {
    /// 创建新的输入数据源
    fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Arc::new(Mutex::new(Cursor::new(data))),
        }
    }

    /// 获取 Arc 引用，用于创建回调函数
    fn get_cursor(&self) -> Arc<Mutex<Cursor<Vec<u8>>>> {
        Arc::clone(&self.cursor)
    }
}

/// 线程安全的输出数据目标
struct OutputDataSink {
    cursor: Arc<Mutex<Cursor<Vec<u8>>>>,
}

impl OutputDataSink {
    /// 创建新的输出数据目标
    fn new() -> Self {
        Self {
            cursor: Arc::new(Mutex::new(Cursor::new(Vec::new()))),
        }
    }

    /// 获取 Arc 引用，用于创建回调函数
    fn get_cursor(&self) -> Arc<Mutex<Cursor<Vec<u8>>>> {
        Arc::clone(&self.cursor)
    }

    /// 获取最终的编码数据
    fn into_data(self) -> Vec<u8> {
        // 安全地获取数据，避免 Arc::try_unwrap 可能的竞态条件
        self.cursor.lock().unwrap().get_ref().clone()
    }
}

/// 准备输入数据，处理 filter graph 初始化的竞态条件
///
/// 通过发送两帧相同的数据来确保 filter graph 在输入流发送 EOF 之前完成配置
fn prepare_input_data(image: &DynamicImage) -> (Vec<u8>, u32, u32) {
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let raw_pixels = rgba_image.into_raw();

    // 发送两帧相同的数据来避免 filter graph 初始化竞态条件
    let mut two_frames_data = raw_pixels.clone();
    two_frames_data.extend_from_slice(&raw_pixels);

    (two_frames_data, width, height)
}

/// 创建并配置输入
fn create_input(input_cursor: Arc<Mutex<Cursor<Vec<u8>>>>, width: u32, height: u32) -> Input {
    let read_callback = move |buf: &mut [u8]| -> i32 {
        let mut cursor = input_cursor.lock().unwrap();
        match cursor.read(buf) {
            Ok(0) => ffmpeg_sys_next::AVERROR_EOF,
            Ok(bytes_read) => bytes_read as i32,
            Err(_) => ffmpeg_sys_next::AVERROR(ffmpeg_sys_next::EIO),
        }
    };

    Input::new_by_read_callback(read_callback)
        .set_format("rawvideo")
        .set_video_codec("rawvideo")
        .set_input_opt("pixel_format", "rgba")
        .set_input_opt("video_size", &format!("{}x{}", width, height))
        .set_input_opt("framerate", "2") // 设置帧率为 2，因为我们发送两帧
}

/// 创建并配置输出
fn create_output(output_cursor: Arc<Mutex<Cursor<Vec<u8>>>>, config: &EncoderConfig) -> Output {
    let write_callback = {
        let cursor = Arc::clone(&output_cursor);
        move |packet: &[u8]| -> i32 {
            let mut cursor = cursor.lock().unwrap();
            match cursor.write_all(packet) {
                Ok(_) => packet.len() as i32,
                Err(_) => ffmpeg_sys_next::AVERROR(ffmpeg_sys_next::EIO),
            }
        }
    };

    let seek_callback = {
        let cursor = Arc::clone(&output_cursor);
        move |offset: i64, whence: i32| -> i64 {
            let mut cursor = cursor.lock().unwrap();

            // 处理 FFmpeg 请求获取流大小的情况
            if whence == ffmpeg_sys_next::AVSEEK_SIZE {
                return cursor.get_ref().len() as i64;
            }

            let seek_from = match whence {
                ffmpeg_sys_next::SEEK_SET => SeekFrom::Start(offset as u64),
                ffmpeg_sys_next::SEEK_CUR => SeekFrom::Current(offset),
                ffmpeg_sys_next::SEEK_END => SeekFrom::End(offset),
                _ => return ffmpeg_sys_next::AVERROR(ffmpeg_sys_next::EINVAL) as i64,
            };

            match cursor.seek(seek_from) {
                Ok(pos) => pos as i64,
                Err(_) => ffmpeg_sys_next::AVERROR(ffmpeg_sys_next::EIO) as i64,
            }
        }
    };

    let mut output = Output::new_by_write_callback(write_callback)
        .set_seek_callback(seek_callback)
        .set_format(config.format)
        .set_video_codec(config.codec)
        .set_max_video_frames(1); // 关键：只编码第一帧

    // 应用编码器选项
    for (key, value) in &config.options {
        output = output.set_video_codec_opt(*key, value);
    }

    output
}

/// 使用 ez_ffmpeg 对内存中的 DynamicImage 进行编码
///
/// # 参数
/// * `format` - 目标编码格式 (Avif, Webp, Png)
/// * `quality` - 图像质量 (0-100，具体范围取决于编码器)
/// * `speed` - 编码速度 (0-最慢但质量最好，值越大速度越快)
/// * `image` - 从 `capture` 模块获取的 DynamicImage 对象
///
/// # 返回
/// * `Result<Vec<u8>>` - 编码后的图像数据字节流
///
/// # 错误
/// 当图像转换、FFmpeg 上下文创建或编码过程中发生错误时返回错误
pub fn encode(
    format: ScreenshotFormat,
    quality: u8,
    speed: u8,
    image: &DynamicImage,
) -> Result<Vec<u8>> {
    // 1. 准备编码器配置
    let config = EncoderConfig::new(format, quality, speed);

    // 2. 准备输入数据（处理竞态条件）
    let (input_data, width, height) = prepare_input_data(image);

    // 3. 创建输入和输出数据处理器
    let data_source = InputDataSource::new(input_data);
    let data_sink = OutputDataSink::new();

    // 4. 配置输入和输出
    let input = create_input(data_source.get_cursor(), width, height);
    let output = create_output(data_sink.get_cursor(), &config);

    // 5. 构建并执行 FFmpeg 上下文（无需滤镜）
    FfmpegContext::builder()
        .input(input)
        .output(output)
        .build()
        .context("Failed to build FFmpeg context")?
        .start()
        .context("Failed to start FFmpeg encoding")?
        .wait()
        .context("FFmpeg encoding failed")?;

    // 6. 返回编码结果
    Ok(data_sink.into_data())
}
