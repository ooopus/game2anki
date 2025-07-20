use anyhow::{Result, anyhow};
use image::DynamicImage;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

// Import core components from ez_ffmpeg
use ez_ffmpeg::core::context::ffmpeg_context::FfmpegContext;
use ez_ffmpeg::core::context::input::Input;
use ez_ffmpeg::core::context::output::Output;

// Use the existing screenshot format definition from the project
use crate::config::ScreenshotFormat;

/// Encodes an in-memory DynamicImage using ez_ffmpeg
///
/// # Arguments
/// * `format` - The target encoding format (Avif, Webp, Png)
/// * `quality` - Image quality (the meaning depends on the encoder)
/// * `_speed` - Encoding speed (this is usually controlled by presets in ez_ffmpeg, temporarily unused)
/// * `image` - The DynamicImage object from the `capture` module
///
/// # Returns
/// * `Result<Vec<u8>>` - A byte stream of the encoded image data
pub fn encode(
    format: ScreenshotFormat,
    quality: u8,
    _speed: u8, // speed parameter is handled differently in ez_ffmpeg, ignoring for now
    image: &DynamicImage,
) -> Result<Vec<u8>> {
    // 1. Prepare the input data
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let raw_pixels = rgba_image.into_raw();

    // Use Arc<Mutex<Cursor>> to create a thread-safe, readable in-memory buffer.
    // This is necessary because FFmpeg's callback might be executed in a different thread.
    let input_data = Arc::new(Mutex::new(Cursor::new(raw_pixels)));

    // 2. Define the input callback
    // This closure acts as the data source for FFmpeg. It's called whenever FFmpeg needs more data.
    let read_callback = move |buf: &mut [u8]| -> i32 {
        // Lock the shared input data
        let mut cursor = input_data.lock().unwrap();
        // Read data from the Cursor into the buffer provided by FFmpeg
        match cursor.read(buf) {
            Ok(0) => ffmpeg_sys_next::AVERROR_EOF, // End of file
            Ok(bytes_read) => bytes_read as i32,   // Return the number of bytes read
            Err(_) => ffmpeg_sys_next::AVERROR(ffmpeg_sys_next::EIO), // An error occurred
        }
    };

    // 3. Prepare the output buffer
    // Similarly, use Arc<Mutex<>> to create a thread-safe Vec<u8> to receive the encoded data.
    let output_buffer = Arc::new(Mutex::new(Vec::<u8>::new()));

    // 4. Define the output callback
    // This closure is called when FFmpeg has finished encoding a chunk of data.
    let write_callback = {
        let buffer = Arc::clone(&output_buffer);
        move |packet: &[u8]| -> i32 {
            // Lock the shared output buffer
            let mut buffer = buffer.lock().unwrap();
            // Write the encoded packet into the Vec
            buffer.extend_from_slice(packet);
            packet.len() as i32 // Return the number of bytes written
        }
    };

    // 5. Configure the ez_ffmpeg Input
    let input = Input::new_by_read_callback(read_callback)
        // Must inform FFmpeg that we are providing raw video data
        .set_format("rawvideo")
        // Set the parameters for the raw video
        .set_input_opt("pixel_format", "rgba") // Pixel format
        .set_input_opt("video_size", &format!("{}x{}", width, height)) // Video dimensions
        .set_input_opt("framerate", "1"); // Frame rate (for a single image, 1 is sufficient)

    // 6. Configure the ez_ffmpeg Output
    let (output_format, video_codec, codec_opts) = match format {
        // For different formats, set the appropriate container format, video codec, and encoder options
        ScreenshotFormat::Avif => (
            "avif",
            "libaom-av1",
            vec![
                ("crf", quality.to_string()), // Constant Rate Factor for quality control
                ("still-picture", "1".to_string()), // Flag as a still picture
                ("cpu-used", "8".to_string()), // Encoding speed/quality trade-off
            ],
        ),
        ScreenshotFormat::Webp => (
            "webp",
            "libwebp",
            vec![
                ("qscale", quality.to_string()), // Quality Scale
                ("preset", "default".to_string()),
            ],
        ),
        // Note: FFmpeg's PNG encoder is often used for video sequences. Using the `image` crate might be simpler.
        // However, for consistency, it's implemented here with FFmpeg.
        ScreenshotFormat::Png => (
            "png",
            "png",
            vec![
                // PNG quality parameters are complex; -compression_level can be used.
                ("compression_level", "100".to_string()),
            ],
        ),
    };

    let mut output = Output::new_by_write_callback(write_callback)
        .set_format(output_format)
        .set_video_codec(video_codec)
        // Crucial: we are only encoding one frame (a single image)
        .set_max_video_frames(1);

    // Apply encoder-specific options
    for (key, value) in codec_opts {
        output = output.set_video_codec_opt(key, value);
    }

    // 7. Build and execute the FfmpegContext
    // This is the core of `ez_ffmpeg`, connecting inputs, outputs, and filters (not used here).
    FfmpegContext::builder()
        .input(input)
        .output(output)
        .build()? // Build the context
        .start()? // Start the FFmpeg task
        .wait()?; // Wait for the task to complete

    // 8. Return the result
    // Extract the encoded data from the shared output buffer
    let final_data = output_buffer
        .lock()
        .map_err(|_| anyhow!("Failed to lock output buffer"))?
        .clone();

    Ok(final_data)
}
