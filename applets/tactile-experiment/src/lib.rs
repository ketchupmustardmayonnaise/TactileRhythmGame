//! 촉각 반응 시간 실험 애플릿
//!
//! 화면 중앙의 커다란 원(지름 22)을 진동시켜 "곧 칠 시각"을 예고하고,
//! 사용자가 **왼쪽 가운데 키**(웹 시뮬레이터의 `Space`)를 누른 시각이 목표 시각 `t` 와
//! 얼마나 차이 나는지를 기록합니다. 이를 `TRIAL_COUNT`(20)회 반복합니다.
//!
//! 한 회차의 시간축
//!
//! ```text
//!   t-(250+전송)        t-(125+전송)          t              t+500        t + 응답 허용
//!   ─────┬───────────────────┬─────────────────┬────────────────┬─────────────┬──────
//!        │◀── 1단계 진동 ──▶│◀── 2단계 진동 ─▶│◀─ 홀딩(선택) ─▶│             │
//!        │◀──────────────────── 키 입력 기록 구간 ───────────────────────────────▶│
//! ```
//!
//! * 홀딩을 켜면 `t` 부터 `t + HOLD_MS` 까지 원을 `HOLD_CODES` 중 고른 코드로 유지하고, 키를 누르면 바로 끕니다.
//!   대기 화면에서 왼쪽 메뉴 키(웹 시뮬레이터의 `Q`)를 누를 때마다 끔 → 15 → 1 → 끔 으로 돕니다.
//!
//! * 단계별 시각은 `STAGE_LEADS_MS`, 세기(4비트 코드 0~15) 조합은 `CODE_PRESETS` 표로 정합니다.
//! * 대기 화면에서 ←/→ 로 조합을 고르고 Fn 으로 역순을 고르면, 그 조합이 반복 재생되고 실험에 쓰입니다.
//! * 홀딩이 꺼져 있으면 마지막 단계는 `t` 에 꺼집니다. 그 사이에 키를 눌러도 `t` 까지 유지합니다.
//! * 전송 시간은 각 단계에서 바뀌는 핀 수로 계산합니다(`stage_tx_ms`).
//! * 1단계가 켜지기 전에 누른 것은 느끼고 친 것이 아니므로 무시합니다.
//! * `t + RESPONSE_WINDOW_MS` 까지 누르지 않으면 미응답(miss)으로 기록합니다.
//!
//! 결과는 매 회차마다 `RESULT_KEY` 에 CSV 로 저장되며, PC 에서 USB 로 받아갈 수 있습니다.
//!
//! ```text
//! cargo run -p device-info -- experiment -o result.csv
//! ```

use std::time::Duration;

use sdk::Applet;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult, KeypadSide};
use sdk::applet::{LocalizedString, SpeechOption, SpeechResult};
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

/// 단계별 시작 시각. 단계 `i` 는 `t - (STAGE_LEADS_MS[i] + 전송 시간)` 에 시작합니다.
///
/// 뒤 단계일수록 작아야 하며, 마지막 단계는 `t` 에 꺼집니다.
const STAGE_LEADS_MS: [f64; 2] = [
    220.0, // 1단계: t - (250ms + 전송) 부터 125ms 동안
    110.0, // 2단계: t - (125ms + 전송) 부터 t 까지 125ms 동안
];

const STAGE_COUNT: usize = STAGE_LEADS_MS.len();

/// **진동 코드 조합 표. 여기만 고치면 됩니다.** 각 줄이 `[1단계 코드, 2단계 코드]` 입니다.
///
/// 대기 화면에서 ←/→ 로 고르고, Fn 키로 역순(2단계 코드를 먼저)으로 바꿉니다.
/// 고른 조합이 그대로 실험에 쓰입니다. 줄을 더 넣거나 빼도 됩니다.
///
/// 코드(0~15)는 펌웨어(`firmware/src/braille_display.rs`)가 이렇게 해석합니다.
///
/// | 코드 | 동작 |
/// |---|---|
/// | 0, 8 | 꺼짐 |
/// | 1~6 | 약 62.5Hz 반송파의 듀티 12.5%·25%·37.5%·50%·62.5%·75% |
/// | 7, 15 | 항상 켜짐 (떨리지 않고 올라와만 있음) |
/// | 9~14 | 점멸 1·2·4·8·16·32Hz |
///
/// 9~14 는 전체 프레임 전송일 때만 점멸로 해석됩니다. 지름 22 원(384핀)은
/// 바꿀 때마다 전체 프레임으로 나가므로 그대로 쓸 수 있습니다.
/// 점멸은 펌웨어가 절대 시각 기준으로 위상을 잡으므로, 반주기가 단계 길이(125ms)보다
/// 긴 코드 9~11 은 한 번도 안 바뀌고 통째로 켜지거나 꺼질 수 있습니다.
const CODE_PRESETS: [[u8; STAGE_COUNT]; 12] = [
    [13, 14],
    [13, 15],
    [14, 15],
    [1, 5],
    [1, 6],
    [1, 7],
    [2, 6],
    [2, 7],
    [3, 6],
    [3, 7],
    [4, 6],
    [4, 7],
];

/// 대기 화면에서 조합을 반복해서 보여 줄 때, 한 번 끝나고 다음 진동까지 쉬는 시간(ms)
const DEMO_PAUSE_MS: f64 = 1_000.0;

/// 홀딩 시간(ms). 홀딩을 켜면 `t` 부터 이 시간 동안 원을 고른 홀딩 코드로 유지합니다.
/// 그 사이에 키(왼쪽 가운데 키)를 누르면 바로 꺼집니다.
///
/// 단계와 마찬가지로 전송 시간만큼 미리 보내, 손끝에는 `[t, t + HOLD_MS)` 동안 닿게 합니다.
const HOLD_MS: f64 = 500.0;

