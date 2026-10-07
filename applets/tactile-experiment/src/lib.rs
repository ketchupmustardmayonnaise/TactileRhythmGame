//! 촉각 반응 시간 실험 애플릿
//!
//! 화면 중앙의 커다란 원(지름 22)을 진동시켜 "곧 칠 시각"을 예고하고,
//! 사용자가 **왼쪽 가운데 키**(웹 시뮬레이터의 `Space`)를 누른 시각이 목표 시각 `t` 와
//! 얼마나 차이 나는지를 기록합니다. 이를 `TRIAL_COUNT`(20)회 반복합니다.
//!
//! 한 회차의 시간축
//!
//! ```text
//!   cue_on = t - (예고 200ms + 전송 시간)        t                t + 응답 허용
//!   ───────┬──────────────────────────────────────┬─────────────────┬──────────
//!          │◀──────────── 원 진동 ─────────────▶│                 │
//!          │◀────────────── 키 입력 기록 구간 ─────────────────────▶│
//! ```
//!
//! * 진동은 `cue_on` 에 켜고 `t` 에 끕니다. 그 사이에 키를 눌러도 `t` 까지 유지합니다.
//! * 전송 시간은 원을 켤 때 바뀌는 핀 수로 계산합니다(`transmission_ms`).
//! * `cue_on` 보다 먼저 누른 것은 느끼고 친 것이 아니므로 무시합니다.
//! * `t + RESPONSE_WINDOW_MS` 까지 누르지 않으면 미응답(miss)으로 기록합니다.
//!
//! 결과는 매 회차마다 `RESULT_KEY` 에 CSV 로 저장되며, PC 에서 USB 로 받아갈 수 있습니다.
//!
//! ```text
//! cargo run -p device-info -- experiment -o result.csv
//! ```

use std::time::Duration;

