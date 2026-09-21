use crate::audio::protocol::{PlaySequenceRequest, TTS_SERVICE_DEFAULT_PORT};
use reqwest::Client;

#[derive(Clone)]
pub struct AudioServiceClient {
    base_url: String,
    client: Client,
}

impl Default for AudioServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioServiceClient {
    pub fn new() -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", TTS_SERVICE_DEFAULT_PORT),
            client: Client::new(),
        }
    }

    pub async fn request_play_sound(&self, audio_data: Vec<u8>, volume: f32) -> Result<(), String> {
        let url = format!("{}/sound?volume={}", self.base_url, volume);
        let resp = self
            .client
            .post(&url)
            .body(audio_data)
            .send()
            .await
            .map_err(|e| format!("바이너리 재생 요청 전송 실패: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!(
                "바이너리 재생 요청 실패 (status: {})",
                resp.status()
            ));
        }
        Ok(())
    }

    pub async fn request_set_volume(&self, volume: f32) -> Result<(), String> {
        let url = format!("{}/volume", self.base_url);
        let req_body = serde_json::json!({ "volume": volume });
        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| format!("볼륨 변경 요청 전송 실패: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("볼륨 변경 요청 실패 (status: {})", resp.status()));
        }
        Ok(())
    }

    pub async fn request_update_settings(
        &self,
        voice: &str,
        language: &str,
        speed: &str,
    ) -> Result<(), String> {
        let url = format!("{}/settings", self.base_url);
        let req_body = serde_json::json!({ "voice": voice, "language": language, "speed": speed});
        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| format!("설정 변경 요청 전송 실패: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!(
                "TTS 설정 변경 요청 실패 (status: {})",
                resp.status()
            ));
        }
        Ok(())
    }

    pub async fn request_audio_segments(
        &self,
        request: &PlaySequenceRequest,
    ) -> Result<(), String> {
        let url = format!("{}/audio_segments", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(request)
            .send()
            .await
            .map_err(|e| format!("시퀀스 재생 요청 전송 실패: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("시퀀스 재생 요청 실패 (status: {})", resp.status()));
        }
        Ok(())
    }
}
