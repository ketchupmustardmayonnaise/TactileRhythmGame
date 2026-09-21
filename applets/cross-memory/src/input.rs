use sdk::api::audio::Audio;
use sdk::api::audio::AudioSegment;
use sdk::api::keypad::KeyCode;
use sdk::tts_segment;
use sdk::types::Language;

use crate::game::CrossMemory;
use crate::types::{ALL_DIRECTIONS, GameState, InputResult};

impl CrossMemory {
    // 키패드 이벤트를 처리하는 함수입니다. 언어 설정을 추가로 인자로 전달받아 다국어 텍스트 음성 출력 시 활용합니다.
    pub fn handle_keypad_event(&mut self, code: KeyCode, audio: &mut Audio, lang: Language) {
        match self.state {
            GameState::Ready => match code {
                KeyCode::Up => {
                    self.prev_mode();
                    audio.speak_text(self.mode.name());
                }
                KeyCode::Down => {
                    self.next_mode();
                    audio.speak_text(self.mode.name());
                }
                KeyCode::Center => match self.mode {
                    // 튜토리얼을 시작할 때 언어 설정을 추가 인자로 전달합니다.
                    crate::types::GameMode::Tutorial => self.start_tutorial(audio, lang),
                    _ => {
                        self.begin_locating();
                        // 정의되지 않은 context.language 대신 인자로 전달받은 lang 변수를 사용합니다.
                        audio.speak_text(match lang {
                            Language::Ko => "화면 중앙에 점멸되는 십자가를 찾아 손가락을 올린 뒤, 가운데 키를 눌러 주세요",
                            Language::En => "Find the blinking cross in the center of the screen, place your finger on it, and press the center key.",
                            Language::Ja => "画面中央で点滅している十字を探して指をのせ、中央のキーを押してください。",
                        });
                    }
                },
                _ => {}
            },
            GameState::Locating if code == KeyCode::Center => {
                self.start_game();
                // 정의되지 않은 context.language 대신 인자로 전달받은 lang 변수를 사용합니다.
                audio.speak_text(match lang {
                    Language::Ko => "게임 시작",
                    Language::En => "Game Start",
                    Language::Ja => "ゲーム開始",
                });
            }
            GameState::Tutorial(step) => match step {
                crate::types::TutorialStep::IdentifyCross if code == KeyCode::Center => {
                    self.state =
                        GameState::Tutorial(crate::types::TutorialStep::SinglePracticeInit(0));
                    self.wait_start = self.time.get_monotonic_time();
                    self.need_draw = true;
                }
                crate::types::TutorialStep::SinglePracticeWait(idx) => {
                    if let Some(input_dir) = crate::types::keycode_to_direction(code) {
                        audio.play(input_dir.sound_bytes());
                        self.active_square = Some(input_dir);
                        self.flash_start = self.time.get_monotonic_time();
                        self.need_draw = true;

                        if input_dir == ALL_DIRECTIONS[idx] {
                            if idx + 1 < ALL_DIRECTIONS.len() {
                                self.state = GameState::Tutorial(
                                    crate::types::TutorialStep::SinglePracticeInit(idx + 1),
                                );
                            } else {
                                self.state = GameState::Tutorial(
                                    crate::types::TutorialStep::SequencePracticeInit,
                                );
                            }
                            self.wait_start = self.time.get_monotonic_time();
                        } else {
                            // 피드백 전송 시 문자열 대신 tts_segment! 매크로를 사용하여 Vec<AudioSegment>로 빌드해 전달합니다.
                            self.schedule_feedback(
                                tts_segment!("틀렸습니다. 다시 눌러보세요.")
                                    .into_iter()
                                    .map(AudioSegment::Text)
                                    .collect(),
                            );
                        }
                    }
                }
                crate::types::TutorialStep::SequencePracticeInput(idx) => {
                    if let Some(input_dir) = crate::types::keycode_to_direction(code) {
                        audio.play(input_dir.sound_bytes());
                        self.active_square = Some(input_dir);
                        self.flash_start = self.time.get_monotonic_time();
                        self.need_draw = true;

                        if input_dir == self.sequence[idx] {
                            if idx + 1 < self.sequence.len() {
                                self.state = GameState::Tutorial(
                                    crate::types::TutorialStep::SequencePracticeInput(idx + 1),
                                );
                            } else {
                                self.state =
                                    GameState::Tutorial(crate::types::TutorialStep::Complete);
                                self.schedule_feedback(
                                    tts_segment!("잘하셨습니다! 이제 모든 준비가 끝났습니다. 가운데 키를 누르면 메인 화면으로 돌아갑니다.")
                                        .into_iter()
                                        .map(AudioSegment::Text)
                                        .collect(),
                                );
                            }
                        } else {
                            self.schedule_feedback(
                                tts_segment!("틀렸습니다. 다시 보여드릴게요.")
                                    .into_iter()
                                    .map(AudioSegment::Text)
                                    .collect(),
                            );
                            self.state = GameState::Tutorial(
                                crate::types::TutorialStep::SequencePracticeInit,
                            );
                        }
                        self.wait_start = self.time.get_monotonic_time();
                        self.need_draw = true;
                    }
                }
                crate::types::TutorialStep::Complete if code == KeyCode::Center => {
                    self.state = GameState::Ready;
                    self.need_draw = true;
                    audio.speak_text(self.mode.name());
                }
                _ => {}
            },
            GameState::Input => match self.handle_input_key(code, audio) {
                InputResult::Wrong => {
                    let score_str = self.score.to_string();
                    self.schedule_feedback(
                        tts_segment!("틀렸습니다. ", score_str, "점")
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect(),
                    );
                }
                InputResult::RoundComplete => {
                    let score_str = self.score.to_string();
                    self.schedule_feedback(
                        tts_segment!(score_str, "단계 성공")
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect(),
                    );
                }
                InputResult::Correct | InputResult::Ignored => {}
            },
            GameState::End if code == KeyCode::Center => {
                self.state = GameState::Ready;
                self.need_draw = true;
                audio.speak_text(self.mode.name());
            }
            _ => {}
        }
    }
}
