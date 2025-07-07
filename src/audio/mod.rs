use crate::{
    anki::AnkiClient,
    config::AudioRecord,
    utils::{border::BorderOverlay, file::generate_safe_filename},
};
use log::{debug, error, info};
mod encode;
use encode::encode;
use std::{
    collections::VecDeque,
    error, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use wasapi::{Direction, SampleType, StreamMode, WaveFormat, get_default_device, initialize_mta};
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
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            channels: 2,
            anki,
            cfg,
            border: Arc::new(Mutex::new(None)),
        }
    }

    fn normalize_audio(samples: &mut [f32]) {
        let max_amplitude = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);

        // 设定合理的阈值范围，避免微调或过度增强
        if max_amplitude > 0.0 && !(0.7..=1.0).contains(&max_amplitude) {
            let scale_factor = 0.95 / max_amplitude.max(1e-6); // 避免除以0
            debug!("Normalizing audio with scale factor: {scale_factor}");

            for sample in samples.iter_mut() {
                *sample *= scale_factor;
            }
        } else {
            debug!("Skipping normalization. max_amplitude: {max_amplitude}");
        }
    }

    // 录音循环
    fn capture_loop(
        is_recording: Arc<Mutex<bool>>,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        sample_rate: usize,
        channels: u16,
    ) -> Res<()> {
        let device = get_default_device(&Direction::Render)?;
        let mut audio_client = device.get_iaudioclient()?;

        let desired_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate,
            channels.into(),
            None,
        );
        let blockalign = desired_format.get_blockalign();

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
            100 * blockalign as usize * (1024 + 2 * buffer_frame_count as usize),
        );

        audio_client.start_stream()?;
        debug!("Audio capture started");

        loop {
            let should_continue = {
                let recording = is_recording.lock().unwrap();
                *recording
            };

            if !should_continue {
                break;
            }

            render_client.read_from_device_to_deque(&mut sample_queue)?;

            // 转换字节数据为f32样本
            while sample_queue.len() >= 4 {
                // 4 bytes per f32 sample
                let mut bytes = [0u8; 4];
                for byte in &mut bytes {
                    *byte = sample_queue.pop_front().unwrap();
                }
                let sample = f32::from_le_bytes(bytes);

                let mut buffer = audio_buffer.lock().unwrap();
                buffer.push(sample);
            }

            if h_event.wait_for_event(100).is_err() {
                // Short timeout to check recording status frequently
                continue;
            }
        }

        audio_client.stop_stream()?;
        debug!("Audio capture stopped");
        Ok(())
    }

    // 开始录音
    pub fn start_recording(&self) -> Res<()> {
        let _ = initialize_mta();
        {
            let mut rec = self.is_recording.lock().unwrap();
            if *rec {
                return Err("Already recording".into());
            }
            *rec = true;
        }
        // 清空缓冲
        self.audio_buffer.lock().unwrap().clear();

        let new_border = BorderOverlay::new()?;
        *self.border.lock().unwrap() = Some(new_border);

        // 启动录音线程
        let is_rec = Arc::clone(&self.is_recording);
        let audio_buf = Arc::clone(&self.audio_buffer);
        let sr = self.cfg.sample_rate as usize;
        let ch = self.channels;
        thread::Builder::new()
            .name("AudioCapture".into())
            .spawn(move || {
                if let Err(e) = Self::capture_loop(is_rec, audio_buf, sr, ch) {
                    error!("Audio capture loop failed: {e}");
                }
            })?;
        Ok(())
    }

    // 停止录音并保存
    pub async fn stop_recording_and_save(&self) -> Res<()> {
        *self.is_recording.lock().unwrap() = false;

        if let Some(border_to_stop) = self.border.lock().unwrap().take() {
            border_to_stop.stop();
        }

        // 等待录音线程真正退出
        thread::sleep(std::time::Duration::from_millis(100));

        // 获取并处理音频数据
        let mut data = {
            let buf = self.audio_buffer.lock().unwrap();
            buf.clone()
        };
        if data.is_empty() {
            return Err("No audio data recorded".into());
        }
        Self::normalize_audio(&mut data);
        let trimmed = Self::trim_silence(&data, 0.01);
        if trimmed.is_empty() {
            return Err("Audio is silent after trimming".into());
        }

        // 编码、保存并更新 Anki
        let raw = encode(
            self.cfg.format.clone(),
            trimmed,
            self.cfg.sample_rate,
            self.channels,
        )?;
        let fname = generate_safe_filename(&self.cfg.field_name, &self.cfg.format.to_string());
        self.save_to_anki(raw, &fname).await?;
        info!("Recording saved as: {fname}");
        Ok(())
    }

    // 保存到Anki
    async fn save_to_anki(&self, _data: Vec<u8>, filename: &str) -> Res<()> {
        // 获取媒体目录并保存文件
        let media_dir = self.anki.get_media_dir().await?;
        let file_path = PathBuf::from(&media_dir).join(filename);
        fs::write(&file_path, _data)?;
        info!("Audio file saved to: {}", file_path.display());

        // 更新最新的卡片
        let note_id = self.anki.get_latest_note_id().await?;
        self.anki
            .update_note_field(
                note_id,
                &self.cfg.field_name,
                &format!("[sound:{filename}]"),
            )
            .await?;

        info!("Audio saved to Anki note: {note_id}");
        Ok(())
    }

    fn trim_silence(samples: &[f32], threshold: f32) -> &[f32] {
        let start = samples
            .iter()
            .position(|&x| x.abs() > threshold)
            .unwrap_or(0);

        let end = samples
            .iter()
            .rposition(|&x| x.abs() > threshold)
            .map(|x| x + 1) // 包含最后一个有效采样点
            .unwrap_or(samples.len());

        &samples[start..end]
    }
}

// 热键处理函数 - 将被外部调用
pub fn on_hotkey_clicked(recorder: &AudioRecorder) -> Res<()> {
    let is_currently_recording = {
        let recording = recorder.is_recording.lock().unwrap();
        *recording
    };

    if is_currently_recording {
        // 当前正在录音，停止录音并保存
        info!("Stopping recording...");
        let recorder_clone = recorder.clone();
        tokio::spawn(async move {
            if let Err(e) = recorder_clone.stop_recording_and_save().await {
                error!("Failed to stop recording: {e}");
            }
        });
    } else {
        // 当前未录音，开始录音
        info!("Starting recording...");
        recorder.start_recording()?;
    }

    Ok(())
}
