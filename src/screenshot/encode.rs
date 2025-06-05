use crate::config::ScreenshotFormat;
use anyhow::Result;
use image::DynamicImage;
use rgb::FromSlice;
use std::io::Cursor;

pub fn encode(
    format: ScreenshotFormat,
    quality: u8,
    speed: u8,
    image: &DynamicImage,
) -> Result<Vec<u8>> {
    match format {
        ScreenshotFormat::Avif => encode_to_avif(quality, speed, image),
        ScreenshotFormat::Webp => encode_to_webp(quality, image),
        ScreenshotFormat::Png => encode_to_png(image),
    }
}

pub fn encode_to_avif(quality: u8, speed: u8, image: &DynamicImage) -> Result<Vec<u8>> {
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();

    // Using ComponentSlice trait
    let pixels_rgba = rgba_image.as_raw().as_rgba();

    let img = ravif::Img::new(pixels_rgba, width as usize, height as usize);

    let encoded = ravif::Encoder::new()
        .with_quality(quality as f32)
        .with_alpha_quality(quality as f32)
        .with_speed(speed)
        .encode_rgba(img)?;

    Ok(encoded.avif_file)
}

pub fn encode_to_webp(quality: u8, image: &DynamicImage) -> Result<Vec<u8>> {
    let encoder = webp::Encoder::from_image(image)
        .map_err(|e| anyhow::anyhow!("Failed to create WebP encoder: {}", e))?;

    let img = encoder.encode(quality as f32).to_vec();

    Ok(img)
}

pub fn encode_to_png(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(buffer)
}
