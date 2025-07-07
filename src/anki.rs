use crate::config::Anki;
use anyhow::{Result, anyhow};
use log::debug;
use reqwest::Client;
use serde_json::{Value, json};
#[derive(Clone)]
pub struct AnkiClient {
    pub client: Client,
    pub anki_url: String,
}

impl AnkiClient {
    pub fn new(cfg: &Anki) -> Self {
        Self {
            client: Client::new(),
            anki_url: cfg.anki_connect_url.to_string(),
        }
    }

    pub async fn get_latest_note_id(&self) -> Result<u64> {
        let request_body = json!({
            "action": "findNotes",
            "version": 6,
            "params": {
                "query": "added:1"
            }
        });

        let response = self
            .client
            .post(&self.anki_url)
            .header("Content-Type", "application/json; charset=UTF-8")
            .json(&request_body)
            .send()
            .await?;

        let data: Value = response.json().await?;
        let results = data["result"]
            .as_array()
            .ok_or_else(|| anyhow!("无法获取搜索结果"))?;

        if results.is_empty() {
            return Err(anyhow!("没有找到任何卡片"));
        }

        let mut note_ids: Vec<u64> = results.iter().filter_map(|v| v.as_u64()).collect();
        note_ids.sort_by(|a, b| b.cmp(a));
        note_ids
            .first()
            .copied()
            .ok_or_else(|| anyhow!("无法获取最新的卡片ID"))
    }

    pub async fn update_note_field(&self, note_id: u64, field: &str, value: &str) -> Result<()> {
        let request_body = json!({
            "action": "updateNoteFields",
            "version": 6,
            "params": {
                "note": {
                    "id": note_id,
                    "fields": {
                        field: value
                    }
                }
            }
        });
        let response = self
            .client
            .post(&self.anki_url)
            .header("Content-Type", "application/json; charset=UTF-8")
            .json(&request_body)
            .send()
            .await?;
        let data: Value = response.json().await?;
        if data["error"].is_null() {
            debug!("Note updated successfully: ID {note_id}, Field: {field}, Value: {value}");
            Ok(())
        } else {
            Err(anyhow!("Failed to update note: {}", data["error"]))
        }
    }

    pub async fn get_media_dir(&self) -> Result<String> {
        let request_body = json!({
            "action": "getMediaDirPath",
            "version": 6
        });
        let response: Value = self
            .client
            .post(&self.anki_url)
            .json(&request_body)
            .send()
            .await?
            .json()
            .await?;
        response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("Failed to get media directory"))
            .map(|s| s.to_string())
    }
}