/// 홀딩 동안 쓸 코드 후보 (0~15, `CODE_PRESETS` 의 표 참고).
///
/// 대기 화면에서 왼쪽 메뉴 키(웹 시뮬레이터의 `Q`)를 누를 때마다
/// 끔 → 15(항상 켜짐) → 1(듀티 12.5% 진동) → 끔 순서로 돕니다. 줄을 더 넣어도 됩니다.
const HOLD_CODES: [u8; 2] = [15, 2];

/// `cue_stage` 에서 홀딩을 가리키는 번호. 단계 번호(0..STAGE_COUNT) 바로 다음입니다.
const HOLD: usize = STAGE_COUNT;

const _: () = {
    assert!(STAGE_COUNT > 0, "단계가 하나 이상 있어야 합니다");
    assert!(CODE_PRESETS.len() > 0, "조합이 하나 이상 있어야 합니다");
    let mut h = 0;
    while h < HOLD_CODES.len() {
        assert!(HOLD_CODES[h] <= 15, "HOLD_CODES 는 0~15 입니다");
        h += 1;
    }
    let mut i = 1;
    while i < STAGE_COUNT {
        assert!(
            STAGE_LEADS_MS[i] < STAGE_LEADS_MS[i - 1],
            "STAGE_LEADS_MS 는 뒤 단계일수록 작아야 합니다"
        );
        i += 1;
    }
    let mut p = 0;
    while p < CODE_PRESETS.len() {
        let mut j = 0;
        while j < STAGE_COUNT {
            assert!(CODE_PRESETS[p][j] <= 15, "코드는 0~15 입니다");
            j += 1;
        }
        p += 1;
    }
};

/// 4비트 코드를 그 코드로 나가는 `Intensity` 로 바꿉니다.
///
/// 런타임이 `(값*7+127)/255` 로 0~7 을 만들고 `blink` 면 8 을 더하므로
/// (`runtime-native/src/hal/display/mod.rs` 의 `map_intensity_to_4bit`) 그 역함수입니다.
const fn code_to_intensity(code: u8) -> Intensity {
    let level = (code & 7) as u16;
    Intensity {
        value: (level * 255 / 7) as u8,
        blink: code >= 8,
    }
}

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

/// 원의 핀 수
fn cue_pin_count() -> usize {
    disc_offsets(CUE_DIAMETER).count()
}

/// 각 단계를 시작할 때의 전송 시간(ms).
///
/// 원 안의 모든 핀이 같은 코드이므로, 직전 상태(1단계는 꺼짐 = 코드 0)와 코드가 다르면
/// 원 전체가 바뀌고, 같으면 아무것도 보내지 않습니다(0ms).
fn stage_tx_ms(display: Size, codes: [u8; STAGE_COUNT]) -> [f64; STAGE_COUNT] {
    let mut prev_code = 0;
    codes.map(|code| {
        // 런타임은 4비트 코드끼리 비교해 바뀐 핀만 골라냅니다.
        let changed = code != prev_code;
        prev_code = code;
        if changed {
            transmission_ms(cue_pin_count(), display)
        } else {
            0.0
        }
    })
}

/// 홀딩을 켤 때와 끌 때의 전송 시간(ms).
///
/// 켤 때는 마지막 단계 코드에서 `hold_code` 로, 끌 때는 `hold_code` 에서 꺼짐(0)으로 바뀝니다.
/// 코드가 같으면 보낼 것이 없으므로 0ms 입니다.
fn hold_tx_ms(display: Size, last_stage_code: u8, hold_code: u8) -> (f64, f64) {
    let full = transmission_ms(cue_pin_count(), display);
    let on = if hold_code != last_stage_code {
        full
    } else {
        0.0
    };
    let off = if hold_code != 0 { full } else { 0.0 };
    (on, off)
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
    /// 안내(미리보기) 화면. 왼쪽 위 키(웹 시뮬레이터의 W)로 시작합니다.
    Ready,
    Running,
    /// 20회 완료. 왼쪽 위 키(W)로 다시 시작합니다.
    Done,
}

/// 한 회차의 기록. 시각은 모두 실험 시작 기준입니다.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrialRecord {
    /// 목표 시각 `t`
    target: Duration,
    /// 각 단계를 시작한 시각 `t - (lead_ms + 전송)`
    stage_on: [Duration; STAGE_COUNT],
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
    stage_on: [Duration; STAGE_COUNT],
    /// 홀딩을 켜고 끌 시각(전송 시간만큼 앞당긴 값). 홀딩이 꺼져 있으면 `None`.
    hold: Option<(Duration, Duration)>,
    press: Option<Duration>,
}

impl PendingTrial {
    /// 진동이 처음 켜지는 시각(1단계)
    fn cue_on(&self) -> Duration {
        self.stage_on[0]
    }

    /// `now` 에 켜져 있어야 할 단계(`HOLD` = 홀딩). 꺼져 있어야 하면 `None`.
    fn stage_at(&self, now: Duration) -> Option<usize> {
        if let Some((hold_on, hold_off)) = self.hold
            && now >= hold_on
        {
            match self.press {
                // 아직 안 눌렀으면 홀딩 시간이 끝날 때까지 유지합니다.
                None => return (now < hold_off).then_some(HOLD),
                // 홀딩 중에 눌렀으면 바로 끕니다.
                Some(p) if p >= hold_on => return None,
                // t 전에 이미 눌렀으면 홀딩 없이 평소처럼 t 까지만 단계를 유지합니다.
                Some(_) => {}
            }
        }
        if now >= self.target {
            return None;
        }
        // 시작 시각이 지난 단계 중 가장 뒤의 것
        self.stage_on.iter().rposition(|&on| now >= on)
    }

    /// 다음 회차로 넘어가도 되는 시각. 진동이 `t` 에 꺼진 뒤여야 합니다.
    fn can_close(&self, now: Duration) -> bool {
        let deadline = self.target + Duration::from_millis(RESPONSE_WINDOW_MS);
        (self.press.is_some() && now >= self.target) || now > deadline
    }
}

