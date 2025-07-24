// src/audio/encode.rs
use anyhow::Result;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use log::debug;
use std::io::Write;
use std::path::Path;

pub fn encode_to_file(
    config: &crate::config::AudioRecord,
    raw_pcm: &[f32],
    channels: u16,
    output_path: &Path,
) -> Result<()> {
    let mut command = FfmpegCommand::new();

    let sample_rate = match config.format {
        crate::config::AudioFormat::Opus => config.opus.sample_rate,
        crate::config::AudioFormat::Mp3 => config.mp3.sample_rate,
    };

    command.args([
        "-f",
        "f32le",
        "-ar",
        &sample_rate.to_string(),
        "-ac",
        &channels.to_string(),
    ]);
    command.input("-");

    match config.format {
        crate::config::AudioFormat::Opus => {
            let opus_cfg = &config.opus;
            command.args([
                "-c:a",
                "libopus",
                "-b:a",
                &format!("{}k", opus_cfg.bit_rate),
            ]);
        }
        crate::config::AudioFormat::Mp3 => {
            let mp3_cfg = &config.mp3;
            command.args([
                "-c:a",
                "libmp3lame",
                "-b:a",
                &format!("{}k", mp3_cfg.bit_rate),
                "-q:a",
                &mp3_cfg.quality.to_string(),
            ]);
        }
    }

    command.overwrite();
    command.output(output_path.to_str().unwrap());

    debug!("Preparing to execute FFmpeg command...");
    command.print_command();

    let mut child = command.spawn()?;

    // Get process ID for debugging
    debug!(
        "Spawned FFmpeg process with PID: {:?}",
        child.as_inner().id()
    );

    // Convert f32 PCM data to bytes
    let pcm_data: Vec<u8> = raw_pcm.iter().flat_map(|&f| f.to_le_bytes()).collect();
    let mut stdin = child.take_stdin().expect("Failed to open FFmpeg stdin");

    let write_thread = std::thread::spawn(move || {
        debug!(
            "Writing {} bytes of raw PCM data to FFmpeg stdin...",
            pcm_data.len()
        );
        let result = stdin.write_all(&pcm_data);
        debug!("Finished writing to FFmpeg stdin.");
        result
    });

    // Collect FFmpeg errors
    let mut ffmpeg_errors = Vec::new();
    for event in child.iter()? {
        if let FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, msg) = event {
            debug!("Captured FFmpeg error log: {}", msg);
            ffmpeg_errors.push(msg);
        }
    }

    let status = child.wait()?;
    debug!("FFmpeg process finished with status: {}", status);

    // Check write thread result
    if let Err(e) = write_thread.join().unwrap() {
        anyhow::bail!("FFmpeg (audio) stdin writer thread failed: {}", e);
    }

    // Check process exit status and errors
    if !status.success() {
        if ffmpeg_errors.is_empty() {
            anyhow::bail!("FFmpeg (audio) exited with non-zero status: {}", status);
        } else {
            anyhow::bail!(
                "FFmpeg (audio) failed with errors: {}",
                ffmpeg_errors.join("\n")
            );
        }
    } else if !ffmpeg_errors.is_empty() {
        anyhow::bail!(
            "FFmpeg (audio) succeeded but reported errors: {}",
            ffmpeg_errors.join("\n")
        );
    }

    Ok(())
}
