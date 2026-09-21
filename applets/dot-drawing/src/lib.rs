use std::time::Duration;

use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Point};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::{Applet, Language};

mod config;
#[allow(dead_code)]
mod drawing;
mod game;
mod input;
mod speak;
mod types;

use crate::config::GameConfig;
use crate::game::Game;
use views::viewport::Viewport;

/// `Applet` 트레이트를 구현하여 게임을 애플릿으로 실행할 수 있도록 합니다.
impl Applet for Game {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        // 사용자 기기 설정에서 저장된 언어 값을 읽어옵니다. (없으면 0: Ko)
        let _ = context.settings.get_options();
        let lang_str = context
            .settings
            .get_value(sdk::api::settings::LANGUAGE)
            .unwrap_or_else(|_| "default".to_string());
        self.language = sdk::Language::from(lang_str.parse::<i32>().unwrap_or(0));

        let width = context.window.width();
        let height = context.window.height();

        // 캔버스 크기가 실제 화면 크기와 다르면 재할당하여 고정시킵니다.
        if self.canvas.width != width as usize || self.canvas.height != height as usize {
            self.canvas = views::canvas::PixelBuffer::new(width as usize, height as usize);
            self.physics_state =
                physics::PhysicsState::new((width as i32) - 1, (height as i32) - 1);
        }

        // 화면 중앙에서 시작
        let initial_x = (width as i32) / 2;
        let initial_y = (height as i32) / 2;
        self.physics_state.pos = (initial_x, initial_y);
        self.viewport = Viewport::new(
            Point::new(initial_x as i16, initial_y as i16),
            width,
            height,
        );
        self.last_px = initial_x as i16;
        self.last_py = initial_y as i16;
        self.physics_state.vel = (0.0, 0.0);
        self.need_redraw = true;
        self.last_time = context.time.get_monotonic_time();

        let start_mag = match self.language {
            Language::Ko => "방향키로 위치를 이동하고,
                중앙 키를 누르면 점을 하나씩 그리거나 지울 수 있습니다.
                기능 키를 누르면 그리기 도구와 지우기 도구가 전환됩니다.
                지금부터 그림을 그려보세요!"
                .to_string(),
            Language::En => "Move the cursor using the arrow keys,
                and press the Center key to draw or erase dots one by one.
                Press the Function key to switch between the Draw and Erase tools.
                Start drawing now!"
                .to_string(),
            Language::Ja => "ゲームが始まりました。方向キーで位置を移動し、
                中央キーを押すと、ポイントを1つずつ描画または消去できます。
                ファンクションキーを押すと、描画ツールと消去ツールが切り替わります。
                これから絵を描いてみてください！"
                .to_string(),
        };
        context.audio.speak_text(&start_mag);

        Ok(())
    }

    /// 키패드 입력을 확인하고 커서 위치를 업데이트합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        loop {
            match context.keypad.pop_event() {
                sdk::api::keypad::KeypadPopResult::NoEvent => {
                    break;
                }
                sdk::api::keypad::KeypadPopResult::Event(event) => {
                    input::handle_key_event(self, event);
                }
            }
        }
        let now = context.time.get_monotonic_time();
        let frame_time = now.saturating_sub(self.last_time);
        self.last_time = now;
        self.accumulator += frame_time;

        // 커서 깜박임 처리 (300ms)
        self.update_cursor_blink(frame_time);

        // 나선형 죽음(Spiral of Death) 방지: 최대 누적 시간 제한 (0.2초)
        let max_acc = self.config.system.max_accumulator_ms;
        if self.accumulator.as_millis() > max_acc as u128 {
            self.accumulator = Duration::from_millis(max_acc);
        }

        let dt = Duration::from_micros(1_000_000 / self.config.system.target_fps);

        while self.accumulator >= dt {
            // 1. 입력 처리
            input::update_input_state(self);
            input::process_input_logic(self, context);

            // 2. 물리 연산 (Controller 위임)
            self.movement_controller.on_update(
                &mut self.physics_state,
                self.input_state,
                &self.config.physics,
                dt.as_secs_f32() * 1000.0,
            );

            // 3. 뷰포트 및 그리기 상태 업데이트
            let px = self.physics_state.pos.0 as i16;
            let py = self.physics_state.pos.1 as i16;

            // 커서의 현재 화면(스크린) 상 위치를 계산
            let screen_pos = self.viewport.world_to_screen(Point::new(px, py));

            // 화면 좌우 경계선 밖으로 나가려 할 때만 뷰포트 이동 (스크롤)
            if screen_pos.x < 0 {
                self.viewport.center.x += screen_pos.x;
            } else if screen_pos.x >= self.viewport.width {
                self.viewport.center.x += screen_pos.x - self.viewport.width + 1;
            }

            // 화면 상하 경계선 밖으로 나가려 할 때만 뷰포트 이동 (스크롤)
            if screen_pos.y < 0 {
                self.viewport.center.y += screen_pos.y;
            } else if screen_pos.y >= self.viewport.height {
                self.viewport.center.y += screen_pos.y - self.viewport.height + 1;
            }

            // 4. 선 그리기 (이전 위치와 현재 위치 연결)
            if px != self.last_px || py != self.last_py {
                self.last_px = px;
                self.last_py = py;
                self.need_redraw = true;
            }

            self.accumulator -= dt;
        }

        if self.need_redraw {
            self.need_redraw = false;
            Ok(UpdateResult::NeedsRedraw) // 다음 프레임을 그리도록 지시
        } else {
            Ok(UpdateResult::Unchanged) // 키 입력이 없으면 계속 실행
        }
    }

    /// 게임 화면을 그립니다.
    /// 현재 커서 위치에 십자 모양을 그립니다.
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        drawing::draw(self, canvas)
    }
}

/// 게임 애플릿의 진입점입니다.
/// 이 함수는 `multiline-braille-display` 런타임에 의해 호출됩니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::api::log::init().expect("Logger 초기화에 실패했습니다.");
    sdk::run(Box::new(Game::new(GameConfig::default()))); // 게임 애플릿 실행
}
