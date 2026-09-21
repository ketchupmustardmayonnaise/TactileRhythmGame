use config::{Config, ConfigError, Environment};
use directories::ProjectDirs;
use serde::Deserialize;
use std::path::PathBuf;

fn default_launcher_app() -> String {
    "launcher".to_string()
}

#[derive(Debug, Deserialize)]
pub struct RuntimeConfig {
    /// WASM 애플릿 디렉토리 경로 (선택 사항, 절대 경로 권장)
    pub applet_dir: Option<PathBuf>,
    #[serde(default = "default_launcher_app")]
    pub launcher_app: String,
}

impl RuntimeConfig {
    pub fn load() -> Result<Self, ConfigError> {
        // 런타임 환경 변수로만 설정 읽기
        let config = Config::builder()
            .add_source(
                Environment::with_prefix("MULTILINE_BRAILLE_DISPLAY_RUNTIME").separator("_"),
            )
            .build()?;

        // 구조체로 역직렬화
        config.try_deserialize()
    }

    /// 설정 파일 기반 검증을 거친 최종 애플릿 절대 경로 반환
    pub fn get_applet_dir(&self) -> PathBuf {
        if let Some(dir) = &self.applet_dir {
            if dir.is_absolute() {
                return dir.clone(); // 절대 경로만 덮어쓰기 허용
            } else {
                tracing::warn!(
                    "설정된 applet_dir({:?})가 절대 경로가 아닙니다. 기본 경로를 사용합니다.",
                    dir
                );
            }
        }
        Self::init_default_applet_dir()
    }

    /// OS의 표준 데이터 디렉토리 기반 디폴트 경로 반환
    pub fn init_default_applet_dir() -> PathBuf {
        // Windows: %APPDATA%\moredream\runtime\data\applets
        // macOS: ~/Library/Application Support/com.moredream.runtime/applets
        // Linux: ~/.local/share/moredream/applets
        let path = if let Some(proj_dirs) = ProjectDirs::from("com", "moredream", "runtime") {
            proj_dirs.data_dir().join("applets")
        } else {
            PathBuf::from("/opt/moredream/applets") // OS 경로 탐색 실패시 최후 폴백
        };

        // 폴더가 없으면 미리 만들어줍니다.
        if !path.exists() {
            if let Err(e) = std::fs::create_dir_all(&path) {
                tracing::error!(
                    "애플릿 기본 디렉토리를 생성하지 못했습니다: {:?} - {}",
                    path,
                    e
                );
            } else {
                tracing::info!("애플릿 기본 디렉토리를 생성했습니다: {:?}", path);
            }
        }

        path
    }

    /// OS의 표준 데이터 디렉토리 기반 디폴트 경로 반환
    pub fn init_default_settings_dir() -> PathBuf {
        // Windows: %APPDATA%\moredream\runtime\data\applets
        // macOS: ~/Library/Application Support/com.moredream.runtime/applets
        // Linux: ~/.local/share/moredream/applets
        let path = if let Some(proj_dirs) = ProjectDirs::from("com", "moredream", "runtime") {
            proj_dirs.data_dir().join("settings")
        } else {
            PathBuf::from("/opt/moredream/settings") // OS 경로 탐색 실패시 최후 폴백
        };

        // 폴더가 없으면 미리 만들어줍니다.
        if !path.exists() {
            if let Err(e) = std::fs::create_dir_all(&path) {
                tracing::error!(
                    "세팅 기본 디렉토리를 생성하지 못했습니다: {:?} - {}",
                    path,
                    e
                );
            } else {
                tracing::info!("세팅 기본 디렉토리를 생성했습니다: {:?}", path);
            }
        }

        path
    }
}
