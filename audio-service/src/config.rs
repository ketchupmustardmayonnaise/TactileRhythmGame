use std::env;
use std::path::PathBuf;
use typed_builder::TypedBuilder;

use crate::error::Result;

pub const DEFAULT_PORT: u16 = 3005;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ServiceConfig {
    #[builder(default = DEFAULT_PORT)]
    pub port: u16,
    #[builder(default = ServiceConfig::default_piper_asset_dir())]
    pub piper_asset_dir: PathBuf,
}

impl ServiceConfig {
    fn default_piper_asset_dir() -> PathBuf {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        env::var("PIPER_ASSET_DIR")
            .or_else(|_| env::var("TTS_ASSET_DIR"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let audio_service_assets = current_dir.join("audio-service").join("assets");
                if audio_service_assets.exists() {
                    audio_service_assets
                } else {
                    current_dir.join("assets")
                }
            })
    }

    pub fn from_env() -> Result<Self> {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);

        let piper_asset_dir = Self::default_piper_asset_dir();
        Self::ensure_espeak_ng_data_dir();

        Ok(ServiceConfig::builder()
            .port(port)
            .piper_asset_dir(piper_asset_dir)
            .build())
    }

    fn ensure_espeak_ng_data_dir() {
        if let Ok(dir_str) = env::var("PIPER_ESPEAKNG_DATA_DIRECTORY") {
            let path = PathBuf::from(&dir_str);
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    tracing::info!(
                        "PIPER_ESPEAKNG_DATA_DIRECTORY 환경변수가 파일({:?})을 가리키고 있어 디렉터리({:?})로 보정합니다.",
                        path,
                        parent
                    );
                    unsafe {
                        env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", parent);
                    }
                    return;
                }
            } else if path.exists() {
                return;
            }
        }

        let candidates = [
            "/usr/share/espeak-ng-data",
            "/usr/lib/espeak-ng-data",
            "/usr/lib/aarch64-linux-gnu/espeak-ng-data",
            "/usr/lib/x86_64-linux-gnu/espeak-ng-data",
            "/opt/homebrew/share/espeak-ng-data",
            "/usr/local/share/espeak-ng-data",
            "audio-service/assets/espeak-ng-data",
            "assets/espeak-ng-data",
        ];

        for candidate in candidates {
            let p = PathBuf::from(candidate);
            if p.exists() && p.is_dir() {
                tracing::info!("PIPER_ESPEAKNG_DATA_DIRECTORY 감지됨: {:?}", p);
                unsafe {
                    env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &p);
                }
                return;
            }
        }

        tracing::warn!(
            "espeak-ng-data 디렉토리를 자동으로 찾을 수 없습니다. PIPER_ESPEAKNG_DATA_DIRECTORY 환경변수를 설정해 주세요."
        );
    }
}
