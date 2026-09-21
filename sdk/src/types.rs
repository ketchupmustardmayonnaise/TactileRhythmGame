use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};
/// TTS 음성 속도 단계
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
pub enum Speed {
    Slow = 0,
    #[default]
    Normal = 1,
    Fast = 2,
}

impl Speed {
    /// 실제 TTS 엔진(또는 오디오 서비스)에 요청할 때 사용할 실수형 배율로 변환합니다.
    /// (값은 예시이므로 실제 엔진에 맞게 조절하시면 됩니다)
    pub fn as_f32(&self) -> f32 {
        match self {
            Speed::Slow => 0.7,
            Speed::Normal => 1.0,
            Speed::Fast => 1.05,
        }
    }
}

impl From<i32> for Speed {
    fn from(value: i32) -> Self {
        match value {
            0 => Speed::Slow,
            1 => Speed::Normal,
            2 => Speed::Fast,
            _ => Speed::Normal,
        }
    }
}

/// 지원하는 언어 목록
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, MaxSize)]
#[repr(i32)]
pub enum Language {
    #[default]
    Ko = 0,
    En = 1,
    Ja = 2,
}

impl From<i32> for Language {
    fn from(value: i32) -> Self {
        match value {
            0 => Language::Ko,
            1 => Language::En,
            2 => Language::Ja,
            _ => Language::Ko,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
#[repr(i32)]
/// 지원하는 목소리 목록
pub enum Voice {
    #[default]
    M1,
    M2,
    M3,
    M4,
    M5,
    F1,
    F2,
    F3,
    F4,
    F5,
}

impl From<i32> for Voice {
    fn from(value: i32) -> Self {
        match value {
            0 => Voice::M1,
            1 => Voice::M2,
            2 => Voice::M3,
            3 => Voice::M4,
            4 => Voice::M5,
            5 => Voice::F1,
            6 => Voice::F2,
            7 => Voice::F3,
            8 => Voice::F4,
            9 => Voice::F5,
            _ => Voice::M1,
        }
    }
}

/// 호스트(서버)로부터 받아올 TTS 설정 정보입니다.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TtsConfigTypes {
    pub voice: Vec<Voice>,
    pub language: Vec<Language>,
}

/// 사용자가 직접 선택한 TTS 개별 설정입니다.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TtsSettings {
    pub voice: Voice,
    pub speed: Speed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub tts: TtsSettings,
    pub language: Language,
    pub volume: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            tts: TtsSettings::default(),
            language: Language::default(),
            volume: 50.0,
        }
    }
}

