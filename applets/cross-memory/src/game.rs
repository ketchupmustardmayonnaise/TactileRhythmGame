use sdk::api::audio::Audio;
use sdk::api::audio::AudioSegment;
use sdk::api::keypad::KeyCode;
use sdk::api::time::Time;
use sdk::tts_segment;
use sdk::types::Language;
use std::time::Duration;

use crate::rng::Rng;
use crate::types::{
    ALL_DIRECTIONS, Direction, GameMode, GameState, InputResult, PendingTransition, TutorialStep,
    keycode_to_direction,
};

pub const BASE_SHOW_ON_MS: u32 = 500;
pub const SHOW_OFF_MS: u32 = 250;
pub const SHOW_SPEED_REDUCE_MS: u32 = 50;
pub const MIN_SHOW_ON_MS: u32 = 100;
pub const SPEED_MODE_SEQ_LEN: usize = 5;

pub const INPUT_TIME_LIMIT_MS: u32 = 10_000;
pub const INPUT_FLASH_MS: u32 = 200;
pub const WAIT_DURATION_MS: u32 = 1_500;
pub const FEEDBACK_TTS_DELAY_MS: u32 = 600;
pub const TUTORIAL_STEP_DELAY_MS: u32 = 1_500;

pub struct CrossMemory {
    pub state: GameState,
    pub need_draw: bool,
    pub mode: GameMode,
    pub sequence: Vec<Direction>,
    pub show_index: usize,
    pub input_index: usize,
    pub active_square: Option<Direction>,
    pub is_locating_visible: bool,
    pub score: u32,
    pub show_on_ms: u32,
    pub end_message: String,
    pub pending: PendingTransition,
    pub pending_feedback: Option<(Vec<AudioSegment>, Duration)>,
    pub time: Time,
    pub show_step_start: Duration,
    pub input_start: Duration,
    pub flash_start: Duration,
    pub wait_start: Duration,
    pub rng: Rng,
}

impl Default for CrossMemory {
    fn default() -> Self {
        Self {
            state: GameState::default(),
            need_draw: false,
            mode: GameMode::default(),
            sequence: Vec::new(),
            show_index: 0,
            input_index: 0,
            active_square: None,
            is_locating_visible: true,
            score: 0,
            show_on_ms: BASE_SHOW_ON_MS,
            end_message: String::new(),
            pending: PendingTransition::default(),
            pending_feedback: None,
            time: Time::new(),
            show_step_start: Duration::ZERO,
            input_start: Duration::ZERO,
            flash_start: Duration::ZERO,
            wait_start: Duration::ZERO,
            rng: Rng::new(),
        }
    }
}

impl CrossMemory {
    pub fn next_mode(&mut self) {
        self.mode = self.mode.next();
        self.need_draw = true;
    }

    pub fn prev_mode(&mut self) {
        self.mode = self.mode.prev();
        self.need_draw = true;
    }

    pub fn begin_locating(&mut self) {
        self.state = GameState::Locating;
        self.active_square = None;
        self.is_locating_visible = true;
        self.wait_start = self.time.get_monotonic_time();
        self.need_draw = true;
    }

    pub fn handle_locating_tick(&mut self) {
        let elapsed = (self.time.get_monotonic_time() - self.wait_start).as_millis() as u32;
        let should_be_visible = (elapsed / 500).is_multiple_of(2);
        if self.is_locating_visible != should_be_visible {
            self.is_locating_visible = should_be_visible;
            self.need_draw = true;
        }
    }

    pub fn start_tutorial(&mut self, audio: &mut Audio, lang: Language) {
        self.state = GameState::Tutorial(TutorialStep::Introduction);
        self.wait_start = self.time.get_monotonic_time();
        self.is_locating_visible = false;
        audio.speak_text(match lang {
            Language::Ko => "크로스 메모리 튜토리얼입니다. 이 게임은 중앙을 기준으로 상하좌우에 있는 점의 위치를 기억하는 게임입니다.",
            Language::En => "This is the Cross Memory tutorial. It is a game where you remember the positions of dots located above, below, left, and right of the center.",
            Language::Ja => "クロスメモリーのチュートリアルです。このゲームは、中央を基準にして上下左右にある点の位置を記憶するゲームです。",
        });
        self.need_draw = true;
    }

