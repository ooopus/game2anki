use super::key_parse::keys_from_str_de;
use rdev::Key;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(rename = "hotKey")]
    pub hot_key: HotKey,

    #[serde(rename = "screenShot")]
    pub screen_shot: Screenshot,

    #[serde(rename = "audioRecord")]
    pub audio_record: AudioRecord,

    #[serde(rename = "anki")]
    pub anki: Anki,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HotKey {
    #[serde(rename = "screenShot", deserialize_with = "keys_from_str_de")]
    pub screen_shot: Vec<Key>,

    #[serde(rename = "audioRecord", deserialize_with = "keys_from_str_de")]
    pub audio_record: Vec<Key>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioRecord {
    #[serde(rename = "format")]
    pub format: AudioFormat,

    #[serde(rename = "fieldName")]
    pub field_name: String,

    #[serde(rename = "sampleRate")]
    pub sample_rate: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Anki {
    #[serde(rename = "ankiConnectUrl")]
    pub anki_connect_url: String,
}

#[derive(Clone, Debug, Deserialize, EnumString, Serialize)]
#[strum(serialize_all = "snake_case")]
pub enum AudioFormat {
    #[serde(rename = "opus")]
    Opus, // ogg Opus
    #[serde(rename = "mp3")]
    Mp3,
}

#[derive(Clone, Debug, Deserialize, Serialize, EnumString)]
#[strum(serialize_all = "snake_case")]
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
