use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::Point;
use sdk::api::keypad::KeyCode;
use sdk::api::time::Time;
use sdk::event::UpdateResult;
use sdk::tts_segment;
use std::time::Duration;

use crate::rng::Rng;
use braille::Label;

pub const BLINK_INTERVAL_FRAMES: u32 = 30;
/// 기본 제한 시간 (3분 = 180,000ms)
pub const BASE_TIME_LIMIT_MS: u32 = 180_000;
pub const TIME_REDUCE_MS: u32 = 500;
pub const MIN_TIME_LIMIT_MS: u32 = 5_000;
pub const SCORE_PER_DIFFICULTY: u32 = 10;
pub const PROGRESS_BAR_HEIGHT: i16 = 2;
pub const PROGRESS_BAR_GAP: i16 = 1;
pub const PLAY_PADDING: i16 = 1;
pub const COLLISION_DIST_SQ: i32 = 5;

const REPEAT_DELAY: Duration = Duration::from_millis(100);
const REPEAT_INTERVAL: Duration = Duration::from_millis(25);
const KEY_ALIVE_TIMEOUT: Duration = Duration::from_millis(300);
const SOUND_EFFECT_DURATION: Duration = Duration::from_millis(700);
const FIND_SOUND: &[u8] = include_bytes!("../assets/sounds/find.mp3");

#[derive(Default, PartialEq)]
pub enum GameState {
    #[default]
    Ready,
    Playing,
    End,
}

pub struct FindDot {
    pub state: GameState,
    pub need_draw: bool,
    pub cursor_position: Point,
    pub dot_position: Point,
    pub frame_count: u32,
    pub dot_visible: bool,
    pub score: u32,
    pub label: Label,
    rng: Rng,
    time: Time,
    round_start: Duration,
    time_limit_ms: u32,
    up_pressed: bool,
    down_pressed: bool,
    left_pressed: bool,
    right_pressed: bool,
    held_since: Duration,
    last_repeat: Duration,
    last_key_event: Duration,
    heartbeat_armed: bool,
    pending_tts: Option<(Vec<AudioSegment>, Duration)>,
}

impl Default for FindDot {
    fn default() -> Self {
        Self {
            state: GameState::default(),
            need_draw: false,
            cursor_position: Point::default(),
            dot_position: Point::default(),
            frame_count: 0,
            dot_visible: false,
            score: 0,
            label: Label::default().scroll_speed(45),
            rng: Rng::new(),
            time: Time::new(),
            round_start: Duration::ZERO,
            time_limit_ms: BASE_TIME_LIMIT_MS,
            up_pressed: false,
            down_pressed: false,
            left_pressed: false,
            right_pressed: false,
            held_since: Duration::ZERO,
            last_repeat: Duration::ZERO,
            last_key_event: Duration::ZERO,
            heartbeat_armed: false,
            pending_tts: None,
        }
    }
}

fn calc_time_limit(score: u32) -> u32 {
    let reduce = (score / SCORE_PER_DIFFICULTY) * TIME_REDUCE_MS;
    BASE_TIME_LIMIT_MS
        .saturating_sub(reduce)
        .max(MIN_TIME_LIMIT_MS)
}

impl FindDot {
    fn random_point(&mut self, width: i16, height: i16) -> Point {
        let x_span = (width - 2 * PLAY_PADDING).max(1) as u32;
        let y_span = (height - PLAY_PADDING).max(1) as u32;
        let x = PLAY_PADDING + (self.rng.next_u32() % x_span) as i16;
        let y = PLAY_PADDING + (self.rng.next_u32() % y_span) as i16;
        Point::new(x, y)
    }

    fn spawn_dot(&mut self, width: i16, height: i16) {
        loop {
            let pt = self.random_point(width, height);
            let dist_x = (self.cursor_position.x - pt.x).abs() as i32;
            let dist_y = (self.cursor_position.y - pt.y).abs() as i32;
            if dist_x * dist_x + dist_y * dist_y > COLLISION_DIST_SQ {
                self.dot_position = pt;
                break;
            }
        }
        self.dot_visible = true;
        self.frame_count = 0;
    }

    pub fn game_area_height(&self, display_height: i16) -> i16 {
        display_height - PROGRESS_BAR_HEIGHT - PROGRESS_BAR_GAP
    }

    fn start_round(&mut self) {
        self.round_start = self.time.get_monotonic_time();
        self.time_limit_ms = calc_time_limit(self.score);
    }

    pub fn remaining_ratio(&self) -> f32 {
        let elapsed = self.time.get_monotonic_time() - self.round_start;
        let elapsed_ms = elapsed.as_millis() as u32;
        if elapsed_ms >= self.time_limit_ms {
            0.0
        } else {
            (self.time_limit_ms - elapsed_ms) as f32 / self.time_limit_ms as f32
        }
    }

