use crate::{Error, Result};

const STORAGE_KEY_APP_SETTINGS: &str = "app_settings";

pub struct Storage {
    inner: web_sys::Storage,
}

impl Storage {
    pub fn new() -> Result<Self> {
        let inner = web_sys::window()
            .ok_or(Error::NoGlobalWindowExistsError)?
            .local_storage()
            .map_err(|_| Error::StorageError("Failed to get local_storage".into()))?
            .ok_or(Error::StorageError("local_storage is None".into()))?;
        Ok(Self { inner })
    }

    /// 사용자 설정을 JSON 문자열로 직렬화하여 로컬 스토리지에 저장합니다.
    pub fn save_app_settings(&self, settings: &sdk::types::AppSettings) -> Result<()> {
        // 기존 설정을 불러와 변경 사항이 있는지 확인합니다.
        if let Ok(Some(json_str)) = self.inner.get_item(STORAGE_KEY_APP_SETTINGS)
            && let Ok(existing_settings) =
                serde_json::from_str::<sdk::types::AppSettings>(&json_str)
            && existing_settings == *settings
        {
            return Ok(()); // 값이 같으면 불필요한 스토리지 쓰기를 생략합니다.
        }

        let json = serde_json::to_string(settings)
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        self.inner
            .set_item(STORAGE_KEY_APP_SETTINGS, &json)
            .map_err(|_| Error::StorageError("Failed to set app settings in storage".into()))
    }

    /// 스토리지에서 사용자 설정을 불러옵니다.
    /// 값이 비어있거나, 구조가 변경되어 파싱에 실패하면 기본값으로 새로 생성하여 덮어씁니다.
    pub fn get_app_settings(&self) -> Result<sdk::types::AppSettings> {
        let item = self.inner.get_item(STORAGE_KEY_APP_SETTINGS);

        let mut settings: sdk::types::AppSettings = if let Ok(Some(json_str)) = item {
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            sdk::types::AppSettings::default()
        };

        if settings.volume <= 9.0 && settings.volume > 0.0 {
            settings.volume *= 10.0;
        }
        if !(10.0..=100.0).contains(&settings.volume) {
            settings.volume = 50.0;
        }

        Ok(settings)
    }
}