use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult, KeypadSide};
use sdk::applet::{LocalizedString, SpeechResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;

// ---------------------------------------------------------------------------
// 실험 상수
// ---------------------------------------------------------------------------

/// 반복 횟수
const TRIAL_COUNT: usize = 20;

/// 진동하는 원의 지름(핀 개수)
const CUE_DIAMETER: i16 = 22;

/// 예고 시간(ms). 진동은 `t - (이 값 + 전송 시간)` 에 켜집니다.
const CUE_LEAD_MS: f64 = 200.0;

/// 진동 세기. 4비트 코드 4 = 약 62.5Hz 반송파의 듀티 50% 입니다.
///
/// 코드 7(`Intensity::MAX`)은 펌웨어가 "항상 켜짐"으로 처리해 핀이 그냥 올라와 있을 뿐
/// 떨리지 않으므로 쓰지 않습니다(`applets/rhythm-game` 의 단계 표 참고).
/// 런타임이 `(v*7+127)/255` 로 양자화하므로 `4*255/7 = 145` 가 코드 4 입니다.
const CUE_INTENSITY: Intensity = Intensity {
    value: 145,
    blink: false,
};

/// `t` 이후 이 시간까지 누르지 않으면 미응답으로 기록합니다.
const RESPONSE_WINDOW_MS: u64 = 1_000;

/// 한 회차가 끝난 뒤 다음 진동이 켜질 때까지의 간격(ms) 범위.
/// 간격이 일정하면 박자를 외워 예측할 수 있으므로 매번 무작위로 정합니다.
const GAP_MIN_MS: u64 = 2_000;
const GAP_MAX_MS: u64 = 4_000;

/// ESP32 로 가는 UART 의 초당 바이트 수 (115,200bps, 8N1 → 10비트/바이트).
/// `runtime-native/src/main.rs` 의 `UartComm::new("/dev/ttyAMA0", 115_200)`.
const UART_BYTES_PER_SEC: f64 = 11_520.0;

/// 차이 전송에서 핀 하나가 차지하는 바이트 수.
///
/// 핀마다 `x(6) | y(6) | 세기(4)` 를 u16 하나로 묶고(`runtime-native/src/hal/display/mod.rs`),
/// postcard 가 varint 로 보냅니다. `x >= 16` 이면 값이 2^14 이상이라 3바이트가 됩니다.
/// 원이 화면 가운데에 있으므로 거의 모든 핀이 3바이트입니다.
const DIFF_BYTES_PER_PIN: f64 = 3.0;

/// 결과 CSV 를 저장하는 preferences 키. `tools/device-info` 의 `experiment` 명령이 이 키를 읽습니다.
const RESULT_KEY: &str = "tactile-experiment.results";

/// 키 입력 시각의 해상도는 프레임 간격입니다. 기본 90fps(약 11ms)보다 촘촘하게 돌립니다.
const TARGET_FPS: u16 = 200;

// ---------------------------------------------------------------------------
// 전송 시간 계산
// ---------------------------------------------------------------------------

/// 지름 `diameter` 원 안에 드는 핀 좌표(원 중심 기준 오프셋)를 돌려줍니다.
fn disc_offsets(diameter: i16) -> impl Iterator<Item = (i16, i16)> {
    let c = (diameter as f32 - 1.0) / 2.0;
    let r = diameter as f32 / 2.0;
    (0..diameter).flat_map(move |y| {
        (0..diameter).filter_map(move |x| {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            (dx * dx + dy * dy <= r * r).then_some((x - diameter / 2, y - diameter / 2))
        })
    })
}

/// 원을 켤 때 바뀌는 핀 수
fn cue_pin_count() -> usize {
    disc_offsets(CUE_DIAMETER).count()
}

/// 핀 `changed_pins` 개를 바꾸는 프레임이 ESP32 까지 가는 데 걸리는 시간(ms).
///
/// 런타임(`runtime-native/src/hal/display/braille_display.rs`)과 같은 규칙을 씁니다.
/// 바뀐 핀 수 × 2 가 전체 프레임 크기(4비트 패킹 = 핀 수 / 2 바이트) 이상이면
/// 전체 프레임을, 아니면 바뀐 핀만 보냅니다.
fn transmission_ms(changed_pins: usize, display: Size) -> f64 {
    let total_pins = (display.width.max(0) as usize) * (display.height.max(0) as usize);
    let full_frame_bytes = total_pins.div_ceil(2);
    let bytes = if changed_pins * 2 >= full_frame_bytes {
        full_frame_bytes as f64
    } else {
        changed_pins as f64 * DIFF_BYTES_PER_PIN
    };
    bytes * 1000.0 / UART_BYTES_PER_SEC
}

// ---------------------------------------------------------------------------
// 난수 (회차 간격용)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    /// `[lo, hi]` 범위의 정수
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u32() as u64 % (hi - lo + 1)
    }
}

// ---------------------------------------------------------------------------
// 실험 진행
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// 안내 화면. 가운데 키로 시작합니다.
    Ready,
    Running,
    /// 20회 완료. 가운데 키로 다시 시작합니다.
    Done,
}

/// 한 회차의 기록. 시각은 모두 실험 시작 기준입니다.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrialRecord {
    /// 목표 시각 `t`
    target: Duration,
    /// 진동을 켠 시각 `t - (예고 + 전송)`
    cue_on: Duration,
    /// 키를 누른 시각. 미응답이면 `None`.
    press: Option<Duration>,
}

impl TrialRecord {
    /// `누른 시각 - t` (ms). 양수 = 늦게 누름.
    fn diff_ms(&self) -> Option<f64> {
        self.press
            .map(|p| p.as_secs_f64() * 1000.0 - self.target.as_secs_f64() * 1000.0)
    }
}

/// 진행 중인 회차
#[derive(Debug, Clone, Copy)]
struct PendingTrial {
    target: Duration,
    cue_on: Duration,
    press: Option<Duration>,
}

impl PendingTrial {
    /// 다음 회차로 넘어가도 되는 시각. 진동이 `t` 에 꺼진 뒤여야 합니다.
    fn can_close(&self, now: Duration) -> bool {
        let deadline = self.target + Duration::from_millis(RESPONSE_WINDOW_MS);
        (self.press.is_some() && now >= self.target) || now > deadline
    }
}