pub struct TactileExperiment {
    phase: Phase,
    rng: Rng,
    /// 단계별 전송 시간(ms)
    tx_ms: [f64; STAGE_COUNT],
    /// 실험을 시작한 모노토닉 시각. 모든 기록의 기준점입니다.
    started_at: Duration,
    /// 실험 시작 시각(유닉스 초). CSV 머리말용.
    started_unix: u64,
    current: Option<PendingTrial>,
    records: Vec<TrialRecord>,
    /// 지금 켜져 있는 단계 (on_draw 에서 씁니다)
    cue_stage: Option<usize>,
    /// 화면 크기 (조합이 바뀌면 전송 시간을 다시 계산하는 데 씁니다)
    display: Size,
    /// `CODE_PRESETS` 에서 고른 줄
    preset: usize,
    /// 역순(2단계 코드를 1단계에) 여부
    reversed: bool,
    /// 대기 화면에서 고른 조합을 반복 재생 중인지, 그렇다면 언제부터인지
    demo_since: Option<Duration>,
    /// 미리보기 중 마지막으로 Space(왼쪽 가운데 키)를 누른 시각. 홀딩을 끄는 연습용입니다.
    demo_press: Option<Duration>,
    /// 홀딩 상태. 0 = 끔, `i` (1 이상) = `HOLD_CODES[i - 1]` 로 홀딩. 왼쪽 메뉴 키로 돕니다.
    hold_mode: usize,
    /// 홀딩을 켜고 끌 때의 전송 시간(ms)
    hold_tx: (f64, f64),
}

impl Default for TactileExperiment {
    fn default() -> Self {
        Self {
            phase: Phase::Ready,
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
            tx_ms: stage_tx_ms(Size::new(0, 0), CODE_PRESETS[0]),
            started_at: Duration::ZERO,
            started_unix: 0,
            current: None,
            records: Vec::with_capacity(TRIAL_COUNT),
            cue_stage: None,
            display: Size::new(0, 0),
            preset: 0,
            reversed: false,
            demo_since: None,
            demo_press: None,
            hold_mode: 0,
            hold_tx: (0.0, 0.0),
        }
    }
}

impl TactileExperiment {
    /// 화면 크기에 맞춰 전송 시간을 다시 계산합니다.
    fn set_display_size(&mut self, size: Size) {
        self.display = size;
        self.recompute_tx();
    }

    /// 화면 크기나 조합이 바뀌면 단계·홀딩의 전송 시간을 다시 계산합니다.
    fn recompute_tx(&mut self) {
        let codes = self.codes();
        self.tx_ms = stage_tx_ms(self.display, codes);
        self.hold_tx = match self.hold_code() {
            Some(hold) => hold_tx_ms(self.display, codes[STAGE_COUNT - 1], hold),
            None => (0.0, 0.0),
        };
    }

    /// 지금 고른 홀딩 코드. 홀딩이 꺼져 있으면 `None`.
    fn hold_code(&self) -> Option<u8> {
        self.hold_mode.checked_sub(1).map(|i| HOLD_CODES[i])
    }

    /// `cue_stage` 번호(단계 또는 `HOLD`)에 해당하는 코드
    fn cue_code(&self, stage: usize) -> u8 {
        if stage == HOLD {
            self.hold_code().unwrap_or(0)
        } else {
            self.codes()[stage]
        }
    }

    /// 지금 고른 조합의 단계별 코드
    fn codes(&self) -> [u8; STAGE_COUNT] {
        let mut codes = CODE_PRESETS[self.preset];
        if self.reversed {
            codes.reverse();
        }
        codes
    }

    /// 단계 `i` 를 `t` 보다 얼마나 앞서 시작하는지(ms) = lead_ms + 전송 시간
    fn stage_advance_ms(&self, i: usize) -> f64 {
        STAGE_LEADS_MS[i] + self.tx_ms[i]
    }

