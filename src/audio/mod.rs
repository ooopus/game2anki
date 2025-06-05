use crate::config::AudioFormat;
use crate::{anki::AnkiClient, config::AudioRecord};

use log::{debug, error, info};

mod encode;
use encode::encode;
use std::collections::VecDeque;
use std::error;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use wasapi::{Direction, SampleType, StreamMode, WaveFormat, get_default_device, initialize_mta};
use windows_capture::window::Window;
type Res<T> = Result<T, Box<dyn error::Error>>;

#[derive(Clone)]
pub struct AudioRecorder {
    is_recording: Arc<Mutex<bool>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    anki: Arc<AnkiClient>,
    cfg: AudioRecord,
}

impl AudioRecorder {
    pub fn new(cfg: AudioRecord, anki: Arc<AnkiClient>) -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            channels: 2,
            anki: anki,
            cfg: cfg,
        }
    }

    // 音频归一化
    fn normalize_audio(samples: &mut [f32]) {
        let max_amplitude = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);

        if max_amplitude > 0.0 && max_amplitude < 1.0 {
            let scale_factor = 0.95 / max_amplitude; // 留一点余量避免削波
            for sample in samples.iter_mut() {
                *sample *= scale_factor;
            }
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
                for i in 0..4 {
                    bytes[i] = sample_queue.pop_front().unwrap();
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
            let mut recording = self.is_recording.lock().unwrap();
            if *recording {
                return Err("Already recording".into());
            }
            *recording = true;
        }

        // 清空缓冲区
        {
            let mut buffer = self.audio_buffer.lock().unwrap();
            buffer.clear();
        }

        let is_recording = Arc::clone(&self.is_recording);
        let audio_buffer = Arc::clone(&self.audio_buffer);
        let sample_rate = self.cfg.sample_rate;
        let channels = self.channels;

        thread::Builder::new()
            .name("AudioCapture".to_string())
            .spawn(move || {
                if let Err(e) =
                    Self::capture_loop(is_recording, audio_buffer, sample_rate as usize, channels)
                {
                    error!("Audio capture loop failed: {}", e);
                }
            })?;

        info!("Recording started");
        Ok(())
    }

    // 停止录音并保存
    pub async fn stop_recording_and_save(&self) -> Res<()> {
        // 停止录音
        {
            let mut recording = self.is_recording.lock().unwrap();
            *recording = false;
        }

        // 等待一小段时间确保录音线程结束
        thread::sleep(std::time::Duration::from_millis(100));

        // 获取录音数据
        let audio_data = {
            let buffer = self.audio_buffer.lock().unwrap();
            buffer.clone()
        };

        if audio_data.is_empty() {
            return Err("No audio data recorded".into());
        }

        info!("Processing {} audio samples", audio_data.len());
        let mut normalized_data = audio_data;
        Self::normalize_audio(&mut normalized_data);
        // 裁剪两端静音（阈值可以根据需要调整）
        let trimmed = Self::trim_silence(&normalized_data, 0.01);
        // 若裁剪后为空则报错
        if trimmed.is_empty() {
            return Err("Audio is silent after trimming".into());
        }
        // 转换格式
        let _data = encode(
            self.cfg.format.clone(),
            &trimmed,
            self.cfg.sample_rate,
            self.channels,
        )?;
        let filename = self
            .generate_filename(&self.cfg.field_name, self.cfg.format.clone())
            .await;
        // 保存到Anki
        self.save_to_anki(_data, &filename).await?;
        info!("Recording saved as: {}", filename);
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
                &format!("[sound:{}]", filename),
            )
            .await?;

        info!("Audio saved to Anki note: {}", note_id);
        Ok(())
    }

    pub async fn generate_filename(&self, prefix: &str, ext: AudioFormat) -> String {
        let window_name = Self::get_foreground_window_name();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // 过滤掉非法字符
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
                error!("Failed to stop recording: {}", e);
            }
        });
    } else {
        // 当前未录音，开始录音
        info!("Starting recording...");
        recorder.start_recording()?;
    }

    Ok(())
}
