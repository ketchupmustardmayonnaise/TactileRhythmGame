use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::DisplayInterface;
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;

/// 게임 메시지를 정의하는 열거형
enum GameMessage {
    Start,
    Stop,
}

impl GameMessage {
    fn text(&self, lang: Language) -> &'static str {
        match self {
            GameMessage::Start => match lang {
                Language::Ko => "미러링 시작",
                Language::En => "Mirroring Started",
                Language::Ja => "ミラーリング開始",
            },
            GameMessage::Stop => match lang {
                Language::Ko => "미러링 종료",
                Language::En => "Mirroring Finished",
                Language::Ja => "ミラーリング終了",
            },
        }
    }
}
/// 게임의 상태를 관리하는 구조체.
/// 디스플레이, 키패드 및 커서의 현재 위치를 포함합니다.
#[derive(Default)]
struct Mirror {}

/// `Applet` 트레이트를 구현하여 게임을 애플릿으로 실행할 수 있도록 합니다.
impl Applet for Mirror {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        // 호스트로부터 설정 스키마 및 사용자가 선택한 값을 불러와 내부 캐시(options)를 갱신합니다.
        let _ = context.settings.get_options();

        let lang_str = context
            .settings
            .get_value(sdk::api::settings::LANGUAGE)
            .unwrap_or_else(|_| "default".to_string());
        let lang = Language::from(lang_str.parse::<i32>().unwrap_or(0));
        context.audio.speak_text(GameMessage::Start.text(lang));
        Ok(())
    }

    fn on_stop(&mut self, context: &mut Context) -> Result<()> {
        let lang_str = context
            .settings
            .get_value(sdk::api::settings::LANGUAGE)
            .unwrap_or_else(|_| "default".to_string());
        let lang = Language::from(lang_str.parse::<i32>().unwrap_or(0));
        context.audio.speak_text(GameMessage::Stop.text(lang));
        Ok(())
    }

    fn on_update(&mut self, _context: &mut Context) -> Result<UpdateResult> {
        Ok(UpdateResult::Unchanged)
    }

    fn on_draw(&self, _canvas: &mut dyn DisplayInterface) -> Result<()> {
        Ok(())
    }
}

/// 게임 애플릿의 진입점입니다.
/// 이 함수는 `multiline-braille-display` 런타임에 의해 호출됩니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(Mirror::default())); // 게임 애플릿 실행
}
