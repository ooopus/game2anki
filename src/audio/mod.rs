// src/audio/mod.rs

use crate::{
    anki::AnkiClient,
    config::{AudioFormat, AudioRecord, MediaConfig},
    utils::{border::BorderOverlay, file::generate_safe_filename},
};
use log::{debug, error, info, warn};
use std::{
    collections::VecDeque,
    error,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};
use tokio;
use wasapi::{Direction, SampleType, StreamMode, WaveFormat, get_default_device, initialize_mta};

mod encode;
use encode::encode_to_file;

type Res<T> = Result<T, Box<dyn error::Error>>;

#[derive(Clone)]
pub struct AudioRecorder {
    is_recording: Arc<Mutex<bool>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    anki: Arc<AnkiClient>,
    cfg: AudioRecord,
    border: Arc<Mutex<Option<BorderOverlay>>>,
}

impl AudioRecorder {
    pub fn new(cfg: AudioRecord, anki: Arc<AnkiClient>) -> Self {
        debug!("AudioRecorder initialized with config: {:?}", cfg);
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            channels: 2,
            anki,
            cfg,
            border: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_recording(&self) -> Res<()> {
        let _ = initialize_mta();
        {
            let mut rec = self.is_recording.lock().unwrap();
            if *rec {
                warn!("Attempted to start recording, but it's already in progress.");
                return Err("Already recording".into());
            }
            *rec = true;
        }
        self.audio_buffer.lock().unwrap().clear();

        let new_border = BorderOverlay::new()?;
        *self.border.lock().unwrap() = Some(new_border);

        let is_rec = Arc::clone(&self.is_recording);
        let audio_buf = Arc::clone(&self.audio_buffer);

        // 从配置中获取采样率
        let sr = match &self.cfg.format {
            AudioFormat::Opus => self.cfg.opus.sample_rate as usize,
            AudioFormat::Mp3 => self.cfg.mp3.sample_rate as usize,
        };
        let ch = self.channels;

        thread::Builder::new()
            .name("AudioCapture".into())
            .spawn(move || {
                if let Err(e) = Self::capture_loop(is_rec, audio_buf, sr, ch) {
                    error!("Audio capture loop failed: {e}");
                }
            })?;
        debug!("Audio capture thread spawned.");
        Ok(())
    }

    pub async fn stop_recording_and_save(&self) -> Res<()> {
        debug!("Stop-and-save process initiated.");
        *self.is_recording.lock().unwrap() = false;
        debug!("is_recording flag set to false.");

        if let Some(border_to_stop) = self.border.lock().unwrap().take() {
            border_to_stop.stop();
            debug!("Recording border removed.");
        }

        debug!("Waiting 200ms for capture thread to receive final audio chunks...");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut data = self.audio_buffer.lock().unwrap().clone();
        debug!("Cloned audio buffer. Total raw samples: {}", data.len());

        if data.is_empty() {
            warn!("No audio data was recorded, aborting save.");
            return Err("No audio data recorded".into());
        }

        Self::normalize_audio(&mut data);

        let before_len = data.len();
        let trimmed = Self::trim_silence(&data, 0.01);
        debug!(
            "Trimming silence. Samples before: {}, Samples after: {}",
            before_len,
            trimmed.len()
        );

        if trimmed.is_empty() {
            warn!("Audio was silent after trimming, aborting save.");
            return Err("Audio is silent after trimming".into());
        }

        let file_name = generate_safe_filename(&self.cfg);
        debug!("Generated safe filename: {}", file_name);

        let media_dir = self.anki.get_media_dir().await?;
        let final_path = Path::new(&media_dir).join(&file_name);
        debug!("Final output path set to: {}", final_path.display());

        info!(
            "Encoding {} samples to '{}' using {} format...",
            trimmed.len(),
            final_path.display(),
            self.cfg.format()
        );

        encode_to_file(&self.cfg, trimmed, self.channels, &final_path)?;
        info!("Successfully encoded and saved recording.");

        let note_id = self.anki.get_latest_note_id().await?;
        debug!(
            "Updating Anki note ID {} with field '{}' and value '[sound:{}]'",
            note_id,
            self.cfg.field_name(),
            file_name
        );
        self.anki
            .update_note_field(
                note_id,
                self.cfg.field_name(),
                &format!("[sound:{}]", file_name),
            )
            .await?;
        info!("Audio saved to Anki note: {note_id}");

        Ok(())
    }

    fn normalize_audio(samples: &mut [f32]) {
        let max_amplitude = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if max_amplitude > 0.0 && !(0.7..=1.0).contains(&max_amplitude) {
            let scale_factor = 0.95 / max_amplitude.max(1e-6);
            debug!("Normalizing audio with scale factor: {scale_factor}");
            for sample in samples.iter_mut() {
                *sample *= scale_factor;
            }
        } else {
            debug!("Skipping normalization. max_amplitude: {max_amplitude}");
        }
    }

    fn trim_silence(samples: &[f32], threshold: f32) -> &[f32] {
        let start = samples
            .iter()
            .position(|&x| x.abs() > threshold)
            .unwrap_or(0);
        let end = samples
            .iter()
            .rposition(|&x| x.abs() > threshold)
            .map(|x| x + 1)
            .unwrap_or(samples.len());
        &samples[start..end]
    }

    fn capture_loop(
        is_recording: Arc<Mutex<bool>>,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        sample_rate: usize,
        channels: u16,
    ) -> Res<()> {
        let device = get_default_device(&Direction::Render)?;
        debug!("Using default audio device: {}", device.get_friendlyname()?);
        let mut audio_client = device.get_iaudioclient()?;
        let desired_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate,
            channels.into(),
            None,
        );
        debug!(
            "Initializing wasapi client with format: {:?}",
            desired_format
        );

        let block_align = desired_format.get_blockalign();
        let (_def_time, min_time) = audio_client.get_device_period()?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: min_time,
        };
        audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;
        let h_event = audio_client.set_get_eventhandle()?;
        let buffer_frame_count = audio_client.get_buffer_size()?;
        let render_client = audio_client.get_audiocaptureclient()?;
        let mut sample_queue: VecDeque<u8> = VecDeque::with_capacity(
            100 * block_align as usize * (1024 + 2 * buffer_frame_count as usize),
        );

        audio_client.start_stream()?;
        debug!("Audio capture started");

        loop {
            if !*is_recording.lock().unwrap() {
                debug!("Capture loop detected stop signal.");
                break;
            }
            if h_event.wait_for_event(100).is_err() {
                continue;
            }
            render_client.read_from_device_to_deque(&mut sample_queue)?;
            while sample_queue.len() >= 4 {
                let mut bytes = [0u8; 4];
                for byte in &mut bytes {
                    *byte = sample_queue.pop_front().unwrap();
                }
                audio_buffer.lock().unwrap().push(f32::from_le_bytes(bytes));
            }
        }

        audio_client.stop_stream()?;
        debug!("Audio capture stream stopped gracefully.");
        Ok(())
    }
}

pub fn on_hotkey_clicked(recorder: &AudioRecorder) -> Res<()> {
    let is_currently_recording = *recorder.is_recording.lock().unwrap();
    if is_currently_recording {
        info!("Stopping recording...");
        let recorder_clone = recorder.clone();
        tokio::spawn(async move {
            if let Err(e) = recorder_clone.stop_recording_and_save().await {
                error!("Failed to stop and save recording: {e:?}");
            }
        });
    } else {
        info!("Starting recording...");
        recorder.start_recording()?;
    }
    Ok(())
}