pub struct TactileExperiment {
    phase: Phase,
    rng: Rng,
    /// 진동 시작을 얼마나 앞당길지(ms) = 예고 + 전송 시간
    advance_ms: f64,
    /// 전송 시간(ms). 기록용.
    tx_ms: f64,
    /// 실험을 시작한 모노토닉 시각. 모든 기록의 기준점입니다.
    started_at: Duration,
    /// 실험 시작 시각(유닉스 초). CSV 머리말용.
    started_unix: u64,
    current: Option<PendingTrial>,
    records: Vec<TrialRecord>,
    /// 진동이 켜져 있는지 (on_draw 에서 씁니다)
    cue_active: bool,
}

impl Default for TactileExperiment {
    fn default() -> Self {
        let tx_ms = transmission_ms(cue_pin_count(), Size::new(0, 0));
        Self {
            phase: Phase::Ready,
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
            advance_ms: CUE_LEAD_MS + tx_ms,
            tx_ms,
            started_at: Duration::ZERO,
            started_unix: 0,
            current: None,
            records: Vec::with_capacity(TRIAL_COUNT),
            cue_active: false,
        }
    }
}

impl TactileExperiment {
    /// 화면 크기에 맞춰 전송 시간과 진동 선행 시간을 다시 계산합니다.
    fn set_display_size(&mut self, size: Size) {
        self.tx_ms = transmission_ms(cue_pin_count(), size);
        self.advance_ms = CUE_LEAD_MS + self.tx_ms;
    }

    fn start(&mut self, now: Duration, unix_secs: u64) {
        self.phase = Phase::Running;
        self.started_at = now;
        self.started_unix = unix_secs;
        self.records.clear();
        self.current = None;
        self.cue_active = false;
        self.schedule_next(now);
        log::info!(
            "실험 시작: {TRIAL_COUNT}회, 예고 {CUE_LEAD_MS}ms + 전송 {:.1}ms (원 지름 {CUE_DIAMETER}, 핀 {}개)",
            self.tx_ms,
            cue_pin_count()
        );
    }

    /// 지금부터 무작위 간격 뒤에 진동이 켜지도록 다음 회차를 잡습니다.
    fn schedule_next(&mut self, now: Duration) {
        let gap = Duration::from_millis(self.rng.range(GAP_MIN_MS, GAP_MAX_MS));
        let cue_on = now + gap;
        let target = cue_on + Duration::from_secs_f64(self.advance_ms / 1000.0);
        self.current = Some(PendingTrial {
            target,
            cue_on,
            press: None,
        });
    }

    /// 왼쪽 가운데 키 입력. 기록되면 `true`.
    fn press(&mut self, now: Duration) -> bool {
        let Some(trial) = self.current.as_mut() else {
            return false;
        };
        if self.phase != Phase::Running || trial.press.is_some() {
            return false;
        }
        // 진동이 나가기 전에 누른 것은 느끼고 친 게 아니므로 무시합니다.
        if now < trial.cue_on {
            log::info!(
                "진동 전 입력 무시 (진동까지 {:.0}ms 남음)",
                (trial.cue_on - now).as_secs_f64() * 1000.0
            );
            return false;
        }
        let deadline = trial.target + Duration::from_millis(RESPONSE_WINDOW_MS);
        if now > deadline {
            return false;
        }
        trial.press = Some(now);
        true
    }

    /// 시간을 진행시킵니다. 회차가 하나 끝났으면 그 기록을 돌려줍니다.
    fn tick(&mut self, now: Duration) -> Option<TrialRecord> {
        if self.phase != Phase::Running {
            self.cue_active = false;
            return None;
        }
        let trial = self.current?;

        // 진동 구간은 정확히 [cue_on, t) 입니다.
        self.cue_active = now >= trial.cue_on && now < trial.target;

        if !trial.can_close(now) {
            return None;
        }

        let record = TrialRecord {
            target: trial.target - self.started_at,
            cue_on: trial.cue_on - self.started_at,
            press: trial.press.map(|p| p - self.started_at),
        };
        self.records.push(record);
        self.cue_active = false;

        match record.diff_ms() {
            Some(d) => log::info!("[{}/{TRIAL_COUNT}] 차이 {d:+.1}ms", self.records.len()),
            None => log::info!("[{}/{TRIAL_COUNT}] 미응답", self.records.len()),
        }

        if self.records.len() >= TRIAL_COUNT {
            self.current = None;
            self.phase = Phase::Done;
            log::info!("실험 완료\n{}", self.to_csv());
        } else {
            self.schedule_next(now);
        }
        Some(record)
    }

