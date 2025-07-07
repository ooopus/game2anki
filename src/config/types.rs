use crate::utils::keyboard::keys_from_str_de;
use rdev::Key;
use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(rename = "hotKey")]
    pub hot_key: HotKey,

    #[serde(rename = "screenShot")]
    pub screen_shot: Screenshot,

    #[serde(rename = "audioRecord")]
    pub audio_record: AudioRecord,

    #[serde(rename = "anki")]
    pub anki: Anki,

    #[serde(rename = "logLevel")]
    pub log_level: LogLevel,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HotKey {
    #[serde(rename = "screenShot", deserialize_with = "keys_from_str_de")]
    pub screen_shot: Vec<Key>,

    #[serde(rename = "audioRecord", deserialize_with = "keys_from_str_de")]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Screenshot {
    #[serde(rename = "format")]
    pub format: ScreenshotFormat,

    #[serde(rename = "fieldName")]
    pub field_name: String,

    #[serde(rename = "quality")]
    pub quality: u8,

    #[serde(rename = "speed")]
    pub speed: u8,

    #[serde(rename = "excludeTitleBar")]
    pub exclude_title_bar: bool,
}

impl Default for Screenshot {
    fn default() -> Self {
        Self {
            format: ScreenshotFormat::Avif,
            field_name: "Picture".to_string(),
            quality: 60,
            speed: 6,
            exclude_title_bar: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioRecord {
    #[serde(rename = "format")]
    pub format: AudioFormat,

    #[serde(rename = "fieldName")]
    pub field_name: String,

    #[serde(rename = "sampleRate")]
    pub sample_rate: u32,
}

impl Default for AudioRecord {
    fn default() -> Self {
        Self {
            format: AudioFormat::Opus,
            field_name: "SentenceAudio".to_string(),
            sample_rate: 48000,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Anki {
    #[serde(rename = "ankiConnectUrl")]
    pub anki_connect_url: String,
}

impl Default for Anki {
    fn default() -> Self {
        Self {
            anki_connect_url: "http://127.0.0.1:8765".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AudioFormat {
    #[serde(rename = "opus")]
    Opus, // ogg Opus
    #[serde(rename = "mp3")]
    Mp3,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ScreenshotFormat {
    #[serde(rename = "avif")]
    Avif,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "png")]
    Png,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FileFormat {
    AudioFormat(AudioFormat),
    ScreenshotFormat(ScreenshotFormat),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LogLevel {
    #[serde(rename = "trace")]
    Trace,
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "error")]
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
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
