use std::time::Duration;

use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Point};
use sdk::applet::Applet;
use sdk::error::Result;
use sdk::event::UpdateResult;

mod config;
mod drawing;
mod game;
mod input;
mod menu;
mod speak;
mod types;

use crate::config::GameConfig;
use crate::game::DrawingGame;
use crate::types::{FunctionKeyFlags, GameStatus};
use sdk::Language;
use views::viewport::Viewport;

/// `Applet` 트레이트를 구현하여 게임을 애플릿으로 실행할 수 있도록 합니다.
impl Applet for DrawingGame {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        // 사용자 기기 설정에서 저장된 언어 값을 읽어옵니다. (없으면 0: Ko)
        let _ = context.settings.get_options();
        let lang_str = context
            .settings
            .get_value(sdk::api::settings::LANGUAGE)
            .unwrap_or_else(|_| "default".to_string());
        self.language = sdk::Language::from(lang_str.parse::<i32>().unwrap_or(0));

        // 캔버스 중앙(500, 500)에서 시작
        let initial_x = (self.config.view.canvas_width as i32) / 2;
        let initial_y = (self.config.view.canvas_height as i32) / 2;
        self.physics_state.pos = (initial_x, initial_y);
        self.viewport = Viewport::new(
            Point::new(initial_x as i16, initial_y as i16),
            context.window.width(),
            context.window.height(),
        );
        self.last_px = initial_x as i16;
        self.last_py = initial_y as i16;
        self.physics_state.vel = (0.0, 0.0);
        self.need_redraw = true;
        self.last_time = context.time.get_monotonic_time();

        let start_mag = match self.language {
                Language::Ko => "
                    방향키로 이동하고, 중앙키를 눌러 현재 선택된 도구로 그림을 그리거나 지웁니다. 
                    기능 키를 누르면 그리기 모드와 지우기 모드가 전환됩니다. 
                    메뉴 키를 누르면 브러시 크기를 조절할 수 있는 메뉴가 열립니다. 
                    중앙 키를 누른 채 방향키를 누르면 이동하면서 그리거나 지우는게 가능합니다. 
                    자유롭게 그림을 그려보세요!".to_string(),
                Language::En => "
Use the arrow keys to move, and press the center key to draw or erase with the currently selected tool. 
Press the function key to switch between drawing and erasing modes. 
Press the menu key to open a menu where you can adjust the brush size.
Hold down the center key and press the arrow keys to draw or erase while moving.
Draw freely!".to_string(),
                Language::Ja => "
 方向キーに移動し、中央キーを押して現在選択されているツールで画像を描画または消去します。 
ファンクションキーを押すと、描画モードと消去モードが切り替わります。
メニューキーを押すと、ブラシのサイズを変更できるメニューが開きます。
中央キーを押しながら方向キーを押すと、移動しながら描画または消去することができます。
自由に絵を描いてみてください！
                    ".to_string(),
            };

        context.audio.speak_text(&start_mag);
        Ok(())
    }
    /// 키패드 입력을 확인하고 커서 위치를 업데이트합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        if self.state == GameStatus::Menu {
            match self.menu.on_update(context, self.language) {
                menu::MenuAction::None => {}
                menu::MenuAction::Redraw => self.need_redraw = true,
                menu::MenuAction::SelectMode(mode) => {
                    self.set_mode(context, mode);
                    self.set_state(context, GameStatus::Playing);
                    self.need_redraw = true;
                }
                menu::MenuAction::Close => {
                    self.set_state(context, GameStatus::Playing);
                    self.need_redraw = true;
                }
            }
        } else {
            loop {
                match context.keypad.pop_event() {
                    sdk::api::keypad::KeypadPopResult::NoEvent => {
                        break;
                    }
                    sdk::api::keypad::KeypadPopResult::Event(event) => {
                        input::handle_key_event(self, context, event);
                    }
                }
            }
        }
        let now = context.time.get_monotonic_time();
        let frame_time = now.saturating_sub(self.last_time);
        self.last_time = now;

        // 메뉴가 켜져있지 않을 때만 누적 시간을 증가시켜 물리 엔진을 가동합니다. (일시정지 효과)
        if self.state == GameStatus::Playing {
            self.accumulator += frame_time;
        }

        // 브러시 크기가 점 하나(반지름 0)일 때만 커서 깜박임을 처리합니다.
        if self.brush_size == 0 {
            self.update_cursor_blink(frame_time);
        } else {
            self.cursor_visible = true; // 브러시가 클 때는 항상 보이도록 유지
        }

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
                if self.fn_key_state.contains(FunctionKeyFlags::CENTER) {
                    drawing::draw_connected_line(self, self.last_px, self.last_py, px, py);
                }
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
        drawing::draw(self, canvas)?;

        if self.state == GameStatus::Menu {
            self.menu.draw(canvas, self);
        }
        Ok(())
    }
}

/// 게임 애플릿의 진입점입니다.
/// 이 함수는 `multiline-braille-display` 런타임에 의해 호출됩니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::api::log::init().expect("Logger 초기화에 실패했습니다.");
    sdk::run(Box::new(DrawingGame::new(GameConfig::default()))); // 게임 애플릿 실행
}
