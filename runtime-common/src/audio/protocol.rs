use sdk::Language;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub const TTS_SERVICE_DEFAULT_PORT: u16 = 3005;
pub const TTS_SERVICE_MAX_TEXT_SIZE: usize = 16 * 1024;
pub const TTS_SERVICE_MAX_AUDIO_SIZE: usize = 5 * 1024 * 1024;

/// audio-service 서버로부터 받는 응답(Response) 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsResponse {
    pub success: bool,
    pub message: Option<String>,
}

/// 오디오 시퀀스 재생을 위한 세그먼트 정의
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SequenceSegment {
    Text(String),
    Sound(Vec<u8>, f32),
    Silence(u32),
}

/// 한 번의 요청으로 텍스트, 효과음, 묵음을 이어서 재생하도록 서버에 요청하는 구조체
#[derive(Serialize, Deserialize, Clone, Debug, TypedBuilder)]
pub struct PlaySequenceRequest {
    pub segments: Vec<SequenceSegment>,
    #[builder(default = 1.0)]
    pub speed: f32,
    #[serde(rename = "voiceType")]
    #[builder(default = "F1".to_string())]
    pub voice_type: String,
    #[serde(default)]
    #[builder(default = false)]
    pub is_important: bool,
    #[serde(default)]
    #[builder(default = false)]
    pub is_forced: bool,
    /// 재생 타겟 언어 (기본값: 한국어)
    #[serde(default)]
    #[builder(default = Language::default())]
    pub language: Language,
}
