// src/screenshot/encode.rs
use crate::config::ScreenshotFormat;
use anyhow::Result;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use image::DynamicImage;
use std::io::Write;
use std::path::Path;

pub fn encode_to_file(
    config: &crate::config::Screenshot,
    image: &DynamicImage,
    output_path: &Path,
) -> Result<()> {
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let raw_pixels = rgba_image.into_raw();

    let mut command = FfmpegCommand::new();

    // 1. 设置输入参数
    command.args([
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &format!("{width}x{height}"),
        "-framerate",
        "1",
    ]);
    command.input("-");

    // 2. 根据格式和配置设置编码参数
    match config.format {
        ScreenshotFormat::Avif => {
            command.args([
                "-c:v",
                &config.avif.encoder,
                "-crf",
                &config.avif.crf.to_string(),
                "-cpu-used",
                &config.avif.cpu_used.to_string(),
                "-still-picture",
                "1",
            ]);
        }
        ScreenshotFormat::Webp => {
            command.args([
                "-c:v",
                "libwebp",
                "-quality",
                &config.webp.quality.to_string(),
            ]);
        }
        ScreenshotFormat::Png => {
            command.args([
                "-c:v",
                "png",
                "-compression_level",
                &config.png.compression_level.to_string(),
            ]);
        }
    }

    command.overwrite();
    command.output(output_path.to_str().unwrap());
    command.print_command();

    // 3. 启动进程
    let mut child = command.spawn()?;
    let mut stdin = child.take_stdin().expect("Failed to open FFmpeg stdin");
    let write_thread = std::thread::spawn(move || stdin.write_all(&raw_pixels));

    // 4. 迭代和等待逻辑
    let mut ffmpeg_errors = Vec::new();
    for event in child.iter()? {
        if let FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, msg) = event {
            ffmpeg_errors.push(msg);
        }
    }

    let status = child.wait()?;

    // 确保写入线程也完成了
    if let Err(_) = write_thread.join() {
        anyhow::bail!("FFmpeg stdin writer thread panicked.");
    }

    // 5. 根据退出状态和捕获的错误来判断结果
    if !status.success() {
        if ffmpeg_errors.is_empty() {
            anyhow::bail!("FFmpeg process exited with non-zero status: {}", status);
        } else {
            anyhow::bail!("FFmpeg failed with errors: {}", ffmpeg_errors.join("\n"));
        }
    } else if !ffmpeg_errors.is_empty() {
        anyhow::bail!(
            "FFmpeg process succeeded but reported errors: {}",
            ffmpeg_errors.join("\n")
        );
    }

    Ok(())
}
