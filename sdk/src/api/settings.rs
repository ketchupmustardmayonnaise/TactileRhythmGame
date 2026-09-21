use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    bridge::host_functions::{settings_get, settings_set},
    error::{Error, Result},
};

pub const TTS_VOICE: &str = "tts_voice";
pub const LANGUAGE: &str = "language";
pub const AUDIO_VOLUME: &str = "audio_volume";
pub const SPEECH_RATE: &str = "speech_rate";

const INITIAL_BUFFER_SIZE: usize = 1024;
const MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

/// 필요한 설정값이나 상태 데이터를 저장하고 불러오는 데 사용
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    options: SettingsOptions,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct SettingsOptions {
    pub items: IndexMap<String, SettingOption>,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct SettingOption {
    pub default: Option<String>,
    pub value: Option<String>,
    pub value_list: IndexMap<String, String>,
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    /// 문자열(String) 데이터를 저장합니다.
    pub fn set_value<S: Into<String>>(&mut self, key: S, value: S) -> Result<()> {
        let key = key.into();
        let value = value.into();

        // 스키마(options)에 정의된 키라면, 허용된 값 목록에 있는지 유효성을 검증합니다.
        if let Some(option) = self.options.items.get_mut(&key) {
            if !option.value_list.is_empty() && !option.value_list.values().any(|v| v == &value) {
                return Err(Error::InvalidValue(value.clone()));
            }
            // 캐시된 스키마의 value 값도 즉시 업데이트하여 이후 get_value 시 바로 반영되도록 합니다.
            option.value = Some(value.clone());
        }

        // 호스트 스토리지에 영구 저장하는 로직
        let result = unsafe { settings_set(key.as_ptr(), key.len(), value.as_ptr(), value.len()) };
        if result < 0 {
            return Err(Error::FailedToSetPreference(key));
        }

        Ok(())
    }

    /// 저장된 문자열(String) 데이터를 불러옵니다.
    pub fn get_value<S: Into<String>>(&self, key: S) -> Result<String> {
        let key_str = key.into();

        // 1. 스키마(options)에 키가 정의되어 있다면 스키마의 우선순위를 따릅니다.
        if let Some(option) = self.options.items.get(&key_str) {
            // 최우선 순위: 사용자가 선택한 값 (value)
            if let Some(val) = &option.value
                && !val.is_empty()
            {
                return Ok(val.clone());
            }
            // 차순위: 기본값 (default)
            if let Some(default_value) = &option.default
                && !default_value.is_empty()
            {
                return Ok(default_value.clone());
            }
        }

        // 2. 스키마에 없는 키라면 기존처럼 호스트 스토리지에서 직접 조회합니다.
        let mut buffer = vec![0u8; INITIAL_BUFFER_SIZE];

        loop {
            let result = unsafe {
                settings_get(
                    key_str.as_ptr(),
                    key_str.len(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                )
            };

            if result >= 0 {
                let len = result as usize;
                return Ok(String::from_utf8_lossy(&buffer[..len]).into_owned());
            }

            match result {
                -1 => return Err(Error::KeyNotFound(key_str)),
                -2 => {
                    let current_len = buffer.len();
                    if current_len >= MAX_BUFFER_SIZE {
                        return Err(Error::BufferTooSmall);
                    }
                    buffer.resize(current_len * 2, 0);
                }
                _ => return Err(Error::FailedToGetPreference(key_str)),
            }
        }
    }

    /// 호스트로부터 지원 가능한 앱 설정 스키마(옵션) 목록을 가져옵니다.
    pub fn get_options(&mut self) -> Result<SettingsOptions> {
        let key_str = "app_settings_options";
        let mut buffer = vec![0u8; INITIAL_BUFFER_SIZE];

        loop {
            let result = unsafe {
                settings_get(
                    key_str.as_ptr(),
                    key_str.len(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                )
            };

            if result >= 0 {
                let len = result as usize;
                let json_str = String::from_utf8_lossy(&buffer[..len]);
                let options: SettingsOptions = serde_json::from_str(&json_str).map_err(|e| {
                    Error::InvalidValue(format!("Failed to parse SettingsOptions: {}", e))
                })?;
                self.options = options.clone();
                return Ok(options);
            }

            match result {
                -1 => return Err(Error::KeyNotFound(key_str.to_string())),
                -2 => {
                    let current_len = buffer.len();
                    if current_len >= MAX_BUFFER_SIZE {
                        return Err(Error::BufferTooSmall);
                    }
                    buffer.resize(current_len * 2, 0);
                }
                _ => return Err(Error::FailedToGetPreference(key_str.to_string())),
            }
        }
    }

    /// 호스트로부터 지원 가능한 언어 및 목소리 설정 목록을 가져옵니다.
    pub fn get_tts_config_types(&self) -> Result<crate::types::TtsConfigTypes> {
        let key_str = "tts_config_types";
        let mut buffer = vec![0u8; INITIAL_BUFFER_SIZE];

        loop {
            let result = unsafe {
                settings_get(
                    key_str.as_ptr(),
                    key_str.len(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                )
            };

            if result >= 0 {
                let len = result as usize;
                let json_str = String::from_utf8_lossy(&buffer[..len]);
                let config: crate::types::TtsConfigTypes = serde_json::from_str(&json_str)
                    .map_err(|e| {
                        Error::InvalidValue(format!("Failed to parse TtsConfigTypes: {}", e))
                    })?;
                return Ok(config);
            }

            match result {
                -1 => return Err(Error::KeyNotFound(key_str.to_string())),
                -2 => {
                    let current_len = buffer.len();
                    if current_len >= MAX_BUFFER_SIZE {
                        return Err(Error::BufferTooSmall);
                    }
                    buffer.resize(current_len * 2, 0);
                }
                _ => return Err(Error::FailedToGetPreference(key_str.to_string())),
            }
        }
    }
}