    /// 단계 구성을 한 줄로 (로그·CSV 머리말용)
    fn stages_summary(&self) -> String {
        (0..STAGE_COUNT)
            .map(|i| {
                format!(
                    "{}:lead{}+tx{:.1}=t-{:.1}ms:code{}",
                    i + 1,
                    STAGE_LEADS_MS[i],
                    self.tx_ms[i],
                    self.stage_advance_ms(i),
                    self.codes()[i]
                )
            })
            .chain(std::iter::once(match self.hold_code() {
                Some(hold) => format!(
                    "hold:t-tx{:.1}..t+{}-tx{:.1}:code{hold}",
                    self.hold_tx.0, HOLD_MS, self.hold_tx.1
                ),
                None => "hold:off".to_string(),
            }))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn start(&mut self, now: Duration, unix_secs: u64) {
        self.phase = Phase::Running;
        self.started_at = now;
        self.started_unix = unix_secs;
        self.records.clear();
        self.current = None;
        self.cue_stage = None;
        self.demo_since = None;
        self.schedule_next(now);
        log::info!(
            "실험 시작: {TRIAL_COUNT}회, 원 지름 {CUE_DIAMETER}(핀 {}개), 단계 {}",
            cue_pin_count(),
            self.stages_summary()
        );
    }

    /// 지금부터 무작위 간격 뒤에 1단계가 켜지도록 다음 회차를 잡습니다.
    fn schedule_next(&mut self, now: Duration) {
        let gap = Duration::from_millis(self.rng.range(GAP_MIN_MS, GAP_MAX_MS));
        let target = now + gap + ms_to_duration(self.stage_advance_ms(0));
        self.current = Some(self.make_trial(target));
    }

    /// 목표 시각 `t` 에 맞춰 단계·홀딩 시각을 잡은 회차를 만듭니다.
    fn make_trial(&self, target: Duration) -> PendingTrial {
        let stage_on = std::array::from_fn(|i| {
            target.saturating_sub(ms_to_duration(self.stage_advance_ms(i)))
        });
        let hold = self.hold_code().map(|_| {
            let (on_tx, off_tx) = self.hold_tx;
            (
                target.saturating_sub(ms_to_duration(on_tx)),
                target + ms_to_duration(HOLD_MS - off_tx),
            )
        });
        PendingTrial {
            target,
            stage_on,
            hold,
            press: None,
        }
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
        if now < trial.cue_on() {
            log::info!(
                "진동 전 입력 무시 (진동까지 {:.0}ms 남음)",
                (trial.cue_on() - now).as_secs_f64() * 1000.0
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
            self.cue_stage = self.demo_stage(now);
            return None;
        }
        let trial = self.current?;

        // 진동 구간은 정확히 [1단계 시작, t) 입니다.
        let stage = trial.stage_at(now);
        if stage != self.cue_stage
            && let Some(i) = stage
        {
            let to_t = (trial.target.as_secs_f64() - now.as_secs_f64()) * 1000.0;
            if i == HOLD {
                log::debug!("홀딩 시작 (t 까지 {to_t:.1}ms)");
            } else {
                log::debug!("{}단계 시작 (t 까지 {to_t:.1}ms)", i + 1);
            }
        }
        self.cue_stage = stage;

        if !trial.can_close(now) {
            return None;
        }

        let record = TrialRecord {
            target: trial.target - self.started_at,
            stage_on: trial.stage_on.map(|on| on - self.started_at),
            press: trial.press.map(|p| p - self.started_at),
        };
        self.records.push(record);
        self.cue_stage = None;

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

    /// ←/→: 조합을 `delta` 만큼 옮기고 바로 반복 재생합니다. 음성 안내 문구를 돌려줍니다.
    fn select_preset(&mut self, delta: isize, now: Duration, lang: Language) -> String {
        let n = CODE_PRESETS.len() as isize;
        self.preset = (self.preset as isize + delta).rem_euclid(n) as usize;
        self.apply_codes(now);
        self.preset_label(lang)
    }

    /// Fn: 역순을 켜고 끕니다. 음성 안내 문구를 돌려줍니다.
    fn toggle_reversed(&mut self, now: Duration, lang: Language) -> String {
        self.reversed = !self.reversed;
        self.apply_codes(now);
        self.preset_label(lang)
    }

    /// 왼쪽 메뉴 키(웹 시뮬레이터의 Q): 홀딩을 끔 → `HOLD_CODES` 차례 → 끔 으로 돌립니다.
    /// 음성 안내 문구를 돌려줍니다.
    fn toggle_holding(&mut self, now: Duration, lang: Language) -> String {
        self.hold_mode = (self.hold_mode + 1) % (HOLD_CODES.len() + 1);
        self.apply_codes(now);
        let secs = HOLD_MS / 1000.0;
        match (self.hold_code(), lang == Language::Ko) {
            (Some(code), true) => format!("홀딩 {code}, {secs}초"),
            (None, true) => "홀딩 끔".to_string(),
            (Some(code), false) => format!("Holding {code}, {secs} seconds"),
            (None, false) => "Holding off".to_string(),
        }
    }

    /// 조합이 바뀌면 전송 시간이 달라지므로 다시 계산하고, 반복 재생을 처음부터 시작합니다.
    fn apply_codes(&mut self, now: Duration) {
        self.recompute_tx();
        self.demo_since = Some(now);
        log::info!(
            "조합 {}번{}: {}",
            self.preset + 1,
            if self.reversed { " 역순" } else { "" },
            self.stages_summary()
        );
    }

    /// 예: "3번, 2 다음 4" / "3번 역순, 4 다음 2"
    fn preset_label(&self, lang: Language) -> String {
        let codes = self.codes().map(|c| c.to_string());
        let n = self.preset + 1;
        if lang == Language::Ko {
            let rev = if self.reversed { " 역순" } else { "" };
            format!("{n}번{rev}, {}", codes.join(" 다음 "))
        } else {
            let rev = if self.reversed { " reversed" } else { "" };
            format!("Preset {n}{rev}, {}", codes.join(" then "))
        }
    }

    /// 대기 화면에서 지금 켜 둘 단계.
    ///
    /// 조합이나 홀딩을 바꾼 뒤로는 `DEMO_PAUSE_MS` 쉬고 → 실험과 똑같은 시간 배치로 단계를 켜고 →
    /// (홀딩이 켜져 있으면 `t + HOLD_MS` 까지 홀딩하고) → 끄기를 반복합니다.
    /// 한 번도 바꾸지 않았으면 진동하지 않습니다.
    ///
    /// 이번 주기의 1단계가 켜진 뒤에 Space 를 눌렀으면 실험과 똑같이 반영합니다(홀딩이 바로 꺼짐).
    fn demo_stage(&self, now: Duration) -> Option<usize> {
        let since = self.demo_since?;
        let first = self.stage_advance_ms(0);
        let tail = if self.hold_code().is_some() {
            HOLD_MS
        } else {
            0.0
        };
        let cycle = DEMO_PAUSE_MS + first + tail;
        let elapsed = now.saturating_sub(since).as_secs_f64() * 1000.0;
        let cycle_start = (elapsed / cycle).floor() * cycle;
        let target = since + ms_to_duration(cycle_start + DEMO_PAUSE_MS + first);
        let mut trial = self.make_trial(target);
        // 이전 주기나 쉬는 동안 누른 것은 실험처럼 무시합니다.
        trial.press = self.demo_press.filter(|&p| p >= trial.cue_on());
        trial.stage_at(now)
    }

    fn abort(&mut self) {
        self.phase = Phase::Ready;
        self.current = None;
        self.cue_stage = None;
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
        let stage_cols: String = (1..=STAGE_COUNT)
            .map(|i| format!("stage{i}_on_ms,"))
            .collect();
        let mut out = format!(
            "# tactile-experiment started_unix={} trials={}/{} stages={} \
             diameter={} pins={} window_ms={}\n\
             trial,target_ms,{stage_cols}press_ms,diff_ms\n",
            self.started_unix,
            self.records.len(),
            TRIAL_COUNT,
            self.stages_summary(),
            CUE_DIAMETER,
            cue_pin_count(),
            RESPONSE_WINDOW_MS,
        );
        for (i, r) in self.records.iter().enumerate() {
            let stages: String = r.stage_on.iter().map(|&on| ms(on) + ",").collect();
            out.push_str(&format!(
                "{},{},{stages}{},{}\n",
                i + 1,
                ms(r.target),
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

/// ms(실수)를 `Duration` 으로. 음수는 0 으로 둡니다.
fn ms_to_duration(ms: f64) -> Duration {
    Duration::from_secs_f64(ms.max(0.0) / 1000.0)
}

/// 나오고 있는 안내 음성을 끊습니다.
///
/// SDK 에는 음성 정지 API 가 없지만, audio-service 는 일반(중요 아님) 음성 요청이 들어오면
/// 그 트랙(`TtsNormal`)을 비우고 새로 재생합니다(`audio-service/src/main.rs` 의 시퀀스 핸들러).
/// 그래서 1ms 무음을 일반 음성으로 보내면 진행 중인 안내가 끊깁니다.
/// 안내 음성이 아직 합성 중이면 서비스의 디바운스 때문에 최대 300ms 늦게 끊길 수 있습니다.
fn stop_speech(context: &mut Context) {
    context
        .audio
        .play_audio_sequence(&[AudioSegment::Silence(1)], SpeechOption::new());
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
            "tactile-experiment 시작: 화면 {size:?}, 원 핀 {}개, 단계 {}",
            cue_pin_count(),
            self.stages_summary()
        );
        Ok(())
    }

    fn target_fps(&self) -> u16 {
        TARGET_FPS
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let now = context.time.get_monotonic_time();
        let was_stage = self.cue_stage;
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
                // 미리보기에서 Space: 실험처럼 눌러 보기 (홀딩이 켜져 있으면 바로 꺼짐)
                (Phase::Ready | Phase::Done, KeyCode::Center, KeypadSide::Left) => {
                    self.demo_press = Some(now);
                }
                // 실전 시작: 왼쪽 위 키 (웹 시뮬레이터의 W)
                (Phase::Ready | Phase::Done, KeyCode::Up, KeypadSide::Left) => {
                    stop_speech(context);
                    self.start(now, context.time.get_time_seconds());
                    self.save(context);
                    needs_redraw = true;
                }
                // 홀딩 전환: 왼쪽 메뉴 키 (웹 시뮬레이터의 Q)
                (Phase::Ready | Phase::Done, KeyCode::Menu, KeypadSide::Left) => {
                    let label = self.toggle_holding(now, context.language);
                    context
                        .audio
                        .speak_text_with_option(&label, SpeechOption::forced());
                    needs_redraw = true;
                }
                // 종료: 오른쪽 메뉴 키 (웹 시뮬레이터의 Delete)
                (Phase::Ready | Phase::Done, KeyCode::Menu, _) => {
                    return Ok(UpdateResult::ExitApp);
                }
                // 조합 고르기: ←/→ (양쪽 키패드)
                (Phase::Ready | Phase::Done, KeyCode::Left | KeyCode::Right, _) => {
                    let delta = if event.code == KeyCode::Right { 1 } else { -1 };
                    let label = self.select_preset(delta, now, context.language);
                    context
                        .audio
                        .speak_text_with_option(&label, SpeechOption::forced());
                    needs_redraw = true;
                }
                // 역순 전환: Fn
                (Phase::Ready | Phase::Done, KeyCode::Function, _) => {
                    let label = self.toggle_reversed(now, context.language);
                    context
                        .audio
                        .speak_text_with_option(&label, SpeechOption::forced());
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        if self.tick(now).is_some() {
            self.save(context);
            needs_redraw = true;
        }
        if self.cue_stage != was_stage {
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
            // 단계가 바뀔 때 바뀌는 핀 수가 정확히 원의 핀 수가 됩니다(전송 시간 계산의 전제).
            Phase::Running => {
                if let Some(i) = self.cue_stage {
                    fill_disc(canvas, center, code_to_intensity(self.cue_code(i)));
                }
            }
            // 대기 화면: 조합을 고르기 전에는 손가락 놓을 자리(테두리)를,
            // 고른 뒤에는 실험과 똑같이 빈 화면에서 원만 켜고 끕니다(전송 시간이 같도록).
            Phase::Ready | Phase::Done => match (self.cue_stage, self.demo_since) {
                (Some(i), _) => fill_disc(canvas, center, code_to_intensity(self.cue_code(i))),
                (None, None) => outline_disc(canvas, center),
                (None, Some(_)) => {}
            },
        }
        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        let ko = context.language == Language::Ko;
        match self.phase {
            Phase::Ready => SpeechResult::text(if ko {
                "촉각 실험. 가운데 원에 손가락을 올리세요. 원이 떨리다가 멈추는 순간 왼쪽 가운데 키를 누르세요. 좌우 키로 진동 조합을 고르면 반복해서 느껴 볼 수 있고, 기능 키로 순서를 뒤집습니다. 왼쪽 메뉴 키를 누를 때마다 홀딩이 끔, 15, 1 순서로 바뀝니다. 미리보기 중에도 왼쪽 가운데 키로 눌러 볼 수 있습니다. 실험을 시작하려면 왼쪽 위 키, 종료는 오른쪽 메뉴 키입니다."
            } else {
                "Tactile experiment. Rest a finger on the circle. Press the left center key the moment the vibration stops. Left and right pick a vibration preset and repeat it; the function key reverses it. The left menu key cycles holding: off, 15, 1. You can practice pressing the left center key during the preview. Press left up to begin the experiment, right menu to exit."
            }),
            Phase::Running => SpeechResult::None,
            Phase::Done => {
                let answered = self.responded().count();
                let mean = self.mean_diff_ms().unwrap_or(0.0).round() as i64;
                let mean_abs = self.mean_abs_diff_ms().unwrap_or(0.0).round() as i64;
                SpeechResult::text(if ko {
                    format!(
                        "실험 완료. {TRIAL_COUNT}회 중 {answered}회 응답. 평균 차이 {mean}밀리초, 평균 절대 차이 {mean_abs}밀리초. 결과는 PC에서 받아갈 수 있습니다. 다시 하려면 왼쪽 위 키를 누르세요."
                    )
                } else {
                    format!(
                        "Done. {answered} of {TRIAL_COUNT} answered. Mean difference {mean} milliseconds, mean absolute {mean_abs}. Results can be read from a PC. Press left up to run again."
                    )
                })
            }
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "촉각 반응 실험입니다. 가운데 원이 떨리기 시작하고 약 0.25초 뒤에 멈춥니다. 멈추는 순간에 왼쪽 가운데 키를 누르세요. 20회 반복하며, 누른 시각과 멈춘 시각의 차이가 기록됩니다. 대기 화면에서 좌우 키로 1단계와 2단계 진동 조합을 고르면 실험과 같은 모양으로 반복해서 느껴 볼 수 있고, 기능 키로 순서를 뒤집을 수 있습니다. 고른 조합이 실험에 쓰입니다. 왼쪽 메뉴 키를 누를 때마다 홀딩이 끔, 15(완전히 켜짐), 1(약한 진동) 순서로 바뀝니다. 홀딩이 켜져 있으면 멈춰야 할 순간부터 0.5초 동안 원이 그 코드로 유지되다가 키를 누르면 바로 꺼집니다. 미리보기 중에도 왼쪽 가운데 키로 눌러 볼 수 있습니다. 왼쪽 위 키로 실험을 시작하고, 실험 중에는 메뉴 키로 중단합니다. 대기 화면에서는 오른쪽 메뉴 키로 종료합니다. 결과는 PC에 USB로 연결한 뒤 device-info experiment 명령으로 받아갈 수 있습니다."
                .to_string(),
            en: "Tactile timing experiment. The center circle vibrates and stops about 0.25 seconds later. Press the left center key the moment it stops. Repeated 20 times; the difference between your press and the stop is recorded. On the waiting screen, left and right pick a two-stage vibration preset that then repeats exactly as in the experiment; the function key reverses its order. The chosen preset is used for the experiment. The left menu key cycles holding through off, code 15 (fully raised) and code 1 (light vibration): from the moment to press, the circle holds that code for 0.5 seconds and turns off as soon as you press. You can practice with the left center key during the preview. Left up starts the experiment; menu stops a run; on the waiting screen the right menu key exits. Read the results from a PC over USB with the device-info experiment command."
                .to_string(),
            ja: "触覚反応実験です。中央の円が振動し、約0.25秒後に止まります。止まった瞬間に左の中央キーを押してください。20回繰り返し、押した時刻と止まった時刻の差を記録します。左メニューキーでホールド(押すべき瞬間から0.5秒間保持、押すと即停止)を切り替えます。プレビュー中も左中央キーで押す練習ができます。左上キーで実験開始、実験中はメニューキーで中断、待機画面では右メニューキーで終了します。結果はPCからdevice-info experimentコマンドで取得できます。"
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

    /// 전체 프레임이 768B 인 화면 (원 384핀 × 2 = 768 → 전체 프레임 경계)
    const DISPLAY: Size = Size {
        width: 64,
        height: 24,
    };

    fn running() -> TactileExperiment {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        e.start(ms(1_000), 0);
        e
    }

    fn to_ms(d: Duration) -> f64 {
        d.as_secs_f64() * 1000.0
    }

    #[test]
    fn disc_of_22_has_384_pins() {
        assert_eq!(cue_pin_count(), 384);
    }

    #[test]
    fn transmission_follows_the_runtime_rule() {
        // 64x24 = 1536핀 → 전체 프레임 768B. 원(384핀) × 2 = 768 이라 전체 프레임으로 갑니다.
        let full = transmission_ms(384, DISPLAY);
        assert!((full - 768.0 * 1000.0 / 11_520.0).abs() < 1e-9);
        // 바뀐 핀이 적으면 차이 전송(핀당 3B)입니다.
        let diff = transmission_ms(100, DISPLAY);
        assert!((diff - 300.0 * 1000.0 / 11_520.0).abs() < 1e-9);
    }

    /// 코드 → Intensity → 런타임 4비트 변환이 원래 코드로 돌아와야 합니다.
    #[test]
    fn every_code_round_trips_through_the_runtime_mapping() {
        for code in 0..=15u8 {
            let i = code_to_intensity(code);
            let level = ((i.value as u16 * 7 + 127) / 255) as u8;
            let back = if i.blink { 8 + level } else { level };
            assert_eq!(back, code, "코드 {code}");
        }
    }

    #[test]
    fn each_stage_starts_at_lead_plus_its_transmission_time() {
        let e = running();
        let trial = e.current.unwrap();
        for (i, lead) in STAGE_LEADS_MS.iter().enumerate() {
            let advance = to_ms(trial.target - trial.stage_on[i]);
            assert!(
                (advance - (lead + e.tx_ms[i])).abs() < 0.01,
                "{}단계: t - {advance}ms",
                i + 1
            );
        }
    }

    #[test]
    fn unchanged_code_costs_no_transmission() {
        let tx = stage_tx_ms(DISPLAY, [5, 5]);
        assert!(tx[0] > 0.0, "꺼짐 → 5 는 원 전체가 바뀝니다");
        assert_eq!(tx[1], 0.0, "5 → 5 는 보낼 것이 없습니다");
    }

    #[test]
    fn stages_switch_in_order_and_stop_at_t() {
        let mut e = running();
        let trial = e.current.unwrap();

        e.tick(trial.stage_on[0] - ms(1));
        assert_eq!(e.cue_stage, None);
        for i in 0..STAGE_COUNT {
            e.tick(trial.stage_on[i]);
            assert_eq!(e.cue_stage, Some(i), "{}단계 시작", i + 1);
        }
        e.tick(trial.target - ms(1));
        assert_eq!(e.cue_stage, Some(STAGE_COUNT - 1));
        e.tick(trial.target);
        assert_eq!(e.cue_stage, None, "t 가 되면 꺼져야 합니다");
    }

    #[test]
    fn early_press_keeps_vibrating_until_t() {
        let mut e = running();
        let trial = e.current.unwrap();
        let press_at = trial.cue_on() + ms(10);
        e.tick(press_at);
        assert!(e.press(press_at));
        assert!(
            e.tick(press_at + ms(10)).is_none(),
            "t 전에는 회차를 닫지 않습니다"
        );
        assert!(e.cue_stage.is_some());
        let rec = e.tick(trial.target).unwrap();
        let expected = -to_ms(trial.target - press_at);
        assert!((rec.diff_ms().unwrap() - expected).abs() < 0.01);
    }

    #[test]
    fn press_before_cue_is_ignored() {
        let mut e = running();
        let trial = e.current.unwrap();
        assert!(!e.press(trial.cue_on() - ms(1)));
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
            e.tick(trial.cue_on());
            e.press(trial.target + ms(5));
            assert!(e.tick(trial.target + ms(5)).is_some());
            done += 1;
        }
        assert_eq!(done, TRIAL_COUNT);
        assert_eq!(e.phase, Phase::Done);
        assert!(!e.press(ms(999_999)), "완료 후 입력은 기록하지 않습니다");

        let csv = e.to_csv();
        let rows: Vec<&str> = csv.lines().filter(|l| !l.starts_with('#')).collect();
        let stage_cols: String = (1..=STAGE_COUNT)
            .map(|i| format!("stage{i}_on_ms,"))
            .collect();
        assert_eq!(
            rows[0],
            format!("trial,target_ms,{stage_cols}press_ms,diff_ms")
        );
        assert_eq!(rows.len(), TRIAL_COUNT + 1);
        for row in &rows {
            assert_eq!(row.split(',').count(), 4 + STAGE_COUNT);
        }
        assert!(rows[1].ends_with(",5.0"));
        assert!(csv.len() < 4096, "USB 응답 버퍼에 들어가야 합니다");
        assert!((e.mean_diff_ms().unwrap() - 5.0).abs() < 0.01);
    }

    #[test]
    fn left_right_cycle_presets_and_fn_reverses() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        assert_eq!(e.codes(), CODE_PRESETS[0]);

        e.select_preset(1, ms(0), Language::Ko);
        assert_eq!(e.codes(), CODE_PRESETS[1]);
        e.select_preset(-2, ms(0), Language::Ko);
        assert_eq!(
            e.codes(),
            CODE_PRESETS[CODE_PRESETS.len() - 1],
            "← 는 거꾸로 돕니다"
        );

        let label = e.toggle_reversed(ms(0), Language::Ko);
        let mut rev = CODE_PRESETS[CODE_PRESETS.len() - 1];
        rev.reverse();
        assert_eq!(e.codes(), rev);
        assert_eq!(
            label,
            format!("{}번 역순, {} 다음 {}", CODE_PRESETS.len(), rev[0], rev[1])
        );

        // 고른 조합이 그대로 실험에 쓰입니다.
        e.start(ms(1_000), 0);
        assert_eq!(e.codes(), rev);
        assert!(e.to_csv().contains(&format!("code{}", rev[0])));
    }

    #[test]
    fn demo_repeats_the_experiment_timing() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        e.tick(ms(0));
        assert_eq!(e.cue_stage, None, "고르기 전에는 진동하지 않습니다");

        let since = ms(10_000);
        e.select_preset(0, since, Language::Ko);
        let first = e.stage_advance_ms(0);
        let at = |offset_ms: f64| since + Duration::from_secs_f64(offset_ms / 1000.0);

        for cycle in 0..3 {
            let base = cycle as f64 * (DEMO_PAUSE_MS + first);
            e.tick(at(base + DEMO_PAUSE_MS - 1.0));
            assert_eq!(e.cue_stage, None, "쉬는 중");
            for i in 0..STAGE_COUNT {
                e.tick(at(
                    base + DEMO_PAUSE_MS + first - e.stage_advance_ms(i) + 0.5
                ));
                assert_eq!(e.cue_stage, Some(i), "{}주기 {}단계", cycle + 1, i + 1);
            }
            e.tick(at(base + DEMO_PAUSE_MS + first - 0.5));
            assert_eq!(e.cue_stage, Some(STAGE_COUNT - 1));
        }

        e.start(ms(100_000), 0);
        assert_eq!(e.demo_since, None, "실험을 시작하면 반복 재생은 멈춥니다");
    }

    #[test]
    fn changing_preset_recomputes_transmission() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        for p in 0..CODE_PRESETS.len() {
            e.preset = p;
            e.apply_codes(ms(0));
            assert_eq!(e.tx_ms, stage_tx_ms(DISPLAY, e.codes()));
        }
    }

    fn running_with_hold() -> TactileExperiment {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        e.toggle_holding(ms(0), Language::Ko);
        assert_eq!(e.hold_code(), Some(HOLD_CODES[0]));
        e.start(ms(1_000), 0);
        e
    }

    #[test]
    fn leads_are_250_and_125() {
        assert_eq!(STAGE_LEADS_MS, [250.0, 125.0]);
    }

    #[test]
    fn hold_runs_from_t_until_t_plus_500_without_a_press() {
        let mut e = running_with_hold();
        let trial = e.current.unwrap();
        let (on, off) = trial.hold.unwrap();
        // 전송 시간만큼 앞당겨 보내서, 손끝에는 [t, t + 500) 동안 닿습니다.
        assert!((to_ms(trial.target - on) - e.hold_tx.0).abs() < 0.01);
        assert!((to_ms(off - trial.target) - (HOLD_MS - e.hold_tx.1)).abs() < 0.01);

        e.tick(on - ms(1));
        assert_eq!(e.cue_stage, Some(STAGE_COUNT - 1));
        e.tick(on);
        assert_eq!(e.cue_stage, Some(HOLD));
        assert_eq!(e.cue_code(HOLD), HOLD_CODES[0]);
        e.tick(off - ms(1));
        assert_eq!(e.cue_stage, Some(HOLD));
        e.tick(off);
        assert_eq!(e.cue_stage, None, "홀딩 시간이 끝나면 꺼집니다");
        // 아직 응답 허용 시간 안이라 누르면 기록됩니다.
        assert!(e.press(off + ms(10)));
    }

    #[test]
    fn pressing_during_hold_turns_it_off() {
        let mut e = running_with_hold();
        let trial = e.current.unwrap();
        let press_at = trial.target + ms(120);
        e.tick(press_at);
        assert_eq!(e.cue_stage, Some(HOLD));
        assert!(e.press(press_at));
        let rec = e.tick(press_at).unwrap();
        assert!((rec.diff_ms().unwrap() - 120.0).abs() < 0.01);
        assert_eq!(e.cue_stage, None, "누르면 바로 꺼집니다");
    }

    #[test]
    fn pressing_before_t_skips_the_hold() {
        let mut e = running_with_hold();
        let trial = e.current.unwrap();
        let (on, _) = trial.hold.unwrap();
        let press_at = trial.cue_on() + ms(20);
        e.tick(press_at);
        assert!(e.press(press_at));
        e.tick(on + ms(1));
        assert_eq!(
            e.cue_stage,
            Some(STAGE_COUNT - 1),
            "t 까지는 단계를 유지합니다"
        );
        e.tick(trial.target);
        assert_eq!(e.cue_stage, None, "홀딩 없이 t 에 꺼집니다");
    }

    #[test]
    fn hold_off_means_no_hold() {
        let e = running();
        assert!(e.current.unwrap().hold.is_none());
        assert!(e.to_csv().contains("hold:off"));
        let e = running_with_hold();
        assert!(e.to_csv().contains(&format!("code{}", HOLD_CODES[0])));
    }

    #[test]
    fn demo_includes_the_hold_when_enabled() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        let since = ms(10_000);
        e.toggle_holding(since, Language::Ko);
        let first = e.stage_advance_ms(0);
        let cycle = DEMO_PAUSE_MS + first + HOLD_MS;
        let at = |offset_ms: f64| since + Duration::from_secs_f64(offset_ms / 1000.0);

        for c in 0..3 {
            let t = c as f64 * cycle + DEMO_PAUSE_MS + first;
            e.tick(at(t + 1.0));
            assert_eq!(e.cue_stage, Some(HOLD), "{}주기 홀딩", c + 1);
            e.tick(at(t + HOLD_MS - e.hold_tx.1 + 1.0));
            assert_eq!(e.cue_stage, None, "{}주기 홀딩 끝", c + 1);
        }
    }

    #[test]
    fn q_cycles_off_15_1_off() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        assert_eq!(e.hold_code(), None);
        assert_eq!(e.toggle_holding(ms(0), Language::Ko), "홀딩 15, 0.5초");
        assert_eq!(e.hold_code(), Some(15));
        assert_eq!(e.toggle_holding(ms(0), Language::Ko), "홀딩 1, 0.5초");
        assert_eq!(e.hold_code(), Some(1));
        assert_eq!(e.cue_code(HOLD), 1);
        assert!(e.stages_summary().ends_with("code1"));
        // 홀딩 코드가 바뀌어도 시간 배치는 같습니다(둘 다 2단계 코드와 달라 전송 시간이 같음).
        e.start(ms(1_000), 0);
        let trial = e.current.unwrap();
        e.tick(trial.target);
        assert_eq!(e.cue_stage, Some(HOLD));
        e.abort();
        assert_eq!(e.toggle_holding(ms(0), Language::Ko), "홀딩 끔");
        assert_eq!(e.hold_code(), None);
        assert_eq!(e.hold_tx, (0.0, 0.0));
    }

    #[test]
    fn pressing_space_in_the_demo_cuts_the_hold() {
        let mut e = TactileExperiment::default();
        e.set_display_size(DISPLAY);
        let since = ms(10_000);
        e.toggle_holding(since, Language::Ko);
        let first = e.stage_advance_ms(0);
        let cycle = DEMO_PAUSE_MS + first + HOLD_MS;
        let at = |offset_ms: f64| since + Duration::from_secs_f64(offset_ms / 1000.0);
        let t = DEMO_PAUSE_MS + first;

        // 쉬는 동안 누른 것은 무시합니다.
        e.demo_press = Some(at(10.0));
        e.tick(at(t + 50.0));
        assert_eq!(e.cue_stage, Some(HOLD));

        // 홀딩 중에 누르면 바로 꺼집니다.
        e.demo_press = Some(at(t + 100.0));
        e.tick(at(t + 101.0));
        assert_eq!(e.cue_stage, None);

        // 다음 주기에는 다시 홀딩합니다.
        e.tick(at(cycle + t + 50.0));
        assert_eq!(e.cue_stage, Some(HOLD));
    }

    #[test]
    fn gaps_stay_in_range() {
        let e = running();
        let first = e.current.unwrap();
        let gap = (first.cue_on() - ms(1_000)).as_millis() as u64;
        assert!((GAP_MIN_MS..=GAP_MAX_MS).contains(&gap));
    }
}
