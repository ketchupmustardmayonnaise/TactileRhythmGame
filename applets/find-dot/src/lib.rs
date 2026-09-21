mod draw;
mod game;
mod rng;

use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadEvent, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;
use widget::{Widget, WidgetUpdateResult};

use draw::{draw_blinking_dot, draw_cursor, draw_progress_bar};
use game::{FindDot, GameState};

impl Applet for FindDot {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        self.need_draw = true;

        let size = context.window.get_size();
        self.label
            .set_bounds(BoundsRect::new(Point::new(0, 0), size));

        context.audio.speak_text(match context.language {
            Language::Ko => "게임을 실행하시려면 가운데 키를 눌러주세요.",
            Language::En => "Press the center key to start the game.",
            Language::Ja => "ゲームを実行するには、中央のキーを押してください。",
        });
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let size = context.window.get_size();
        let game_area_height = self.game_area_height(size.height);
        let mut updated = false;

        loop {
            match context.keypad.pop_event() {
                KeypadPopResult::NoEvent => break,
                KeypadPopResult::Event(KeypadEvent { code, state, .. }) => {
                    if state == KeyState::Pressed {
                        self.on_key_pressed(code);
                        match self.state {
                            GameState::Ready | GameState::End if code == KeyCode::Center => {
                                context.audio.speak_text(match context.language {
                                    Language::Ko => "게임 시작",
                                    Language::En => "Game Start",
                                    Language::Ja => "ゲーム開始",
                                });
                                self.start_game(size.width, game_area_height);
                                updated = true;
                            }
                            GameState::Playing => {
                                updated |= self.handle_playing_input(
                                    context,
                                    size.width,
                                    game_area_height,
                                );
                            }
                            _ => {}
                        }
                    } else if state == KeyState::Released {
                        self.on_key_released(code);
                    }
                }
            }
        }

        if self.state == GameState::Playing {
            if self.poll_repeat() {
                self.handle_playing_input(context, size.width, game_area_height);
            }
            Ok(self.handle_playing_tick(context))
        } else {
            let widget_update = self.label.on_update(context, false)?;
            if updated || self.need_draw || widget_update == WidgetUpdateResult::NeedsRedraw {
                self.need_draw = false;
                Ok(UpdateResult::NeedsRedraw)
            } else {
                Ok(UpdateResult::Unchanged)
            }
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        match self.state {
            GameState::Ready | GameState::End => {
                let size = canvas.get_size();
                let game_area_height = self.game_area_height(size.height);
                draw_cursor(canvas, Point::new(size.width / 2, game_area_height / 2));
            }
            GameState::Playing => {
                draw_cursor(canvas, self.cursor_position);
                draw_blinking_dot(canvas, self.dot_position, self.dot_visible);
                draw_progress_bar(canvas, self.remaining_ratio());
            }
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(FindDot::default()));
}
