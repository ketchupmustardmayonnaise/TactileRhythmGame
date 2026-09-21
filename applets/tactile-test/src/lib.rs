// applets/tactile-game/src/lib.rs

use sdk::Language;
use sdk::api::audio::AudioSegment; // 다중 오디오 세그먼트 처리를 위해 임포트
use sdk::api::context::Context;
use sdk::api::display::DisplayInterface;
use sdk::api::keypad::{KeyState, KeypadPopResult};
use sdk::api::log as sdk_log;
use sdk::applet::Applet;
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::tts_segment; // 다중 텍스트 처리를 위한 tts_segment 매크로 임포트
mod config;
mod draw;
mod game;
mod guide; // [추가] 가이드 전용 동작 및 화면 제어를 수반하는 서브모듈
mod pattern;
mod rng;

use crate::config::GameState;
use crate::game::TactileGame;

impl Applet for TactileGame {
    /// 애플릿이 최초로 부팅되어 진입하는 시작 지점 핸들러입니다.
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk_log::init();
        let width = context.window.width();
        let height = context.window.height();

        // 윈도우 창 크기에 입각해 격자 및 객관식 가로 선택 바 컴포넌트 레이아웃 정밀 빌드
        self.init_layouts(width, height);
        // 타이머 및 애니메이션 프레임 초기화
        self.tick = 0;

        // 촉각게임 테스트 전에 패턴소개를 제공하고, 키 안내를 알려주는 대기 음성 시퀀스를 구성합니다.
        let welcome_tts = match context.language {
            Language::Ko => tts_segment!(
                "촉지시험 테스트전에 패턴소개 페이지입니다.",
                " 가이드 화면으로 넘어가려면 중앙키를,",
                " 테스트화면으로 넘어가려면 메뉴키를 눌러주세요,"
            ),
            Language::En => tts_segment!(
                "This is the pattern introduction page before the tactile test.",
                " Press the center key to go to the guide screen, ",
                "or the menu key to go to the test screen."
            ),
            Language::Ja => tts_segment!(
                "触覚試験テスト前のパターン紹介ページです。",
                "ガイド画面に移動するには中央キーを、",
                "テスト画面に移動するにはメニューキーを押してください。"
            ),
        }
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();

        // 오디오 장치를 통해 대기 화면 안내 음성을 출력합니다.
        context
            .audio
            .play_audio_sequence(&welcome_tts, sdk::applet::SpeechOption::new());
        Ok(())
    }

    /// 매 프레임(60Hz 단위 등)의 입력 수집 및 물리 틱 갱신을 총괄 처리합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let width = context.window.width();
        let height = context.window.height();

        let mut key_event_occurred = false; // [추가] 키 입력 이벤트가 발생했는지 기록할 플래그

        // 1. 피드백 연출 타이머 연계 점검 및 애니메이션 상태 전이 연산
        let animation_needs_redraw = self.update_ticks(width, height, context);

        // 2. 키패드 이벤트 큐가 빌 때까지 팝핑 처리 루프 작동
        loop {
            match context.keypad.pop_event() {
                KeypadPopResult::NoEvent => {
                    break;
                }
                KeypadPopResult::Event(event) => {
                    // 키가 눌려지는 누름(Pressed) 상태일 때만 반응하여 오조작 완벽 필터링
                    if event.state == KeyState::Pressed {
                        self.handle_key_event(event.code, context);
                        key_event_occurred = true; // [변경] 키 이벤트가 발생했음을 표시합니다.
                    }
                }
            }
        }

        // 가이드 화면 이동을 위해 애플리케이션 종료 요청이 감지되었을 경우 호스트로 ExitApp을 즉각 전달합니다.
        if self.should_exit {
            return Ok(UpdateResult::ExitApp);
        }

        // [변경] 키 입력 이벤트가 일어났거나, 내부 애니메이션(깜빡임 등)에 화면 재그리기가 필요할 때만 그립니다.
        if key_event_occurred || animation_needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, display: &mut dyn DisplayInterface) -> Result<()> {
        match self.state {
            GameState::Ready => {
                draw::draw_ready_screen(display, self);
            }
            GameState::Guide => {
                // [추가] 가이드 진행 단계(1~8단계)에 맞는 촉각 가이드 화면을 드로잉합니다.
                guide::draw_guide_screen(display, self);
            }
            GameState::Playing => {
                // 게임 도전 진행 중 플레이 화면만 상시 렌더링합니다.
                draw::draw_play_screen(display, self);
            }
        }
        Ok(())
    }

    // 음성 피드백 보조가 필요할 때의 결과값을 처리합니다.
    // fn on_speech(&self, _context: &Context) -> sdk::applet::SpeechResult {
    //     // SpeechResult에는 default가 없으므로 비어있는 텍스트를 담은 결과를 수동으로 생성하여 반환합니다.
    //     sdk::applet::SpeechResult::text("")
    // }

    /// 애플릿이 백그라운드로 소거되거나 구동을 멈추었을 때 실행할 정리 핸들러입니다.
    fn on_stop(&mut self, _context: &mut Context) -> Result<()> {
        Ok(())
    }
}

/// 런타임 시스템 환경에 애플릿을 안전하게 레지스터하기 위한 고유 진입 함수를 동적 선언합니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let app = Box::new(TactileGame::default());
    sdk::applet::run(app);
}
