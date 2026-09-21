mod draw;
mod game;
mod input;
mod rng;
mod types;

use sdk::Applet;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Point};
use sdk::api::keypad::{KeyState, KeypadEvent, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::tts_segment;
use sdk::types::Language;

use draw::{PROGRESS_BAR_GAP, PROGRESS_BAR_HEIGHT};
use draw::{draw_cross_pattern, draw_end_screen, draw_mode_menu, draw_progress_bar};
use game::CrossMemory;
use types::{GameState, TutorialStep};

impl Applet for CrossMemory {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        self.need_draw = true;
        context
            .audio
            .speak_text(match context.language {
            Language::Ko => "크로스 메모리. 상하 방향키로 모드를 선택하고 가운데 키를 누르세요.",
            Language::En => "Cross Memory. Select a mode using the up and down arrow keys, then press the center key.",
            Language::Ja => "クロスメモリー。上下の方向キーでモードを選択し、中央のキーを押してください。",
        });
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        loop {
            match context.keypad.pop_event() {
                KeypadPopResult::NoEvent => break,
                KeypadPopResult::Event(KeypadEvent { code, state, .. }) => {
                    if state == KeyState::Pressed {
                        // 수정: 키패드 이벤트 처리 시 현재 설정된 언어 정보를 함께 전달합니다.
                        self.handle_keypad_event(code, &mut context.audio, context.language);
                    }
                }
            }
        }

        if let Some(segments) = self.take_due_feedback() {
            context
                .audio
                .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
        }

        match self.state {
            GameState::Waiting => {
                self.handle_waiting_tick();
                if self.need_draw {
                    self.need_draw = false;
                    Ok(UpdateResult::NeedsRedraw)
                } else {
                    Ok(UpdateResult::Unchanged)
                }
            }
            GameState::Locating => {
                self.handle_locating_tick();
                if self.need_draw {
                    self.need_draw = false;
                    Ok(UpdateResult::NeedsRedraw)
                } else {
                    Ok(UpdateResult::Unchanged)
                }
            }
            GameState::Tutorial(_) => {
                // 수정: 튜토리얼 틱 업데이트 시 현재 설정된 언어 정보를 함께 전달합니다.
                self.handle_tutorial_tick(&mut context.audio, context.language);
                if self.need_draw {
                    self.need_draw = false;
                    Ok(UpdateResult::NeedsRedraw)
                } else {
                    Ok(UpdateResult::Unchanged)
                }
            }
            GameState::Showing => {
                self.handle_showing_tick(&mut context.audio);
                Ok(UpdateResult::NeedsRedraw)
            }
            GameState::Input => {
                self.handle_input_tick();
                if self.state == GameState::End {
                    let score_str = self.score.to_string();
                    let segments = tts_segment!("시간 초과. ", score_str, "점")
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                    context
                        .audio
                        .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                }
                Ok(UpdateResult::NeedsRedraw)
            }
            _ => {
                if self.need_draw {
                    self.need_draw = false;
                    Ok(UpdateResult::NeedsRedraw)
                } else {
                    Ok(UpdateResult::Unchanged)
                }
            }
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();
        let size = canvas.get_size();
        let game_area_height = size.height - PROGRESS_BAR_HEIGHT - PROGRESS_BAR_GAP;
        let game_center = Point::new(size.width / 2, game_area_height / 2);

        match self.state {
            GameState::Ready => draw_mode_menu(canvas, self.mode),
            GameState::Locating => {
                if self.is_locating_visible {
                    draw_cross_pattern(canvas, game_center, None);
                }
            }
            GameState::Tutorial(step) => match step {
                TutorialStep::Introduction => {}
                TutorialStep::IdentifyCross => {
                    if self.is_locating_visible {
                        draw_cross_pattern(canvas, game_center, None);
                    }
                }
                TutorialStep::SinglePracticeInit(_) | TutorialStep::SinglePracticeWait(_) => {
                    draw_cross_pattern(canvas, game_center, self.active_square);
                }
                TutorialStep::SequencePracticeInit
                | TutorialStep::SequencePracticeShow(_)
                | TutorialStep::SequencePracticeInput(_) => {
                    draw_cross_pattern(canvas, game_center, self.active_square);
                }
                TutorialStep::Complete => {}
            },
            GameState::Waiting => draw_cross_pattern(canvas, game_center, None),
            GameState::Showing => draw_cross_pattern(canvas, game_center, self.active_square),
            GameState::Input => {
                draw_cross_pattern(canvas, game_center, self.active_square);
                draw_progress_bar(canvas, self.remaining_ratio());
            }
            GameState::End => draw_end_screen(canvas, &self.end_message, self.score),
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(CrossMemory::default()));
}
