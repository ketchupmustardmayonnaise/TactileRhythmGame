//! 리듬 게임 애플릿
//!
//! 위에서 아래로 내려오는 노트를 판정선(judgement line)에 맞춰 누르는 3레인 리듬 게임입니다.
//! - 왼쪽 키  → 0번 레인
//! - 가운데 키 → 1번 레인
//! - 오른쪽 키 → 2번 레인
//!
//! 애플릿 골격(상태 머신 + on_update/on_draw/on_speech/on_help)을 보여주는 것이 목적이므로
//! 채보(chart)는 코드 안에 상수로 박아두었습니다.

use std::time::Duration;

use graphics::{Graphics, style::Style};
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::applet::{LocalizedString, SpeechResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;

/// 레인 개수입니다.
const LANE_COUNT: i16 = 3;
/// 노트가 화면 위에서 판정선까지 내려오는 데 걸리는 시간입니다.
const APPROACH_MS: i64 = 1_500;
/// Perfect 판정 허용 오차입니다.
const PERFECT_MS: i64 = 110;
/// Good 판정 허용 오차입니다.
const GOOD_MS: i64 = 240;
/// 이 시간이 지나도록 누르지 않으면 Miss 처리합니다.
const MISS_MS: i64 = 300;

/// 채보: (노트가 판정선에 도달해야 하는 시각(ms), 레인 번호)
const CHART: &[(i64, usize)] = &[
    (1500, 1),
    (2000, 0),
    (2500, 2),
    (3000, 1),
    (3500, 0),
    (3750, 1),
    (4000, 2),
    (4500, 1),
    (5000, 0),
    (5250, 2),
    (5500, 1),
    (6000, 0),
    (6500, 2),
    (7000, 1),
    (7500, 0),
    (7750, 1),
    (8000, 2),
    (8500, 1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judge {
    Perfect,
    Good,
    Miss,
}

impl Judge {
    fn score(self) -> u32 {
        match self {
            Judge::Perfect => 100,
            Judge::Good => 50,
            Judge::Miss => 0,
        }
    }

    fn text(self, lang: Language) -> &'static str {
        match (self, lang) {
            (Judge::Perfect, Language::Ko) => "퍼펙트",
            (Judge::Good, Language::Ko) => "굿",
            (Judge::Miss, Language::Ko) => "미스",
            (Judge::Perfect, Language::Ja) => "パーフェクト",
            (Judge::Good, Language::Ja) => "グッド",
            (Judge::Miss, Language::Ja) => "ミス",
            (Judge::Perfect, _) => "Perfect",
            (Judge::Good, _) => "Good",
            (Judge::Miss, _) => "Miss",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameState {
    Ready,
    Playing,
    Result,
}

struct Note {
    /// 판정선에 도달해야 하는 시각(곡 시작 기준 ms)
    time_ms: i64,
    lane: usize,
    /// 이미 판정이 끝난 노트인지 여부
    judged: bool,
}

pub struct RhythmGame {
    state: GameState,
    notes: Vec<Note>,
    /// 곡 시작 시점의 모노토닉 시간
    started_at: Duration,
    /// 현재 곡 진행 시간(ms)
    elapsed_ms: i64,
    score: u32,
    combo: u32,
    max_combo: u32,
    perfect: u32,
    good: u32,
    miss: u32,
    /// 마지막 판정 결과 (화면 표시 + TTS 용)
    last_judge: Option<Judge>,
    /// 판정 이펙트를 몇 ms 까지 보여줄지
    effect_until_ms: i64,
    /// 눌린 레인의 하이라이트 표시 (레인별 종료 시각 ms)
    lane_flash_until_ms: [i64; LANE_COUNT as usize],
}

impl Default for RhythmGame {
    fn default() -> Self {
        Self {
            state: GameState::Ready,
            notes: Vec::new(),
            started_at: Duration::ZERO,
            elapsed_ms: 0,
            score: 0,
            combo: 0,
            max_combo: 0,
            perfect: 0,
            good: 0,
            miss: 0,
            last_judge: None,
            effect_until_ms: 0,
            lane_flash_until_ms: [0; LANE_COUNT as usize],
        }
    }
}

impl RhythmGame {
    /// 채보를 다시 깔고 점수를 초기화합니다.
    fn start(&mut self, now: Duration) {
        self.notes = CHART
            .iter()
            .map(|&(time_ms, lane)| Note {
                time_ms,
                lane,
                judged: false,
            })
            .collect();
        self.started_at = now;
        self.elapsed_ms = 0;
        self.score = 0;
        self.combo = 0;
        self.max_combo = 0;
        self.perfect = 0;
        self.good = 0;
        self.miss = 0;
        self.last_judge = None;
        self.effect_until_ms = 0;
        self.lane_flash_until_ms = [0; LANE_COUNT as usize];
        self.state = GameState::Playing;

        log::info!("리듬 게임 시작: 노트 {}개", self.notes.len());
    }

    /// 화면 세로 좌표 기준 판정선 위치입니다.
    fn judge_line_y(height: i16) -> i16 {
        (height - 4).max(0)
    }

    /// 특정 레인에서 지금 누르면 판정될 노트의 인덱스를 찾습니다.
    fn find_hittable(&self, lane: usize) -> Option<usize> {
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.judged && n.lane == lane)
            .filter(|(_, n)| (n.time_ms - self.elapsed_ms).abs() <= GOOD_MS)
            .min_by_key(|(_, n)| (n.time_ms - self.elapsed_ms).abs())
            .map(|(i, _)| i)
    }

    /// 판정 결과를 점수/콤보에 반영합니다.
    fn apply_judge(&mut self, judge: Judge) {
        match judge {
            Judge::Perfect => {
                self.perfect += 1;
                self.combo += 1;
            }
            Judge::Good => {
                self.good += 1;
                self.combo += 1;
            }
            Judge::Miss => {
                self.miss += 1;
                self.combo = 0;
            }
        }
        self.max_combo = self.max_combo.max(self.combo);
        self.score += judge.score();
        self.last_judge = Some(judge);
        self.effect_until_ms = self.elapsed_ms + 250;

        log::debug!(
            "판정 {:?} at {}ms (score={}, combo={})",
            judge,
            self.elapsed_ms,
            self.score,
            self.combo
        );
    }

    /// 레인 키 입력을 처리합니다.
    fn on_lane_pressed(&mut self, lane: usize) {
        self.lane_flash_until_ms[lane] = self.elapsed_ms + 120;

        let Some(index) = self.find_hittable(lane) else {
            // 노트가 없을 때의 헛손질은 감점 없이 무시합니다.
            return;
        };

        let diff = (self.notes[index].time_ms - self.elapsed_ms).abs();
        self.notes[index].judged = true;
        self.apply_judge(if diff <= PERFECT_MS {
            Judge::Perfect
        } else {
            Judge::Good
        });
    }

    /// 판정선을 지나쳐 버린 노트를 Miss 처리합니다.
    fn reap_missed_notes(&mut self) {
        let elapsed = self.elapsed_ms;
        let missed: Vec<usize> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.judged && elapsed - n.time_ms > MISS_MS)
            .map(|(i, _)| i)
            .collect();

        for index in missed {
            self.notes[index].judged = true;
            self.apply_judge(Judge::Miss);
        }
    }

    /// 마지막 노트까지 처리되었는지 확인합니다.
    fn is_finished(&self) -> bool {
        self.notes.iter().all(|n| n.judged)
    }

    /// 레인의 x 시작 좌표와 폭을 계산합니다.
    fn lane_rect(width: i16, lane: usize) -> (i16, i16) {
        let lane_width = width / LANE_COUNT;
        (lane as i16 * lane_width, lane_width)
    }

    /// 노트의 현재 y 좌표입니다. (판정선 기준으로 위에서 내려옴)
    fn note_y(&self, note: &Note, height: i16) -> i16 {
        let judge_y = Self::judge_line_y(height);
        let remain = note.time_ms - self.elapsed_ms;
        let travelled = ((judge_y as i64) * (APPROACH_MS - remain)) / APPROACH_MS;
        travelled as i16
    }
}

impl Applet for RhythmGame {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        // 로거를 초기화해야 log::info! 등이 호스트 콘솔로 전달됩니다.
        let _ = sdk::api::log::init();
        log::info!(
            "rhythm-game on_start: window = {:?}",
            context.window.get_size()
        );
        self.state = GameState::Ready;
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let now = context.time.get_monotonic_time();
        let mut needs_redraw = false;

        // 1) 입력 처리: 큐에 쌓인 키 이벤트를 모두 소진합니다.
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state != KeyState::Pressed {
                continue;
            }

            match (self.state, event.code) {
                (GameState::Ready, KeyCode::Center) | (GameState::Result, KeyCode::Center) => {
                    self.start(now);
                    needs_redraw = true;
                }
                (GameState::Playing, KeyCode::Left) => {
                    self.on_lane_pressed(0);
                    needs_redraw = true;
                }
                (GameState::Playing, KeyCode::Center) => {
                    self.on_lane_pressed(1);
                    needs_redraw = true;
                }
                (GameState::Playing, KeyCode::Right) => {
                    self.on_lane_pressed(2);
                    needs_redraw = true;
                }
                (_, KeyCode::Menu) => {
                    // 메뉴 키로 애플릿을 종료합니다.
                    return Ok(UpdateResult::ExitApp);
                }
                _ => {}
            }
        }

        // 2) 시간 진행 및 상태 갱신
        if self.state == GameState::Playing {
            self.elapsed_ms = now.saturating_sub(self.started_at).as_millis() as i64;
            self.reap_missed_notes();

            if self.is_finished() {
                self.state = GameState::Result;
                log::info!(
                    "게임 종료: score={}, max_combo={}, P/G/M = {}/{}/{}",
                    self.score,
                    self.max_combo,
                    self.perfect,
                    self.good,
                    self.miss
                );
            }

            // 노트가 계속 내려오므로 매 프레임 다시 그립니다.
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        let Size { width, height } = canvas.get_size();
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let judge_y = Self::judge_line_y(height);

        match self.state {
            GameState::Ready => {
                // 시작 안내: 판정선만 보여줍니다.
                canvas.draw_line(
                    Point::new(0, judge_y),
                    Point::new(width - 1, judge_y),
                    Style::with_stroke(Intensity::new(160), 1),
                );
            }
            GameState::Playing => {
                // 레인 구분선
                for lane in 1..LANE_COUNT {
                    let (x, _) = Self::lane_rect(width, lane as usize);
                    canvas.draw_line(
                        Point::new(x, 0),
                        Point::new(x, judge_y),
                        Style::with_stroke(Intensity::new(60), 1),
                    );
                }

                // 판정선 (눌린 레인은 더 강하게)
                for lane in 0..LANE_COUNT as usize {
                    let (x, w) = Self::lane_rect(width, lane);
                    let pressed = self.elapsed_ms < self.lane_flash_until_ms[lane];
                    let intensity = if pressed {
                        Intensity::MAX
                    } else {
                        Intensity::new(120)
                    };
                    canvas.draw_line(
                        Point::new(x, judge_y),
                        Point::new(x + w - 1, judge_y),
                        Style::with_stroke(intensity, 1),
                    );
                }

                // 노트
                for note in self.notes.iter().filter(|n| !n.judged) {
                    let y = self.note_y(note, height);
                    if y < -2 || y > judge_y {
                        continue;
                    }
                    let (x, w) = Self::lane_rect(width, note.lane);
                    canvas.draw_rectangle(
                        Point::new(x + 1, y),
                        Size::new(w - 2, 2),
                        Style::with_fill(Intensity::MAX),
                    );
                }

                // 판정 이펙트: 최근 판정이 있으면 화면 위쪽에 짧게 표시
                if self.elapsed_ms < self.effect_until_ms
                    && let Some(judge) = self.last_judge
                {
                    let bar_width = match judge {
                        Judge::Perfect => width,
                        Judge::Good => width / 2,
                        Judge::Miss => width / 4,
                    };
                    canvas.draw_rectangle(
                        Point::new(0, 0),
                        Size::new(bar_width, 1),
                        Style::with_fill(Intensity::MAX),
                    );
                }
            }
            GameState::Result => {
                // 결과 화면: 최대 콤보를 막대 길이로 촉각 표현
                let ratio = if CHART.is_empty() {
                    0
                } else {
                    (self.max_combo as i16 * width) / CHART.len() as i16
                };
                canvas.draw_rectangle(
                    Point::new(0, height / 2 - 1),
                    Size::new(ratio.max(1), 3),
                    Style::with_fill(Intensity::MAX),
                );
                canvas.draw_line(
                    Point::new(0, judge_y),
                    Point::new(width - 1, judge_y),
                    Style::with_stroke(Intensity::new(160), 1),
                );
            }
        }

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        // on_speech 는 매 프레임 호출되며, 반환값이 "직전과 달라졌을 때만" 음성이 나갑니다.
        // 따라서 플레이 중에 매번 바뀌는 값(점수 등)을 그대로 넣으면 TTS가 끊임없이 겹칩니다.
        match self.state {
            GameState::Ready => SpeechResult::text(match context.language {
                Language::Ko => "리듬 게임. 가운데 키를 눌러 시작하세요.",
                Language::Ja => "リズムゲーム。中央キーを押して開始してください。",
                _ => "Rhythm game. Press the center key to start.",
            }),
            GameState::Playing => {
                // 판정 이펙트가 떠 있는 동안만 판정명을 읽어 줍니다.
                // (같은 판정이 연속되면 값이 바뀌지 않으므로 음성이 다시 나가지 않습니다.
                //  TTS 지연이 거슬리면 이 분기를 통째로 `SpeechResult::None` 으로 바꾸세요.)
                match self.last_judge {
                    Some(judge) if self.elapsed_ms < self.effect_until_ms => {
                        SpeechResult::text(judge.text(context.language))
                    }
                    _ => SpeechResult::None,
                }
            }
            GameState::Result => SpeechResult::text(match context.language {
                Language::Ko => format!(
                    "게임 종료. 점수 {}점, 최대 콤보 {}. 퍼펙트 {}, 굿 {}, 미스 {}. 다시 하려면 가운데 키를 누르세요.",
                    self.score, self.max_combo, self.perfect, self.good, self.miss
                ),
                Language::Ja => format!(
                    "ゲーム終了。スコア{}点、最大コンボ{}。もう一度は中央キーです。",
                    self.score, self.max_combo
                ),
                _ => format!(
                    "Game over. Score {}, max combo {}. Perfect {}, good {}, miss {}. Press the center key to retry.",
                    self.score, self.max_combo, self.perfect, self.good, self.miss
                ),
            }),
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "리듬 게임입니다. 가운데 키로 시작하고, 노트가 판정선에 닿을 때 왼쪽, 가운데, 오른쪽 키를 누르세요. 메뉴 키를 누르면 종료합니다."
                .to_string(),
            en: "Rhythm game. Press center to start, then press left, center or right when a note reaches the judgement line. Press menu to exit."
                .to_string(),
            ja: "リズムゲームです。中央キーで開始し、ノートが判定線に来たら左、中央、右キーを押してください。メニューキーで終了します。"
                .to_string(),
        }
    }
}

/// WASM 런타임 진입점 함수 정의
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(RhythmGame::default()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::api::display::Canvas;

    /// `on_draw` 는 `DisplayInterface` 하나만 받는 순수 함수이므로,
    /// 런타임 없이 오프스크린 `Canvas` 에 그려서 화면을 그대로 들여다볼 수 있습니다.
    /// 핀 강도를 4단계 문자로 찍어 터미널에서 눈으로 확인합니다.
    fn render_ascii(game: &RhythmGame, size: Size) -> String {
        let mut canvas = Canvas::new(size);
        game.on_draw(&mut canvas).expect("on_draw 실패");

        let mut out = String::new();
        for y in 0..size.height {
            for x in 0..size.width {
                let v = canvas.get_pin(Point::new(x, y)).value;
                out.push(match v {
                    0 => '.',
                    1..=99 => '-',
                    100..=199 => '+',
                    _ => '#',
                });
            }
            out.push('\n');
        }
        out
    }

    /// `cargo test -p rhythm-game -- --nocapture snapshot` 으로 화면을 직접 볼 수 있습니다.
    #[test]
    fn snapshot_playing_frame() {
        let mut game = RhythmGame::default();
        game.start(Duration::ZERO);
        game.elapsed_ms = 2_400; // 노트 몇 개가 내려오는 중인 시점

        let frame = render_ascii(&game, Size::new(48, 32));
        println!("\n--- Playing @ {}ms ---\n{}", game.elapsed_ms, frame);

        // 눈으로 보는 것과 별개로, 최소한의 자동 검증도 겁니다.
        let judge_row = frame.lines().nth(28).expect("판정선 행이 있어야 합니다");
        assert!(
            judge_row.chars().filter(|&c| c != '.').count() > 40,
            "판정선이 화면 폭 전체에 그려져야 합니다: {judge_row}"
        );
    }

    /// 판정 로직은 WASM 없이도 `cargo test -p rhythm-game` 으로 바로 검증할 수 있습니다.
    #[test]
    fn perfect_and_good_windows() {
        let mut game = RhythmGame::default();
        game.start(Duration::ZERO);

        // 첫 노트(1500ms, 레인 1)를 정확히 맞춘 경우
        game.elapsed_ms = 1500;
        game.on_lane_pressed(1);
        assert_eq!(game.perfect, 1);
        assert_eq!(game.combo, 1);

        // 두 번째 노트(2000ms, 레인 0)를 200ms 늦게 친 경우 → Good
        game.elapsed_ms = 2200;
        game.on_lane_pressed(0);
        assert_eq!(game.good, 1);
        assert_eq!(game.combo, 2);
    }

    #[test]
    fn missed_note_breaks_combo() {
        let mut game = RhythmGame::default();
        game.start(Duration::ZERO);

        game.elapsed_ms = 1500;
        game.on_lane_pressed(1);
        assert_eq!(game.combo, 1);

        // 두 번째 노트를 그냥 지나쳐 버립니다.
        game.elapsed_ms = 2000 + MISS_MS + 1;
        game.reap_missed_notes();
        assert_eq!(game.miss, 1);
        assert_eq!(game.combo, 0);
        assert_eq!(game.max_combo, 1);
    }
}
