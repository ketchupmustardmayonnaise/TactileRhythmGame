use crate::{
    Language,
    api::{audio::AudioSegment, context::Context, display::DisplayInterface},
    error::Result,
    event::UpdateResult,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

pub static DEFAULT_TARGET_FPS: u16 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum AppletPriority {
    Critical,
    High,
    Normal,
    #[default]
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum AppletCategory {
    System,
    Utility,
    Game,
    #[default]
    Test,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppletInfo {
    pub id: String,
    pub name: LocalizedString,
    pub version: String,
    pub description: LocalizedString,
    pub icon: String,
    pub hidden: bool,
    pub priority: AppletPriority,
    pub category: LocalizedString,
}



/// 다국어 문자열을 파싱하기 위한 구조체입니다.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalizedString {
    #[serde(default)]
    pub ko: String,
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub ja: String,
}

impl LocalizedString {
    /// 현재 설정된 언어에 맞춰 다국어 문자열을 반환합니다.
    /// 해당 언어의 문자열이 비어있을 경우 영어(En) -> 한국어(Ko) 순으로 폴백(fallback)합니다.
    pub fn get_string_by_lang(&self, lang: &Language) -> &str {
        let text = match lang {
            Language::Ko => &self.ko,
            Language::En => &self.en,
            Language::Ja => &self.ja,
        };

        if !text.is_empty() {
            text
        } else if !self.en.is_empty() {
            &self.en
        } else if !self.ko.is_empty() {
            &self.ko
        } else if !self.ja.is_empty() {
            &self.ja
        } else {
            "Unknown" // 모든 문자열이 비어있을 경우의 기본값
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpeechOption {
    pub forced: bool,
    pub important: bool,
}

impl SpeechOption {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn forced() -> Self {
        Self {
            forced: true,
            important: false,
        }
    }

    pub fn important() -> Self {
        Self {
            forced: false,
            important: true,
        }
    }

    pub fn forced_important() -> Self {
        Self {
            forced: true,
            important: true,
        }
    }
}

/// TTS(Text-to-Speech) 출력 결과를 정의하는 열거형입니다.
#[derive(Debug, Clone, PartialEq)]
pub enum SpeechResult {
    None,

    Text {
        text: String,
        options: SpeechOption,
    },

    AudioSegments {
        segments: Vec<AudioSegment>,
        options: SpeechOption,
    },
}

impl SpeechResult {
    /// 기본 옵션으로 텍스트 음성 출력 결과를 생성합니다.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            options: SpeechOption::new(),
        }
    }

    /// 지정된 옵션으로 텍스트 음성 출력 결과를 생성합니다.
    pub fn text_with_options(text: impl Into<String>, options: SpeechOption) -> Self {
        Self::Text {
            text: text.into(),
            options,
        }
    }

    /// 기본 옵션으로 오디오 세그먼트 시퀀스 출력 결과를 생성합니다.
    pub fn segments(segments: Vec<AudioSegment>) -> Self {
        Self::AudioSegments {
            segments,
            options: SpeechOption::new(),
        }
    }

    /// 지정된 옵션으로 오디오 세그먼트 시퀀스 출력 결과를 생성합니다.
    pub fn segments_with_options(segments: Vec<AudioSegment>, options: SpeechOption) -> Self {
        Self::AudioSegments { segments, options }
    }
}

/// WASM 애플릿이 구현해야 하는 인터페이스를 정의하는 트레이트입니다.
/// 모든 애플릿은 이 트레이트를 구현하여 런타임에 의해 관리될 수 있습니다.
pub trait Applet: Send {
    fn on_start(&mut self, _context: &mut Context) -> Result<()> {
        Ok(())
    }

    fn on_stop(&mut self, _context: &mut Context) -> Result<()> {
        Ok(())
    }

    /// 애플릿이 목표로 하는 프레임 속도를 반환합니다.
    fn target_fps(&self) -> u16 {
        DEFAULT_TARGET_FPS
    }

    /// 애플리케이션 상태를 업데이트합니다.
    /// 애플리케이션이 다음 프레임을 그리기를 원하면 `Ok(UpdateResult::DrawNextFrame)`을 반환하고,
    /// 단순히 계속 실행하려면 `Ok(UpdateResult::ContinueRunning)`을 반환합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult>;

    /// 현재 애플리케이션 상태를 디스플레이에 그립니다.
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()>;

    /// 현재 애플릿의 상태를 기반으로 출력해야 할 음성(TTS) 텍스트를 반환합니다.
    /// 상태 변화에 따른 선언적 오디오 출력을 위해 런타임 메인 루프에서 주기적으로 호출됩니다.
    ///
    /// 반환된 문자열이 이전 프레임과 다를 경우 런타임이 자동으로 음성을 출력합니다.
    fn on_speech(&self, _context: &Context) -> SpeechResult {
        SpeechResult::None
    }

    /// 시각장애인 사용자를 위한 현재 애플릿 조작 도움말(가이드)을 반환합니다.
    /// 기능 키(Function)와 메뉴 키(Menu)를 동시에 누르면 런타임이 이 메소드를 호출하여 TTS로 독출합니다.
    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString::default()
    }
}


/// 현재 실행 중인 애플릿 인스턴스를 저장하는 정적 변수입니다.
/// `once_cell::sync::Lazy`와 `std::sync::Mutex`를 사용하여 안전하게 한 번만 초기화되고 동시 접근이 가능합니다.
pub static APP: Lazy<Mutex<Option<Box<dyn Applet>>>> = Lazy::new(|| Mutex::new(None));
pub static CONTEXT: Lazy<Mutex<Option<Context>>> = Lazy::new(|| Mutex::new(None));

/// 이전에 출력된 TTS 상태를 추적하는 정적 변수입니다.
pub static LAST_SPEECH: Lazy<Mutex<SpeechResult>> = Lazy::new(|| Mutex::new(SpeechResult::None));

/// 애플릿의 상태(TTS) 변화를 감지하고 오디오 출력을 수행하는 내부 처리 함수입니다.
/// 락(Lock)을 미리 확보한 상태에서 호출하여 데드락을 방지합니다.
pub fn process_tts_tracking(app: &dyn Applet, context: &mut Context) {
    let current_speech = app.on_speech(context);
    let mut last_speech = LAST_SPEECH.lock();
    if current_speech != *last_speech {
        match &current_speech {
            SpeechResult::None => {}
            SpeechResult::Text { text, options } => {
                context.audio.speak_text_with_option(text, *options);
            }
            SpeechResult::AudioSegments { segments, options } => {
                context.audio.play_audio_sequence(segments, *options);
            }
        }
        *last_speech = current_speech;
    }
}

/// 매 프레임 업데이트 직후에 호출되어, `on_speech` 문자열의 변화를 감지하고
/// 변경 사항이 있으면 자동으로 오디오 출력을 호스트로 요청합니다.
pub fn handle_tts_tracking() {
    let app_guard = APP.lock();
    let mut context_guard = CONTEXT.lock();

    if let (Some(app), Some(context)) = (app_guard.as_deref(), context_guard.as_mut()) {
        process_tts_tracking(app, context);
    }
}

/// WASM 애플릿을 런타임에 등록하고 실행 준비를 마칩니다.
pub fn run(app: Box<dyn Applet>) {
    *APP.lock() = Some(app);
    *LAST_SPEECH.lock() = SpeechResult::None; // 앱 시작 시 이전 TTS 상태를 초기화합니다.

    let mut context = Context::new();
    let lang_str = context
        .settings
        .get_value(crate::api::settings::LANGUAGE)
        .unwrap_or_else(|_| "0".to_string());
    context.language = Language::from(lang_str.parse::<i32>().unwrap_or(0));

    // AppletManager를 통해 현재 실행 중인 앱의 정보를 가져와 Context의 info를 초기화합니다.
    if let Ok(info) = crate::api::applet_manager::AppletManager::new().get_current_applet_info() {
        context.info = info;
    }

    context.keypad.set_perkins_mode(false);

    *CONTEXT.lock() = Some(context);
}