    fn abort(&mut self) {
        self.phase = Phase::Ready;
        self.current = None;
        self.cue_active = false;
    }

    fn responded(&self) -> impl Iterator<Item = f64> + '_ {
        self.records.iter().filter_map(TrialRecord::diff_ms)
    }

    fn mean_diff_ms(&self) -> Option<f64> {
        let (sum, n) = self
            .responded()
            .fold((0.0, 0usize), |(s, n), d| (s + d, n + 1));
        (n > 0).then(|| sum / n as f64)
    }

    fn mean_abs_diff_ms(&self) -> Option<f64> {
        let (sum, n) = self
            .responded()
            .fold((0.0, 0usize), |(s, n), d| (s + d.abs(), n + 1));
        (n > 0).then(|| sum / n as f64)
    }

    /// PC 로 넘길 결과. `#` 줄은 조건, 나머지는 회차별 기록입니다.
    fn to_csv(&self) -> String {
        let ms = |d: Duration| format!("{:.1}", d.as_secs_f64() * 1000.0);
        let mut out = format!(
            "# tactile-experiment started_unix={} trials={}/{} lead_ms={} tx_ms={:.1} \
             diameter={} pins={} intensity={} window_ms={}\n\
             trial,target_ms,cue_on_ms,press_ms,diff_ms\n",
            self.started_unix,
            self.records.len(),
            TRIAL_COUNT,
            CUE_LEAD_MS,
            self.tx_ms,
            CUE_DIAMETER,
            cue_pin_count(),
            CUE_INTENSITY.value,
            RESPONSE_WINDOW_MS,
        );
        for (i, r) in self.records.iter().enumerate() {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                i + 1,
                ms(r.target),
                ms(r.cue_on),
                r.press.map(ms).unwrap_or_default(),
                r.diff_ms().map(|d| format!("{d:.1}")).unwrap_or_default(),
            ));
        }
        out
    }

    fn save(&self, context: &Context) {
        if let Err(e) = context.preferences.set_string(RESULT_KEY, &self.to_csv()) {
            log::warn!("결과 저장 실패: {e:?}");
        }
    }
}

/// 원을 채웁니다. 진동 시점과 전송 시간 계산이 같은 핀 집합을 쓰도록 `disc_offsets` 를 씁니다.
fn fill_disc(canvas: &mut dyn DisplayInterface, center: Point, intensity: Intensity) {
    for (dx, dy) in disc_offsets(CUE_DIAMETER) {
        canvas.set_pin(Point::new(center.x + dx, center.y + dy), intensity);
    }
}

/// 원의 테두리만 그립니다. 대기 화면에서 손가락 놓을 자리를 알려 주는 용도입니다.
fn outline_disc(canvas: &mut dyn DisplayInterface, center: Point) {
    let pins: Vec<(i16, i16)> = disc_offsets(CUE_DIAMETER).collect();
    for &(dx, dy) in &pins {
        let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(ox, oy)| !pins.contains(&(dx + ox, dy + oy)));
        if edge {
            canvas.set_pin(Point::new(center.x + dx, center.y + dy), Intensity::MAX);
        }
    }
}

