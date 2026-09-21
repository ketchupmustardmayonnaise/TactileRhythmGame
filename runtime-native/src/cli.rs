use crate::config::RuntimeConfig;
use anyhow::Result;
use clap::Parser;

/// CLI 인자를 파싱하기 위한 구조체 정의 (clap 라이브러리 사용)
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// 디스플레이의 너비를 지정합니다.
    #[arg(long, default_value_t = 48)]
    pub width: i16,

    /// 디스플레이의 높이를 지정합니다.
    #[arg(long, default_value_t = 32)]
    pub height: i16,

    /// 애플릿의 언어 설정을 지정합니다.
    #[arg(long, value_enum, default_value_t = Language::Ko)]
    pub lang: Language,

    /// 설정된 애플릿 기본 디렉토리 경로를 출력하고 종료합니다.
    #[arg(long)]
    pub print_applet_dir: bool,

    /// 실행할 WASM 애플릿의 이름. 지정하지 않으면 선택 목록을 보여줍니다.
    pub app_name: Option<String>,

    /// 장치에서 실행
    #[arg(long)]
    pub device: bool,

    #[arg(long, default_value_t = 4)]
    pub bits_per_pixel: u8,

    /// 초당 프레임 수(FPS)를 로그로 출력합니다.
    #[arg(long)]
    pub benchmark: bool,
}

/// 사용 가능한 언어 종류를 정의하는 열거형
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Language {
    #[value(name = "ko")]
    Ko,
    #[value(name = "en")]
    En,
    #[value(name = "ja")]
    Ja,
}
impl From<Language> for i32 {
    fn from(lang: Language) -> Self {
        match lang {
            Language::Ko => 0,
            Language::En => 1,
            Language::Ja => 2,
        }
    }
}

/// 실행할 WASM 애플릿의 이름을 결정합니다.
///
/// 인자로 앱 이름이 주어지면 그 값을 사용합니다.
/// 그렇지 않으면 기본 런처 애플릿 이름을 반환합니다.
pub fn determine_app_name(arg_app_name: Option<String>) -> Result<String> {
    match arg_app_name {
        Some(name) => {
            // 인자로 이름이 주어진 경우, 파일 확장자를 제외한 이름 부분만 추출합니다.
            if let Some(stem) = std::path::Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                Ok(stem.to_string()) // 하이픈 변환 제거: 원본 파일명 그대로 사용
            } else {
                anyhow::bail!("Invalid app name provided: '{}'", name)
            }
        }
        None => {
            // 1. 설정 파일에서 경로와 '처음 실행할 앱 이름'을 가져옵니다.
            let settings = RuntimeConfig::load().unwrap();
            let wasm_dir = settings.get_applet_dir();
            let default_app_name = settings.launcher_app; // 예: 설정 파일에 적힌 "launcher" 또는 "home"

            if let Ok(entries) = std::fs::read_dir(wasm_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("wasm")
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        // 2. 하드코딩된 "launcher" 대신 설정 파일에서 가져온 이름을 사용해 검색합니다.
                        if stem.contains(&default_app_name) {
                            return Ok(stem.to_string());
                        }
                    }
                }
            }
            // 3. 찾지 못했을 때도 설정 파일의 기본 앱 이름을 반환합니다.
            Ok(default_app_name)
        }
    }
}
