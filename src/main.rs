mod anki;
mod audio;
mod config;
mod hotkey_manager;
mod screenshot;
mod utils;
use std::sync::Arc;

use anki::AnkiClient;
use anyhow::Result;
use audio::AudioRecorder;
use hotkey_manager::HotKeyManager;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    let cfg = Arc::new(config::load_user_config()?);
    let anki = Arc::new(AnkiClient::new(&cfg.anki));

    hotkey_manager::HotKeyManager::init();
    setup_screenshot_hotkey(cfg.clone(), anki.clone());
    setup_audio_record_hotkey(cfg.clone(), anki.clone());

    log::info!("Application started. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;
    log::info!("Shutting down...");
    Ok(())
}

fn setup_screenshot_hotkey(cfg: Arc<config::Config>, anki: Arc<AnkiClient>) {
    let (screenshot_tx, mut screenshot_rx) = mpsc::channel(1);

    let screenshot_tool = screenshot::AnkiScreenshot::new(cfg.screen_shot.clone(), anki);

    HotKeyManager::register_hotkey(&cfg.hot_key.screen_shot, move || {
        if let Err(e) = screenshot_tx.try_send(()) {
            eprintln!("Failed to send screenshot signal: {}", e);
        }
    });

    tokio::spawn(async move {
        while let Some(_) = screenshot_rx.recv().await {
            if let Err(e) = screenshot_tool.on_hotkey_clicked().await {
                eprintln!("Failed to take screenshot: {}", e);
            }
        }
    });
}

fn setup_audio_record_hotkey(cfg: Arc<config::Config>, anki: Arc<AnkiClient>) {
    let (audio_tx, mut audio_rx) = mpsc::channel(1);
    let recorder = AudioRecorder::new(cfg.audio_record.clone(), anki);
    HotKeyManager::register_hotkey(&cfg.hot_key.audio_record, move || {
        if let Err(e) = audio_tx.try_send(()) {
            eprintln!("Failed to send audio record signal: {}", e);
        }
    });

    tokio::spawn(async move {
        let recorder = recorder; // move into async block
        while let Some(_) = audio_rx.recv().await {
            if let Err(e) = audio::on_hotkey_clicked(&recorder) {
                eprintln!("Failed to start recording: {}", e);
            }
        }
    });
}