impl Applet for TactileExperiment {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        let size = context.window.get_size();
        self.set_display_size(size);
        self.rng = Rng::new(context.time.get_monotonic_time().as_nanos() as u64);
        log::info!(
            "tactile-experiment 시작: 화면 {size:?}, 원 핀 {}개, 전송 {:.1}ms → 진동은 t - {:.1}ms 에 켜짐",
            cue_pin_count(),
            self.tx_ms,
            self.advance_ms
        );
        Ok(())
    }

    fn target_fps(&self) -> u16 {
        TARGET_FPS
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let now = context.time.get_monotonic_time();
        let was_cue = self.cue_active;
        let mut needs_redraw = false;

        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state != KeyState::Pressed {
                continue;
            }
            match (self.phase, event.code, event.side) {
                // 측정 키: 왼쪽 가운데 키
                (Phase::Running, KeyCode::Center, KeypadSide::Left) => {
                    self.press(now);
                }
                (Phase::Running, KeyCode::Menu, _) => {
                    log::info!("실험 중단 ({}회까지 저장됨)", self.records.len());
                    self.abort();
                    needs_redraw = true;
                }
                (Phase::Ready | Phase::Done, KeyCode::Center, _) => {
                    self.start(now, context.time.get_time_seconds());
                    self.save(context);
                    needs_redraw = true;
                }
                (Phase::Ready | Phase::Done, KeyCode::Menu, _) => {
                    return Ok(UpdateResult::ExitApp);
                }
                _ => {}
            }
        }

        if self.tick(now).is_some() {
            self.save(context);
            needs_redraw = true;
        }
        if self.cue_active != was_cue {
            needs_redraw = true;
        }

        Ok(if needs_redraw {
            UpdateResult::NeedsRedraw
        } else {
            UpdateResult::Unchanged
        })
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();
        let size = canvas.get_size();
        let center = Point::new(size.width / 2, size.height / 2);

        match self.phase {
            // 진행 중에는 진동할 때만 원을 그립니다. 그 외에는 화면 전체가 비어 있어야
            // 진동을 켤 때 바뀌는 핀 수가 정확히 원의 핀 수가 됩니다(전송 시간 계산의 전제).
            Phase::Running => {
                if self.cue_active {
                    fill_disc(canvas, center, CUE_INTENSITY);
                }
            }
            Phase::Ready | Phase::Done => outline_disc(canvas, center),
        }
        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        let ko = context.language == Language::Ko;
        match self.phase {
            Phase::Ready => SpeechResult::text(if ko {
                "촉각 실험. 가운데 원에 손가락을 올리세요. 원이 떨리다가 멈추는 순간 왼쪽 가운데 키를 누르세요. 시작하려면 가운데 키를 누르세요."
            } else {
                "Tactile experiment. Rest a finger on the circle. Press the left center key the moment the vibration stops. Press center to begin."
            }),
            Phase::Running => SpeechResult::None,
            Phase::Done => {
                let answered = self.responded().count();
                let mean = self.mean_diff_ms().unwrap_or(0.0).round() as i64;
                let mean_abs = self.mean_abs_diff_ms().unwrap_or(0.0).round() as i64;
                SpeechResult::text(if ko {
                    format!(
                        "실험 완료. {TRIAL_COUNT}회 중 {answered}회 응답. 평균 차이 {mean}밀리초, 평균 절대 차이 {mean_abs}밀리초. 결과는 PC에서 받아갈 수 있습니다. 다시 하려면 가운데 키를 누르세요."
                    )
                } else {
                    format!(
                        "Done. {answered} of {TRIAL_COUNT} answered. Mean difference {mean} milliseconds, mean absolute {mean_abs}. Results can be read from a PC. Press center to run again."
                    )
                })
            }
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "촉각 반응 실험입니다. 가운데 원이 떨리기 시작하고 약 0.2초 뒤에 멈춥니다. 멈추는 순간에 왼쪽 가운데 키를 누르세요. 20회 반복하며, 누른 시각과 멈춘 시각의 차이가 기록됩니다. 가운데 키로 시작하고, 메뉴 키로 중단하거나 종료합니다. 결과는 PC에 USB로 연결한 뒤 device-info experiment 명령으로 받아갈 수 있습니다."
                .to_string(),
            en: "Tactile timing experiment. The center circle vibrates and stops about 0.2 seconds later. Press the left center key the moment it stops. Repeated 20 times; the difference between your press and the stop is recorded. Center starts, menu stops or exits. Read the results from a PC over USB with the device-info experiment command."
                .to_string(),
            ja: "触覚反応実験です。中央の円が振動し、約0.2秒後に止まります。止まった瞬間に左の中央キーを押してください。20回繰り返し、押した時刻と止まった時刻の差を記録します。中央キーで開始、メニューキーで中断または終了します。結果はPCからdevice-info experimentコマンドで取得できます。"
                .to_string(),
        }
    }
}

