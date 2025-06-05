use anyhow::{Context, Result};
use rdev::Key;
use std::fs;
use std::path::PathBuf;
pub mod key_parse;
mod types;

pub use types::*;

pub fn load_user_config() -> Result<Config> {
    let config_dir = get_config_directory()?;
    let config_file_path = config_dir.join("config.toml");

    // 确保配置目录存在
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;

    if !config_file_path.exists() {
        create_default_config(&config_file_path)?;
    }

    // 读取并解析配置文件
    let config_content = fs::read_to_string(&config_file_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_file_path))?;

    let config: Result<Config, toml::de::Error> = toml::from_str(&config_content);
    match config {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            // 解析失败，自动备份原配置并重建
            let bak_path = config_file_path.with_extension("bak");
            fs::rename(&config_file_path, &bak_path)
                .with_context(|| format!("Failed to backup old config to {:?}", bak_path))?;
            create_default_config(&config_file_path)?;
            let config_content = fs::read_to_string(&config_file_path).with_context(|| {
                format!("Failed to read new config file: {:?}", config_file_path)
            })?;
            let config: Config = toml::from_str(&config_content)
                .with_context(|| "Failed to parse new config file")?;
            log::warn!(
                "Config parse error: {}. Old config has been backed up to {:?}, new config created.",
                e,
                bak_path
            );
            Ok(config)
        }
    }
}

fn get_config_directory() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Windows: %LOCALAPPDATA%\Game2Anki
        if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
            Ok(PathBuf::from(local_appdata).join("Game2Anki"))
        } else {
            anyhow::bail!("LOCALAPPDATA environment variable not found")
        }
    }
}

fn create_default_config(config_path: &PathBuf) -> Result<()> {
    // 构造默认配置结构体
    let default_cfg = Config {
        hot_key: HotKey {
            screen_shot: vec![Key::CapsLock],
            audio_record: vec![Key::Tab],
        },
        screen_shot: Screenshot {
            format: ScreenshotFormat::Avif,
            quality: 60,
            speed: 6,
            exclude_title_bar: true,
            field_name: "Picture".to_string(), // Anki note field name for screenshot
        },
        audio_record: AudioRecord {
            format: AudioFormat::Opus,               // Audio format
            sample_rate: 48000, // opus Sampling rate of input signal (Hz) This must be one of 8000, 12000, 16000, 24000, or 48000.
            field_name: "SentenceAudio".to_string(), // Anki note field name for audio
        },
        anki: Anki {
            anki_connect_url: "http://127.0.0.1:8765".to_string(), // Anki Connect URL
        },
    };
    // 序列化为 TOML
    let default_content = toml::to_string_pretty(&default_cfg)
        .map_err(|e| anyhow::anyhow!("Failed to serialize default config: {}", e))?;
    fs::write(config_path, default_content)
        .with_context(|| format!("Failed to write default config to {:?}", config_path))?;
    log::info!("Created default config file at: {:?}", config_path);
    Ok(())
}