    pub fn handle_tutorial_tick(&mut self, audio: &mut Audio, lang: Language) {
        let step = match self.state {
            GameState::Tutorial(s) => s,
            _ => return,
        };

        let now = self.time.get_monotonic_time();
        let elapsed = (now - self.wait_start).as_millis() as u32;

        // 사용자 입력 피드백 (200ms 플래시) 처리
        // flash_start >= wait_start 가드로 프로그램적 안내 점은 제외
        if self.active_square.is_some()
            && matches!(
                step,
                TutorialStep::SinglePracticeInit(_)
                    | TutorialStep::SinglePracticeWait(_)
                    | TutorialStep::SequencePracticeInput(_)
                    | TutorialStep::SequencePracticeInit
            )
            && self.flash_start >= self.wait_start
        {
            let flash_elapsed = (now - self.flash_start).as_millis() as u32;
            if flash_elapsed >= INPUT_FLASH_MS {
                self.active_square = None;
                self.need_draw = true;
            }
        }

        match step {
            TutorialStep::Introduction if elapsed > 9000 => {
                self.state = GameState::Tutorial(TutorialStep::IdentifyCross);
                self.wait_start = now;
                self.is_locating_visible = true;
                audio.speak_text(match lang {
                    Language::Ko => "먼저 화면 중앙에 점멸되는 십자가를 찾아 손가락을 올린 뒤 가운데 키를 눌러 주세요",
                    Language::En => "First, find the blinking cross in the center of the screen, place your finger on it, and press the center key.",
                    Language::Ja => "まず、画面中央で点滅している十字を探して指をのせ、中央のキーを押してください。",
                });
                self.need_draw = true;
            }
            TutorialStep::IdentifyCross => {
                let should_be_visible = (elapsed / 500).is_multiple_of(2);
                if self.is_locating_visible != should_be_visible {
                    self.is_locating_visible = should_be_visible;
                    self.need_draw = true;
                }
            }
            TutorialStep::SinglePracticeInit(idx)
                if elapsed > TUTORIAL_STEP_DELAY_MS && idx < ALL_DIRECTIONS.len() =>
            {
                let dir = ALL_DIRECTIONS[idx];
                audio.play(dir.sound_bytes());
                self.active_square = Some(dir);
                self.state = GameState::Tutorial(TutorialStep::SinglePracticeWait(idx));
                self.wait_start = now;
                self.need_draw = true;
            }
            TutorialStep::SinglePracticeWait(idx) => {
                let dir = ALL_DIRECTIONS[idx];
                let name = dir.name();

                // 소리 재생 후 약 1초 뒤에 TTS 안내 시작 (안내 연출용 점 소멸)
                if elapsed > 1000 && self.active_square == Some(dir) {
                    let flash_elapsed = (now - self.flash_start).as_millis() as u32;
                    // 사용자가 방금 입력한 것이 아닐 때만 안내 멘트와 함께 점 복원
                    if flash_elapsed >= INPUT_FLASH_MS {
                        let segments =
                            tts_segment!(name, " 점이 사라졌습니다. ", name, " 키를 눌러보세요.")
                                .into_iter()
                                .map(AudioSegment::Text)
                                .collect::<Vec<_>>();
                        audio.play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                        self.active_square = None;
                        self.need_draw = true;
                    }
                }
            }
            TutorialStep::SequencePracticeInit if elapsed > 500 => {
                self.sequence = vec![Direction::Left, Direction::Right];
                audio.speak_text(match lang {
                    Language::Ko => "이제 두 개의 점이 순서대로 사라집니다. 잘 듣고 기억해 보세요.",
                    Language::En => {
                        "Now two dots will disappear in order. Listen carefully and remember."
                    }
                    Language::Ja => {
                        "これから2つの点が順番に消えます。よく聞いて覚えてみてください。"
                    }
                });
                self.state = GameState::Tutorial(TutorialStep::SequencePracticeShow(0));
                self.wait_start = now;
                self.need_draw = true;
            }
            TutorialStep::SequencePracticeShow(idx) => {
                let wait_ms = if idx == 0 {
                    5000
                } else {
                    TUTORIAL_STEP_DELAY_MS
                };
                if elapsed > wait_ms {
                    if idx < self.sequence.len() {
                        let dir = self.sequence[idx];
                        audio.play(dir.sound_bytes());
                        self.active_square = Some(dir);
                        self.state =
                            GameState::Tutorial(TutorialStep::SequencePracticeShow(idx + 1));
                        self.wait_start = now;
                        self.need_draw = true;
                    } else {
                        audio.speak_text(match lang {
                            Language::Ko => "방금 사라진 순서대로 키를 눌러보세요.",
                            Language::En => "Press the keys in the order they just disappeared.",
                            Language::Ja => "先ほど消えた順番通りにキーを押してみてください。",
                        });
                        self.active_square = None;
                        self.state = GameState::Tutorial(TutorialStep::SequencePracticeInput(0));
                        self.need_draw = true;
                    }
                } else if elapsed > 800 && self.active_square.is_some() {
                    let flash_elapsed = (now - self.flash_start).as_millis() as u32;
                    if flash_elapsed >= INPUT_FLASH_MS {
                        self.active_square = None;
                        self.need_draw = true;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn start_game(&mut self) {
        self.score = 0;
        self.show_on_ms = BASE_SHOW_ON_MS;
        let len = match self.mode {
            GameMode::Length => 1,
            GameMode::Speed | GameMode::Tutorial => SPEED_MODE_SEQ_LEN,
        };
        self.generate_sequence(len);
        self.begin_waiting();
    }

    pub fn schedule_feedback(&mut self, message: Vec<AudioSegment>) {
        let due =
            self.time.get_monotonic_time() + Duration::from_millis(FEEDBACK_TTS_DELAY_MS as u64);
        self.pending_feedback = Some((message, due));
    }

    pub fn take_due_feedback(&mut self) -> Option<Vec<AudioSegment>> {
        let (_, due) = self.pending_feedback.as_ref()?;
        if self.time.get_monotonic_time() >= *due {
            self.pending_feedback.take().map(|(msg, _)| msg)
        } else {
            None
        }
    }

    fn generate_sequence(&mut self, len: usize) {
        self.sequence.clear();
        for _ in 0..len {
            let idx = (self.rng.next_u32() as usize) % ALL_DIRECTIONS.len();
            self.sequence.push(ALL_DIRECTIONS[idx]);
        }
    }

    fn begin_showing(&mut self) {
        self.state = GameState::Showing;
        self.show_index = 0;
        self.active_square = None;
        self.show_step_start = self.time.get_monotonic_time();
        self.need_draw = true;
    }

    pub fn handle_showing_tick(&mut self, audio: &mut Audio) {
        let now = self.time.get_monotonic_time();
        let elapsed_ms = (now - self.show_step_start).as_millis() as u32;
        let step_total = self.show_on_ms + SHOW_OFF_MS;

        if self.show_index >= self.sequence.len() {
            self.begin_input();
            return;
        }

        if elapsed_ms < self.show_on_ms {
            let new_dir = self.sequence[self.show_index];
            let new_active = Some(new_dir);
            if self.active_square != new_active {
                self.active_square = new_active;
                audio.play(new_dir.sound_bytes());
            }
        } else if elapsed_ms < step_total {
            if self.active_square.is_some() {
                self.active_square = None;
            }
        } else {
            self.show_index += 1;
            self.show_step_start = now;
        }
    }

    fn begin_waiting(&mut self) {
        self.state = GameState::Waiting;
        self.active_square = None;
        self.wait_start = self.time.get_monotonic_time();
        self.need_draw = true;
    }

    pub fn handle_waiting_tick(&mut self) {
        let elapsed = (self.time.get_monotonic_time() - self.wait_start).as_millis() as u32;
        if elapsed >= WAIT_DURATION_MS {
            self.begin_showing();
        }
    }

    fn begin_input(&mut self) {
        self.state = GameState::Input;
        self.input_index = 0;
        self.active_square = None;
        self.input_start = self.time.get_monotonic_time();
        self.need_draw = true;
    }

    pub fn remaining_ratio(&self) -> f32 {
        let elapsed_ms = (self.time.get_monotonic_time() - self.input_start).as_millis() as u32;
        if elapsed_ms >= INPUT_TIME_LIMIT_MS {
            0.0
        } else {
            (INPUT_TIME_LIMIT_MS - elapsed_ms) as f32 / INPUT_TIME_LIMIT_MS as f32
        }
    }

    pub fn handle_input_tick(&mut self) {
        if self.active_square.is_some() {
            let flash_elapsed =
                (self.time.get_monotonic_time() - self.flash_start).as_millis() as u32;
            if flash_elapsed >= INPUT_FLASH_MS {
                self.active_square = None;
                match std::mem::take(&mut self.pending) {
                    PendingTransition::NextRound => self.begin_waiting(),
                    PendingTransition::GameOver(msg) => self.end_game(&msg),
                    PendingTransition::None => {}
                }
                return;
            }
        }

        if self.remaining_ratio() <= 0.0 {
            self.end_game("시간 초과");
        }
    }

    pub fn handle_input_key(&mut self, code: KeyCode, audio: &mut Audio) -> InputResult {
        let dir = match keycode_to_direction(code) {
            Some(d) => d,
            None => return InputResult::Ignored,
        };

        self.active_square = Some(dir);
        self.flash_start = self.time.get_monotonic_time();
        audio.play(dir.sound_bytes());

        if dir != self.sequence[self.input_index] {
            self.pending = PendingTransition::GameOver("틀렸습니다".to_string());
            return InputResult::Wrong;
        }

        self.input_index += 1;

        if self.input_index >= self.sequence.len() {
            self.score += 1;
            self.advance_round();
            self.pending = PendingTransition::NextRound;
            InputResult::RoundComplete
        } else {
            InputResult::Correct
        }
    }

    fn advance_round(&mut self) {
        match self.mode {
            GameMode::Length => {
                let new_len = self.sequence.len() + 1;
                self.generate_sequence(new_len);
            }
            GameMode::Speed | GameMode::Tutorial => {
                self.generate_sequence(SPEED_MODE_SEQ_LEN);
                self.show_on_ms = self
                    .show_on_ms
                    .saturating_sub(SHOW_SPEED_REDUCE_MS)
                    .max(MIN_SHOW_ON_MS);
            }
        }
    }

    pub fn end_game(&mut self, message: &str) {
        self.state = GameState::End;
        self.active_square = None;
        self.end_message = message.to_string();
        self.need_draw = true;
    }
}