    pub fn start_game(&mut self, display_width: i16, game_area_height: i16) {
        self.state = GameState::Playing;
        self.cursor_position = Point::new(display_width / 2, game_area_height / 2);
        self.spawn_dot(display_width, game_area_height);
        self.score = 0;
        self.up_pressed = false;
        self.down_pressed = false;
        self.left_pressed = false;
        self.right_pressed = false;
        self.heartbeat_armed = false;
        self.start_round();
    }

    pub fn handle_playing_input(
        &mut self,
        context: &mut Context,
        display_width: i16,
        game_area_height: i16,
    ) -> bool {
        let mut dx = 0;
        let mut dy = 0;

        if self.up_pressed && self.cursor_position.y > PLAY_PADDING {
            dy -= 1;
        }
        if self.down_pressed && self.cursor_position.y < game_area_height - 1 {
            dy += 1;
        }
        if self.left_pressed && self.cursor_position.x > PLAY_PADDING {
            dx -= 1;
        }
        if self.right_pressed && self.cursor_position.x < display_width - 1 - PLAY_PADDING {
            dx += 1;
        }

        if dx != 0 || dy != 0 {
            self.cursor_position.x += dx;
            self.cursor_position.y += dy;

            let dist_x = (self.cursor_position.x - self.dot_position.x).abs() as i32;
            let dist_y = (self.cursor_position.y - self.dot_position.y).abs() as i32;
            let dist_sq = dist_x * dist_x + dist_y * dist_y;

            if dist_sq <= COLLISION_DIST_SQ {
                self.score += 1;
                context.audio.play(FIND_SOUND);
                let speak_at = self.time.get_monotonic_time() + SOUND_EFFECT_DURATION;
                let score_str = self.score.to_string();
                let segments = tts_segment!(score_str, "점")
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect::<Vec<_>>();
                self.pending_tts = Some((segments, speak_at));
                self.spawn_dot(display_width, game_area_height);
                self.start_round();
            }
            return true;
        }

        false
    }

    pub fn on_key_pressed(&mut self, code: KeyCode) {
        let now = self.time.get_monotonic_time();
        match code {
            KeyCode::Up => self.up_pressed = true,
            KeyCode::Down => self.down_pressed = true,
            KeyCode::Left => self.left_pressed = true,
            KeyCode::Right => self.right_pressed = true,
            _ => {}
        }
        self.held_since = now;
        self.last_repeat = now;
        self.last_key_event = now;
        self.heartbeat_armed = false;
    }

    pub fn on_key_released(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => self.up_pressed = false,
            KeyCode::Down => self.down_pressed = false,
            KeyCode::Left => self.left_pressed = false,
            KeyCode::Right => self.right_pressed = false,
            _ => {}
        }
        self.heartbeat_armed = false;
    }

    pub fn poll_repeat(&mut self) -> bool {
        let any_direction =
            self.up_pressed || self.down_pressed || self.left_pressed || self.right_pressed;
        if !any_direction {
            return false;
        }

        let now = self.time.get_monotonic_time();

        if self.heartbeat_armed && now - self.last_key_event > KEY_ALIVE_TIMEOUT {
            self.up_pressed = false;
            self.down_pressed = false;
            self.left_pressed = false;
            self.right_pressed = false;
            self.heartbeat_armed = false;
            return false;
        }

        if now - self.held_since < REPEAT_DELAY {
            return false;
        }
        if now - self.last_repeat >= REPEAT_INTERVAL {
            self.last_repeat = now;
            true
        } else {
            false
        }
    }

    pub fn handle_playing_tick(&mut self, context: &mut Context) -> UpdateResult {
        if self.remaining_ratio() <= 0.0 {
            self.state = GameState::End;
            self.pending_tts = None;
            let score_str = self.score.to_string();
            let segments = match context.language {
                sdk::Language::Ko => tts_segment!(
                    "게임 종료. ",
                    score_str,
                    "점. 다시 시작하시려면 가운데 키를 눌러주세요."
                ),
                sdk::Language::En => tts_segment!(
                    "Game Over. ",
                    score_str,
                    " points. Press center key to restart."
                ),
                sdk::Language::Ja => tts_segment!(
                    "ゲーム終了。 ",
                    score_str,
                    "点。再起動するには中央キーを押してください。"
                ),
            }
            .into_iter()
            .map(AudioSegment::Text)
            .collect::<Vec<_>>();
            context
                .audio
                .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
            return UpdateResult::NeedsRedraw;
        }
        if let Some((_, speak_at)) = &self.pending_tts
            && self.time.get_monotonic_time() >= *speak_at
            && let Some((segments, _)) = self.pending_tts.take()
        {
            context
                .audio
                .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
        }
        self.frame_count += 1;
        if self.frame_count >= BLINK_INTERVAL_FRAMES {
            self.frame_count = 0;
            self.dot_visible = !self.dot_visible;
        }
        UpdateResult::NeedsRedraw
    }
}
