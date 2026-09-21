// applets/tactile-game/src/lib.rs

use sdk::Language;
use sdk::api::context::Context;
use sdk::api::display::DisplayInterface;
use sdk::api::keypad::{KeyState, KeypadPopResult};
use sdk::api::log as sdk_log;
use sdk::applet::Applet;
use sdk::error::Result;
use sdk::event::UpdateResult;
mod config;
mod draw;
mod game;
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

        // 환영 국문 및 다국어 TTS 음성 출력 (가이드 안내 제거 및 간소화)
        let welcome_tts = match context.language {
            Language::Ko => {
                "촉각 게임에 오신 것을 환영합니다. 센터 키를 누르면 즉시 게임이 시작됩니다."
                    .to_string()
            }
            Language::En => "Welcome to Tactile Game! Press Center key to start.".to_string(),
            Language::Ja => {
                "触覚ゲームへようこそ！センターキーを押すとすぐにゲームが開始されます。".to_string()
            }
        };
        context.audio.speak_text(&welcome_tts);
        Ok(())
    }

    /// 매 프레임(60Hz 단위 등)의 입력 수집 및 물리 틱 갱신을 총괄 처리합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let width = context.window.width();
        let height = context.window.height();

        // 1. 피드백 연출 타이머 연계 점검 및 애니메이션 상태 전이 연산
        // (게임 내부의 바 컴포넌트 on_update 가 먼저 키를 소모하도록 함)
        self.update_ticks(width, height, context);

        // 2. 남은 키패드 이벤트 큐가 빌 때까지 팝핑 처리 루프 작동
        loop {
            match context.keypad.pop_event() {
                KeypadPopResult::NoEvent => {
                    break;
                }
                KeypadPopResult::Event(event) => {
                    // 키가 눌려지는 누름(Pressed) 상태일 때만 반응하여 오조작 완벽 필터링
                    if event.state == KeyState::Pressed {
                        self.handle_key_event(event.code, context);
                    }
                }
            }
        }

        // 실시간 테두리 회전 및 정답 깜빡임 등의 풍부한 인터랙션을 위해 상시 재그리기(NeedsRedraw) 인가
        Ok(UpdateResult::NeedsRedraw)
    }

    /// 현재 게임 상태 머신(GameState)에 따라, draw.rs의 수려한 그래픽/촉각 렌더러를 매끄럽게 연결합니다.
    fn on_draw(&self, display: &mut dyn DisplayInterface) -> Result<()> {
        match self.state {
            GameState::Ready => {
                draw::draw_ready_screen(display, self);
            }
            GameState::Playing => {
                // 채점 연출 피드백 타이머가 작동 중이며 정오답 판정이 내려진 경우, 대형 O/X 피드백 화면을 전체 렌더링합니다.
                if let Some(correct) = self.feedback_correct {
                    draw::draw_feedback_screen(display, self, correct);
                } else {
                    draw::draw_play_screen(display, self);
                }
            }
            GameState::GameOver => {
                draw::draw_over_screen(display, self);
            }
            GameState::Stop => {
                draw::draw_stop_screen(display, self);
            }
            GameState::Reset => {
                draw::draw_reset_screen(display, self);
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