/// WASM 런타임 진입점 함수 정의
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(TactileExperiment::default()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    fn running() -> TactileExperiment {
        let mut e = TactileExperiment::default();
        e.set_display_size(Size::new(64, 24));
        e.start(ms(1_000), 0);
        e
    }

    #[test]
    fn disc_of_22_has_384_pins() {
        assert_eq!(cue_pin_count(), 384);
    }

    #[test]
    fn transmission_follows_the_runtime_rule() {
        // 64x24 = 1536핀 → 전체 프레임 768B. 원(384핀) × 2 = 768 이라 전체 프레임으로 갑니다.
        let full = transmission_ms(384, Size::new(64, 24));
        assert!((full - 768.0 * 1000.0 / 11_520.0).abs() < 1e-9);
        // 바뀐 핀이 적으면 차이 전송(핀당 3B)입니다.
        let diff = transmission_ms(100, Size::new(64, 24));
        assert!((diff - 300.0 * 1000.0 / 11_520.0).abs() < 1e-9);
    }

    #[test]
    fn cue_runs_from_advance_before_t_until_t() {
        let mut e = running();
        let t = e.current.unwrap().target;
        let cue_on = e.current.unwrap().cue_on;
        let advance = (t - cue_on).as_secs_f64() * 1000.0;
        assert!((advance - (CUE_LEAD_MS + e.tx_ms)).abs() < 0.01);

        e.tick(cue_on - ms(1));
        assert!(!e.cue_active);
        e.tick(cue_on);
        assert!(e.cue_active);
        e.tick(t - ms(1));
        assert!(e.cue_active);
        e.tick(t);
        assert!(!e.cue_active, "t 가 되면 꺼져야 합니다");
    }

    #[test]
    fn early_press_keeps_vibrating_until_t() {
        let mut e = running();
        let trial = e.current.unwrap();
        e.tick(trial.cue_on + ms(10));
        assert!(e.press(trial.cue_on + ms(10)));
        assert!(
            e.tick(trial.cue_on + ms(20)).is_none(),
            "t 전에는 회차를 닫지 않습니다"
        );
        assert!(e.cue_active);
        let rec = e.tick(trial.target).unwrap();
        let expected = -((trial.target - trial.cue_on - ms(10)).as_secs_f64() * 1000.0);
        assert!((rec.diff_ms().unwrap() - expected).abs() < 0.01);
    }

    #[test]
    fn press_before_cue_is_ignored() {
        let mut e = running();
        let trial = e.current.unwrap();
        assert!(!e.press(trial.cue_on - ms(1)));
        assert!(e.current.unwrap().press.is_none());
    }

    #[test]
    fn late_press_is_positive_and_no_press_is_a_miss() {
        let mut e = running();
        let t = e.current.unwrap().target;
        e.tick(t + ms(30));
        assert!(e.press(t + ms(30)));
        let rec = e.tick(t + ms(31)).unwrap();
        assert!((rec.diff_ms().unwrap() - 30.0).abs() < 0.01);

        let t = e.current.unwrap().target;
        assert!(e.tick(t + ms(RESPONSE_WINDOW_MS)).is_none());
        let rec = e.tick(t + ms(RESPONSE_WINDOW_MS + 1)).unwrap();
        assert_eq!(rec.press, None);
    }

    #[test]
    fn runs_exactly_twenty_trials_and_exports_csv() {
        let mut e = running();
        let mut done = 0;
        while let Some(trial) = e.current {
            e.tick(trial.cue_on);
            e.press(trial.target + ms(5));
            assert!(e.tick(trial.target + ms(5)).is_some());
            done += 1;
        }
        assert_eq!(done, TRIAL_COUNT);
        assert_eq!(e.phase, Phase::Done);
        assert!(!e.press(ms(999_999)), "완료 후 입력은 기록하지 않습니다");

        let csv = e.to_csv();
        let rows: Vec<&str> = csv.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(rows[0], "trial,target_ms,cue_on_ms,press_ms,diff_ms");
        assert_eq!(rows.len(), TRIAL_COUNT + 1);
        assert!(rows[1].ends_with(",5.0"));
        assert!(csv.len() < 4096, "USB 응답 버퍼에 들어가야 합니다");
        assert!((e.mean_diff_ms().unwrap() - 5.0).abs() < 0.01);
    }

    #[test]
    fn gaps_stay_in_range() {
        let e = running();
        let first = e.current.unwrap();
        let gap = (first.cue_on - ms(1_000)).as_millis() as u64;
        assert!((GAP_MIN_MS..=GAP_MAX_MS).contains(&gap));
    }
}
