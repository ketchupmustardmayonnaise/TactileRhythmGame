use serde::{Deserialize, Serialize};

use crate::TtsConfig;

/// SDK 내 설정 객체들이 공통으로 구현할 트레이트입니다.
/// 호스트나 서버에서 전달받은 새로운 설정값으로 기존 상태를 덮어쓰거나 병합합니다.
pub trait Config {
    fn apply_config(&mut self, new_config: Self);
}

impl Config for TtsConfig {
    fn apply_config(&mut self, new_config: Self) {
        // 서버에서 받아온 값으로 기존 설정을 완전히 덮어씁니다.
        self.voice = new_config.voice;
        self.language = new_config.language;
    }
}

/// 애플릿의 전체 설정을 관리하는 구조체입니다.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub tts_configs: TtsConfig,
}

impl Config for AppConfig {
    fn apply_config(&mut self, new_config: Self) {
        // 전체 TTS 설정을 덮어씁니다.
        self.tts_configs = new_config.tts_configs;
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tts_config(&mut self, config: TtsConfig) {
        self.tts_configs.voice.extend(config.voice);
        self.tts_configs.language.extend(config.language);
    }
}
