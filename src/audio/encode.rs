use super::Res;
use crate::config::AudioFormat;
use log::{error, info};
use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm, MonoPcm};
use ogg::{PacketWriteEndInfo, writing::PacketWriter};
use opus::{Application, Channels, Encoder};
use std::io::Cursor;

pub fn encode(
    format: AudioFormat,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Res<Vec<u8>> {
    match format {
        AudioFormat::Opus => encode_to_ogg_opus(samples, sample_rate, channels),
        AudioFormat::Mp3 => encode_to_mp3(samples, sample_rate, channels),
    }
}

pub fn encode_to_ogg_opus(samples: &[f32], sample_rate: u32, channels: u16) -> Res<Vec<u8>> {
    info!(
        "Preparing to encode to Ogg Opus: sample_rate={}, channels={}, samples_len={}",
        sample_rate,
        channels,
        samples.len()
    );
    let opus_channels = match channels {
        1 => Channels::Mono,
        2 => Channels::Stereo,
        _ => {
            error!("Unsupported channel count for Opus: {channels}");
            return Err("Unsupported channel count".into());
        }
    };

    let encoder = Encoder::new(sample_rate, opus_channels, Application::Audio);
    let mut encoder = match encoder {
        Ok(enc) => enc,
        Err(e) => {
            error!(
                "Failed to create Opus encoder: sample_rate={sample_rate}, channels={opus_channels:?}, error={e}"
            );
            return Err(e.into());
        }
    };
    if let Err(e) = encoder.set_bitrate(opus::Bitrate::Bits(128 * 1000)) {
        error!("Failed to set Opus bitrate: {e}");
    }

    // Ogg Opus header (ID + Comment)
    let mut ogg_buf = Vec::new();
    let mut writer = PacketWriter::new(Cursor::new(&mut ogg_buf));
    let pre_skip = 312; // Opus spec recommends 312 for 48kHz
    let id_header = {
        let mut v = Vec::new();
        v.extend_from_slice(b"OpusHead"); // Magic signature
        v.push(1); // Version
        v.push(channels as u8); // Channel count
        v.extend_from_slice(&(pre_skip as u16).to_le_bytes()); // Pre-skip
        v.extend_from_slice(&sample_rate.to_le_bytes()); // Original sample rate
        v.extend_from_slice(&[0u8; 2]); // Output gain
        v.push(0); // Channel mapping family
        v
    };
    writer.write_packet(id_header, 1, PacketWriteEndInfo::EndPage, 0)?;
    let comment_header = {
        let mut v = Vec::new();
        v.extend_from_slice(b"OpusTags");
        v.extend_from_slice(&[6, 0, 0, 0]); // Vendor string length (6)
        v.extend_from_slice(b"rust"); // Vendor string (short)
        v.extend_from_slice(&[0, 0, 0, 0]); // User comment list length (0)
        v
    };
    writer.write_packet(comment_header, 1, PacketWriteEndInfo::EndPage, 0)?;

    // PCM to Opus
    let frame_size = 960; // 20ms at 48kHz
    let mut absgp = 0u64;
    for chunk in samples.chunks(frame_size * channels as usize) {
        let mut output = vec![0u8; 4000];
        let mut padded = Vec::from(chunk);
        if padded.len() < frame_size * channels as usize {
            padded.resize(frame_size * channels as usize, 0.0);
        }

        match encoder.encode_float(&padded, &mut output) {
            Ok(encoded_size) => {
                absgp += frame_size as u64;
                writer.write_packet(
                    output[..encoded_size].to_vec(),
                    1,
                    PacketWriteEndInfo::EndPage,
                    absgp,
                )?;
            }
            Err(e) => {
                error!("Opus encoding error: {e}");
            }
        }
    }
    Ok(ogg_buf)
}

pub fn encode_to_mp3(samples: &[f32], sample_rate: u32, channels: u16) -> Res<Vec<u8>> {
    info!(
        "Preparing to encode to MP3: sample_rate={}, channels={}, samples_len={}",
        sample_rate,
        channels,
        samples.len()
    );

    // Create and configure encoder
    let mut builder = Builder::new().ok_or_else(|| {
        let msg = "Failed to create LAME builder";
        error!("{msg}");
        msg.to_string()
    })?;
    builder.set_num_channels(channels as u8).map_err(|e| {
        error!("Failed to set channels: {e}");
        e.to_string()
    })?;
    builder.set_sample_rate(sample_rate).map_err(|e| {
        error!("Failed to set sample rate: {e}");
        e.to_string()
    })?;
    builder
        .set_brate(mp3lame_encoder::Bitrate::Kbps192)
        .map_err(|e| {
            error!("Failed to set bitrate: {e}");
            e.to_string()
        })?;
    builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| {
            error!("Failed to set quality: {e}");
            e.to_string()
        })?;

    let mut encoder = builder.build().map_err(|e| {
        error!("Failed to build LAME encoder: {e}");
        e.to_string()
    })?;

    let frame_size = 1152; // Typical MP3 frame size

    let mut mp3_out = Vec::new();

    for chunk in samples.chunks(frame_size * channels as usize) {
        if channels == 1 {
            let input = MonoPcm(chunk);
            let required = mp3lame_encoder::max_required_buffer_size(chunk.len());
            mp3_out.reserve(required);

            let encoded_size = encoder
                .encode(input, mp3_out.spare_capacity_mut())
                .map_err(|e| {
                    error!("MP3 encoding error: {e}");
                    e.to_string()
                })?;

            unsafe {
                mp3_out.set_len(mp3_out.len() + encoded_size);
            }
        } else if channels == 2 {
            let input = InterleavedPcm(chunk);
            let required = mp3lame_encoder::max_required_buffer_size(chunk.len() / 2);
            mp3_out.reserve(required);

            let encoded_size = encoder
                .encode(input, mp3_out.spare_capacity_mut())
                .map_err(|e| {
                    error!("MP3 encoding error: {e}");
                    e.to_string()
                })?;

            unsafe {
                mp3_out.set_len(mp3_out.len() + encoded_size);
            }
        } else {
            error!("Unsupported channel count for MP3: {channels}");
            return Err("Unsupported channel count".into());
        }
    }

    // Final flush
    let required = mp3lame_encoder::max_required_buffer_size(0);
    mp3_out.reserve(required);
    let flushed_size = encoder
        .flush::<FlushNoGap>(mp3_out.spare_capacity_mut())
        .map_err(|e| {
            error!("MP3 flush error: {e}");
            e.to_string()
        })?;
    unsafe {
        mp3_out.set_len(mp3_out.len() + flushed_size);
    }

    Ok(mp3_out)
}
