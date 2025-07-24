// src/config/types.rs

use crate::utils::keyboard::keys_from_str_de;
use rdev::Key;
use serde::{Deserialize, Serialize};
use std::fmt;

// --- 通用媒体配置特性 ---
pub trait MediaConfig {
    type Format: fmt::Display;
    fn field_name(&self) -> &str;
    fn format(&self) -> &Self::Format;
}

// --- 截图格式的具体编码配置 ---

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AvifEncodeConfig {
    pub crf: u8, // CRF ↓ → 质量 ↑ → 文件体积 ↑
    #[serde(rename = "cpuUsed")]
    pub cpu_used: u8, // -cpu-used ↓ → 编码速度 ↓ → 压缩效率 ↑（输出更小、质量更高）
    pub encoder: String,
}

impl Default for AvifEncodeConfig {
    fn default() -> Self {
        Self {
            crf: 46,
            cpu_used: 4,
            encoder: "libaom-av1".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebpEncodeConfig {
    pub quality: u8,
}

impl Default for WebpEncodeConfig {
    fn default() -> Self {
        Self { quality: 30 }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PngEncodeConfig {
    #[serde(rename = "compressionLevel")]
    pub compression_level: u8,
}

impl Default for PngEncodeConfig {
    fn default() -> Self {
        Self {
            compression_level: 6,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OpusEncodeConfig {
    #[serde(rename = "sampleRate")]
    pub sample_rate: u32,
    #[serde(rename = "bitRate")]
    pub bit_rate: u32,
}

impl Default for OpusEncodeConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            bit_rate: 32,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mp3EncodeConfig {
    #[serde(rename = "sampleRate")]
    pub sample_rate: u32,
    #[serde(rename = "bitRate")]
    pub bit_rate: u32,
    pub quality: u8,
}

impl Default for Mp3EncodeConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            bit_rate: 64,
            quality: 2,
        }
    }
}

// --- 主配置结构体 ---

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub hot_key: HotKey,
    pub screen_shot: Screenshot,
    pub audio_record: AudioRecord,
    pub anki: Anki,
    pub log_level: LogLevel,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotKey {
    #[serde(deserialize_with = "keys_from_str_de")]
    pub screen_shot: Vec<Key>,
    #[serde(deserialize_with = "keys_from_str_de")]
    pub audio_record: Vec<Key>,
}

impl Default for HotKey {
    fn default() -> Self {
        Self {
            screen_shot: vec![Key::CapsLock],
            audio_record: vec![Key::Tab],
        }
    }
}

// ---截图配置结构 ---
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub field_name: String,
    pub format: ScreenshotFormat,
    pub exclude_title_bar: bool,
    pub avif: AvifEncodeConfig,
    pub webp: WebpEncodeConfig,
    pub png: PngEncodeConfig,
}

impl Default for Screenshot {
    fn default() -> Self {
        Self {
            field_name: "Picture".to_string(),
            format: ScreenshotFormat::Avif,
            exclude_title_bar: true,
            avif: AvifEncodeConfig::default(),
            webp: WebpEncodeConfig::default(),
            png: PngEncodeConfig::default(),
        }
    }
}

impl MediaConfig for Screenshot {
    type Format = ScreenshotFormat;
    fn field_name(&self) -> &str {
        &self.field_name
    }
    fn format(&self) -> &Self::Format {
        &self.format
    }
}

// ---录音配置结构 ---
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRecord {
    pub field_name: String,
    pub format: AudioFormat,
    pub opus: OpusEncodeConfig,
    pub mp3: Mp3EncodeConfig,
}

impl Default for AudioRecord {
    fn default() -> Self {
        Self {
            field_name: "SentenceAudio".to_string(),
            format: AudioFormat::Opus,
            opus: OpusEncodeConfig::default(),
            mp3: Mp3EncodeConfig::default(),
        }
    }
}

impl MediaConfig for AudioRecord {
    type Format = AudioFormat;
    fn field_name(&self) -> &str {
        &self.field_name
    }
    fn format(&self) -> &Self::Format {
        &self.format
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Anki {
    pub anki_connect_url: String,
}

impl Default for Anki {
    fn default() -> Self {
        Self {
            anki_connect_url: "http://127.0.0.1:8765".to_string(),
        }
    }
}

// --- 枚举 ---

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Opus,
    Mp3,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScreenshotFormat {
    Avif,
    Webp,
    Png,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioFormat::Opus => write!(f, "opus"),
            AudioFormat::Mp3 => write!(f, "mp3"),
        }
    }
}

impl fmt::Display for ScreenshotFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScreenshotFormat::Avif => write!(f, "avif"),
            ScreenshotFormat::Webp => write!(f, "webp"),
            ScreenshotFormat::Png => write!(f, "png"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level_str = match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        };
        write!(f, "{}", level_str)
    }
}

impl From<LogLevel> for log::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => log::Level::Trace,
            LogLevel::Debug => log::Level::Debug,
            LogLevel::Info => log::Level::Info,
            LogLevel::Warn => log::Level::Warn,
            LogLevel::Error => log::Level::Error,
        }
    }
}
