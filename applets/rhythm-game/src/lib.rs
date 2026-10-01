//! 리듬 게임 애플릿
//!
//! 화면 중앙에 원이 하나 있고, 노트 시각이 다가올수록 이 원의 진동이 점점 강해집니다.
//! 기본값은 기기 부팅/종료 애니메이션과 같은 PWM 강도 램프(`VibrationMode::DutyRatio`)입니다.
//! 진동이 최고조에 달하는 순간(= 노트 시각)에 가운데 키(웹 시뮬레이터의 `Space`)를 누르면 됩니다.
//!
//! 흐름
//! 1. 곡 선택 화면 — 좌/우 키로 곡을 고르면 곡 이름을 음성으로 읽어 줍니다.
//! 2. 가운데 키(`Enter`)를 누르면 음악이 재생되면서 게임이 시작됩니다.
//! 3. 노트마다 `예고 시간` 동안 원이 진동하며, 그 끝에서 가운데 키(`Space`)를 누릅니다.
//! 4. 기능 키(`E`)를 누르면 진동 방식 두 가지가 번갈아 전환되고, 바뀐 방식을 음성으로 알려 줍니다.
//!
//! 채보는 `src/audio/*.json` 에서 읽습니다. `meta.seconds_per_beat` 이 예고 시간의 기본값이고,
//! `LEAD_OVERRIDE_MS` 로 덮어쓸 수 있습니다. 예고는 항상 노트 시각 *이전에* 시작합니다.
//! (노트 1.465초, 예고 150ms → 1.315초부터 진동)

use std::time::Duration;

use graphics::{Graphics, style::Style};
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult, KeypadSide};
use sdk::applet::{LocalizedString, SpeechOption, SpeechResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// 튜닝 상수
// ---------------------------------------------------------------------------

/// 노트가 다가올 때 원이 반응하는 방식입니다.
///
/// 점자 핀은 물리적으로 두 가지 다른 방식으로 움직일 수 있고, 손끝에 닿는 느낌이 완전히 다릅니다.
/// 펌웨어의 핀 구동 루프(`firmware/src/braille_display.rs`)를 보면:
///
/// * **PWM 강도** — 약 62.5Hz 고정 반송파의 듀티비를 바꿉니다. 세기가 서서히 차오르는 느낌.
///   기기 부팅/종료 애니메이션이 바로 이 방식입니다.
/// * **점멸(blink)** — 1·2·4·8·16·32Hz 중 하나로 핀을 완전히 올렸다 내립니다. 또각또각 끊기는 느낌.
///
/// 두 방식은 `tactile-display-demo` 의 `Static8` / `Blink8` 과 정확히 같은 것을,
/// 공간(상자 8개)이 아니라 시간축(예고 구간)에 펼친 것입니다.
/// 실기에서 바로 비교할 수 있도록 **기능 키(Function)로 전환**합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VibrationMode {
    /// 약 62.5Hz 반송파의 **듀티비**로 세기를 냅니다.
    /// 기기 부팅·종료 애니메이션과 같은 방식입니다.
    /// (`tactile-display-demo` 의 `Static8`, `firmware/src/shutdown_animation.rs`)
    DutyRatio,
    /// 같은 단계를 하드웨어 **점멸 주기**로 냅니다.
    /// 1~6 단계가 1·2·4·8·16·32Hz, 7 단계는 항상 켜짐입니다.
    /// (`tactile-display-demo` 의 `Blink8`)
    BlinkPeriod,
}

/// 애플릿을 켰을 때의 기본 방식입니다.
const DEFAULT_VIBRATION_MODE: VibrationMode = VibrationMode::DutyRatio;

impl VibrationMode {
    /// 이 방식에서 쓸 수 있는 단계 범위입니다.
    /// 두 방식 모두 하드웨어 전 범위(0~7)를 씁니다.
    fn level_range(self) -> (u8, u8) {
        (MIN_VIBRATION_LEVEL, MAX_VIBRATION_LEVEL)
    }

    /// 기능 키를 누를 때마다 다른 방식으로 넘어갑니다.
    fn next(self) -> Self {
        match self {
            VibrationMode::DutyRatio => VibrationMode::BlinkPeriod,
            VibrationMode::BlinkPeriod => VibrationMode::DutyRatio,
        }
    }

    /// 전환 시 음성으로 읽어 줄 이름입니다. 시연 중 지금 무엇을 만지고 있는지 알려 줍니다.
    fn label(self, lang: Language) -> &'static str {
        match (self, lang) {
            (VibrationMode::DutyRatio, Language::Ko) => {
                "듀티 레이시오 기반. 부팅 애니메이션과 같은 방식입니다."
            }
            (VibrationMode::BlinkPeriod, Language::Ko) => {
                "주기 기반. 하드웨어가 직접 깜빡입니다."
            }
            (VibrationMode::DutyRatio, Language::Ja) => {
                "強さの立ち上がり。起動アニメーションと同じ方式です。"
            }
            (VibrationMode::BlinkPeriod, Language::Ja) => "点滅8段階。ハードウェアが点滅します。",
            (VibrationMode::DutyRatio, _) => "Intensity swell, same as the boot animation.",
            (VibrationMode::BlinkPeriod, _) => "Eight blink steps, driven by the hardware.",
        }
    }
}

/// 진동 단계의 범위. **단계 번호가 곧 하드웨어 레벨**입니다.
///
/// 강도가 4비트로 양자화되어 하드웨어 레벨은 0~7 의 여덟 개뿐입니다.
/// 단계가 그대로 4비트 코드가 되고, 펌웨어가 그 코드로 듀티를 만듭니다.
/// `counter` 가 16씩 도는 16칸이고 `threshold = 코드 * 32` 이므로
/// (`firmware/src/braille_display.rs`) 듀티는 **12.5% 간격**입니다.
///
/// | 단계 | 코드 | 듀티 |
/// |---|---|---|
/// | 0 | 0 | 0% (꺼짐) |
/// | 1 | 1 | 12.5% |
/// | 2 | 2 | 25% |
/// | 3 | 3 | 37.5% |
/// | 4 | 4 | 50% |
/// | 5 | 5 | 62.5% |
/// | 6 | 6 | 75% |
/// | 7 | 7 | **100%** (87.5% 가 아닙니다) |
///
/// 7단계만 간격이 어긋납니다. 펌웨어가 코드 7 을 `7 => true` 로 특수 처리해
/// 듀티 계산을 거치지 않고 항상 켜 두기 때문입니다. 애플릿에서는 못 고칩니다 —
/// 4비트 듀티 코드가 0~7 뿐이라 87.5% 를 지시할 코드가 없습니다.
///
/// 예고 구간 내내 이 레벨 하나로 고정됩니다. 세기가 변하지 않으므로
/// 노트당 전송은 켜기 1회 + 끄기 1회뿐이고, 단계를 올려도 전송량은 그대로입니다.
///
/// 두 방식 모두 전 범위를 쓸 수 있습니다. 점멸 모드의 낮은 단계는 반주기가
/// 예고 창보다 길어 주기가 제대로 담기지 않지만(`blink_frequency_hz` 주석 참고),
/// **확인용으로 고를 수 있도록 막지 않습니다.** 대신 로그로 알려 줍니다.
const MIN_VIBRATION_LEVEL: u8 = 0;
const MAX_VIBRATION_LEVEL: u8 = 7;

/// 켰을 때의 기본 단계. A/D 키로 바꿉니다.
const DEFAULT_VIBRATION_LEVEL: u8 = MAX_VIBRATION_LEVEL;

/// `blink` 플래그를 보존하면서 채워진 원을 그립니다.
///
/// **`graphics` 확장의 `draw_circle`/`draw_rectangle` 을 쓰면 안 됩니다.**
/// 그 경로는 `embedded-graphics` 의 `Gray8`(8비트 밝기 하나)로 색을 다루고,
/// 픽셀을 찍을 때 `Intensity::from(color.luma())` 로 되돌립니다
/// (`extensions/graphics/src/lib.rs`). `luma()` 는 `u8` 뿐이라
/// **`blink` 가 그 지점에서 조용히 버려집니다.** 점멸 모드가 듀티 모드로 둔갑합니다.
///
/// `tactile-display-demo` 의 격자가 제대로 깜빡이는 이유도 이것입니다 —
/// 거기서는 `display.set_pin(point, intensity)` 로 `Intensity` 를 통째로 넘깁니다
/// (`extensions/widget/src/item_selector/grid.rs`).
fn fill_circle(
    canvas: &mut dyn DisplayInterface,
    center: Point,
    diameter: i16,
    intensity: Intensity,
) {
    // `graphics` 의 `draw_circle` 과 같은 기준점 계산입니다.
    let top_left_x = center.x - diameter / 2;
    let top_left_y = center.y - diameter / 2;
    // 중심을 반 픽셀 보정해 두 방식의 모양을 맞춥니다.
    let cx = top_left_x as f32 + (diameter as f32 - 1.0) / 2.0;
    let cy = top_left_y as f32 + (diameter as f32 - 1.0) / 2.0;
    let r = diameter as f32 / 2.0;

    for y in top_left_y..top_left_y + diameter {
        for x in top_left_x..top_left_x + diameter {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r * r {
                canvas.set_pin(Point::new(x, y), intensity);
            }
        }
    }
}

/// 점멸 모드에서 한 단계가 내는 주파수(Hz)입니다. 7단계는 항상 켜짐이라 없습니다.
///
/// 펌웨어가 4비트 코드 9~14 를 반주기 500·250·125·62.5·31.25·15.625ms 로 해석합니다
/// (`firmware/src/braille_display.rs`). 런타임이 `8 + 레벨` 로 코드를 만드므로
/// 레벨 1~6 이 코드 9~14, 레벨 7 이 코드 15(항상 켜짐)가 됩니다.
fn blink_frequency_hz(level: u8) -> Option<f32> {
    match level {
        1 => Some(1.0),
        2 => Some(2.0),
        3 => Some(4.0),
        4 => Some(8.0),
        5 => Some(16.0),
        6 => Some(32.0),
        _ => None,
    }
}

/// 단계(= 하드웨어 레벨 0~7)를 그 레벨이 나오는 강도값으로 바꿉니다.
///
/// 런타임이 `(v*7+127)/255` 로 양자화하므로, 그 역함수에 해당합니다.
fn level_to_value(level: u8) -> u8 {
    (level.min(7) as u16 * 255 / 7) as u8
}

/// 재생 중인 음악을 끊기 위해 쓰는 아주 짧은 무음 클립입니다.
///
/// audio-service 의 `POST /sound` 핸들러는 새 소리를 재생하기 전에 반드시
/// `audio_player.clear(PlaybackCategory::SoundEffect)` 를 먼저 호출합니다.
/// 별도의 정지 API 가 없으므로, 무음을 한 번 재생해서 재생 중인 곡을 비웁니다.
const SILENCE: &[u8] = include_bytes!("audio/silence.mp3");

/// 음악 재생 요청 ~ 실제 소리가 나기까지의 지연 보정값(ms).
///
/// 정적 오디오는 audio-service 로 HTTP 요청을 보낸 뒤 디코딩·리샘플링을 거쳐 재생되므로,
/// `context.audio.play()` 를 호출한 시점과 실제로 소리가 나는 시점 사이에 지연이 있습니다.
/// 게임 시계는 호출 시점부터 도므로 그만큼 **게임이 음악보다 앞서갑니다.**
///
/// `now_s = 경과시간 + AUDIO_OFFSET_MS/1000` 이므로 부호는 다음과 같습니다.
/// * **음수** — 게임을 그만큼 늦춥니다. 음악이 늦게 시작되는 보통의 경우에 씁니다.
/// * **양수** — 게임을 그만큼 당깁니다.
///
/// 판정이 곡 내내 일정하게 한쪽으로 쏠릴 때만 이 값으로 맞추세요.
/// 곡이 진행될수록 점점 어긋난다면 그건 오프셋 문제가 아니라 리샘플링 드리프트입니다.
/// (`audio-service/src/audio/format.rs` 의 `resample_linear` 참고)
const AUDIO_OFFSET_MS: i64 = 0;

/// 판정 단계 정의. **이 표 하나가 판정에 관한 유일한 기준입니다.**
///
/// 각 항목은 `(판정, 허용 오차(초), 점수)` 이며, 오차가 작은 것부터 차례로 검사합니다.
/// 허용 오차는 노트 시각을 기준으로 **앞뒤 양쪽**에 적용됩니다.
/// (Perfect 0.10 이면 -100ms ~ +100ms)
///
/// 단계를 추가·삭제하거나 점수를 바꾸려면 이 표만 고치면 됩니다.
/// 아래 것들이 전부 여기서 파생됩니다.
/// * 어떤 판정을 받는지 (`Judge::from_error`)
/// * 점수 (`Judge::score`)
/// * 칠 수 있는 한계 시각 (`HIT_WINDOW_S`)
/// * 정확도 계산 (`RhythmGame::accuracy`)
///
/// Miss 는 "표의 어디에도 못 든 경우"라서 표에 넣지 않습니다.
const HIT_TIERS: [(Judge, f32, u32); 2] = [(Judge::Perfect, 0.15, 100), (Judge::Good, 0.30, 50)];

/// 노트를 칠 수 있는 마지막 경계(초) = 표의 가장 너그러운 허용 오차.
///
/// 이 시간을 넘기면 누르든 말든 Miss 입니다. 예전에는 "칠 수 있는 한계(0.22초)"와
/// "Miss 로 확정하는 시각(0.30초)"이 따로 있어서, 그 사이 80ms 동안 눌러도
/// 아무 반응이 없는 사각지대가 있었습니다. 지금은 하나로 통일했습니다.
const HIT_WINDOW_S: f32 = HIT_TIERS[HIT_TIERS.len() - 1].1;

/// 예고 시간을 읽지 못했을 때 사용할 기본값(초)
const DEFAULT_LEAD_S: f32 = 0.3;

/// 애플릿이 핀 값을 바꾼 뒤 그 진동이 손끝에 닿기까지의 출력 지연(ms).
///
/// **진동만 이만큼 미리** 내보냅니다. 판정 시계(`now_s`)는 건드리지 않습니다.
/// 둘을 같이 밀면 진동을 당긴 만큼 판정도 밀려 아무 의미가 없습니다.
///
/// 이 경로는 오디오와 무관해서 `/position` 동기화로는 보정되지 않습니다.
/// 기기 로그로 확인한 바, 애플릿의 진동 창 자체는 정확히 `[T-예고, T]` 이고
/// 타격 평균 오차도 -5ms 였습니다. 즉 남은 어긋남은 전부 이 구간에서 생깁니다.
///
/// | 구간 | 시간 | 근거 |
/// |---|---|---|
/// | UART 전송 (지름 18 원 = 756B) | 66ms | 756 ÷ 11,520B/s |
/// | PWM 주기 정렬 | 최대 16ms | `PWM_STEP`(16) × `PWM_INTERVAL`(1ms) |
/// | 합계 | **약 82ms** | |
///
/// 여기에 핀이 "떨림"으로 인지되기까지 반송파가 몇 주기 돌아야 하는 몫이 더 붙습니다
/// (62.5Hz 에서 5주기 ≈ 80ms). 이 부분은 센서 없이는 못 재므로 포함하지 않았습니다.
///
/// 그래서 측정·유도 가능한 82ms 를 반올림해 85 로 둡니다.
/// 이전 값 30ms 는 이 경로를 재지 않고 타격 오차 평균만 보고 넣은 값이라
/// 실제 지연의 1/3 밖에 못 메웠습니다.
///
/// 여전히 늦게 느껴지면 키우고, 앞서 느껴지면 줄이세요.
const TACTILE_OUTPUT_DELAY_MS: i64 = 85;

/// 진동하는 원의 지름(핀 개수).
///
/// 크기 자체도 요구사항이지만, 이 값은 **전체 프레임 전송을 보장하는 역할**도 합니다.
///
/// 런타임은 `diff_data.len() * 2 >= 768` 일 때만 전체 프레임(`UpdateDisplay`)을 보내고
/// (`runtime-native/src/hal/display/braille_display.rs`), 펌웨어는 **그 경로에서만**
/// `IS_4BIT_MODE` 를 켭니다. 차이 경로는 읽기만 합니다(`firmware/src/bin/main.rs`).
/// 플래그가 꺼져 있으면 점멸 코드 9~14 가 듀티값 153~238 로 둔갑해,
/// 점멸 모드가 그냥 센 듀티 모드가 됩니다.
///
/// 원이 384핀 이상이면 켜고 끌 때마다 전체 프레임이 나가므로 플래그가 계속 유지됩니다.
///
/// | 지름 | 핀 | diff×2 | 전체 프레임 |
/// |---|---|---|---|
/// | 18 | 256 | 512 | 아니오 |
/// | 22 | 384 | 768 | 예 (경계) |
/// | **24** | **448** | **896** | **예** |
///
/// 전송량은 전체 프레임 768B 로, 지름 18 의 차이 전송 756B 와 사실상 같습니다(약 66ms).
/// 즉 원을 키워서 얻는 안정성이 공짜입니다.
const VIBRATING_DISC_DIAMETER: i16 = 24;

// --- 오디오 재생 위치 동기화 -------------------------------------------------
//
// `context.audio.play()` 는 audio-service 에 재생을 **요청**할 뿐이고,
// 실제로 첫 샘플이 스피커로 나가기까지는 디코딩·리샘플링 시간만큼 걸립니다.
// 실측값: CM5 릴리스 빌드 약 180ms, PC 디버그 빌드 약 1,580ms.
// 요청 시점부터 게임 시계를 돌리면 그만큼 게임이 음악보다 앞서 버립니다.
//
// 그래서 audio-service 의 `GET /position` 으로 **실제 재생 위치**를 읽어
// 게임 시계를 거기에 맞춥니다. 지연이 얼마든, 어떤 기기든 알아서 흡수됩니다.

/// audio-service 의 재생 위치 조회 주소.
/// 포트 3005 는 `runtime_common::audio::protocol::TTS_SERVICE_DEFAULT_PORT` 와 같은 값인데,
/// 애플릿(WASM)은 호스트 크레이트에 의존할 수 없어 여기에 복제해 둡니다.
const POSITION_URL: &str = "http://127.0.0.1:3005/position";

/// 재생이 시작되기를 기다리는 동안의 조회 간격(ms)
const SYNC_POLL_MS: u64 = 40;
/// 동기화가 끝난 뒤 어긋남을 확인하는 간격(ms)
const RESYNC_INTERVAL_MS: u64 = 5_000;
/// 재확인 시 이 이상 벌어졌을 때만 시계를 다시 맞춥니다.
const RESYNC_THRESHOLD_S: f32 = 0.05;
/// 이 시간 안에 재생이 시작되지 않으면 동기화를 포기하고 요청 시점 기준으로 진행합니다.
/// (audio-service 가 꺼져 있어도 게임이 멈추지 않도록 하는 안전장치)
const SYNC_TIMEOUT_MS: u64 = 5_000;

/// `GET /position` 응답
#[derive(Deserialize)]
struct PositionResponse {
    seconds: f64,
}

/// 노트 예고 시간을 코드에서 직접 지정합니다.
///
/// `None` 이면 채보의 `meta.seconds_per_beat`(현재 0.3초)를 그대로 씁니다.
/// 예고가 짧을수록 촉각 신호가 날카로워지지만, 그만큼 표현할 수 있는 단계가 줄어듭니다.
///
/// | 예고 | 8단계 한 칸 | 비고 |
/// |---|---|---|
/// | 300ms | 37.5ms | 채보 기본값 |
/// | 200ms | 25.0ms | |
/// | 150ms | 18.8ms | 현재 값 |
/// | 100ms | 12.5ms | PWM 반송파(16ms)보다 짧아 단계 구분이 어려움 |
///
/// `VibrationMode::BlinkPeriod` 는 200ms 아래에서는 제 기능을 못 합니다.
/// 느린 단계(1·2·4·8Hz)의 한 주기가 각 칸보다 길어서 켜짐/꺼짐이 한 번도 안 일어납니다.
const LEAD_OVERRIDE_MS: Option<u32> = Some(200);

// ---------------------------------------------------------------------------
// 곡 에셋
// ---------------------------------------------------------------------------

/// 컴파일 시점에 바이너리에 포함되는 곡 하나의 원본 자료입니다.
struct SongAsset {
    /// [ko, en, ja] 순서의 곡 제목
    title: [&'static str; 3],
    chart_json: &'static str,
    audio: &'static [u8],
}

/// 곡을 추가하려면 `src/audio/` 에 mp3 + json 을 넣고 여기에 한 줄 추가하면 됩니다.
const SONG_ASSETS: &[SongAsset] = &[SongAsset {
    title: ["꼬마 눈사람", "Little Snowman", "小さな雪だるま"],
    chart_json: include_str!("audio/little_snowman.json"),
    audio: include_bytes!("audio/little_snowman.mp3"),
}];

// --- 채보 JSON 스키마 -------------------------------------------------------

#[derive(Deserialize, Default)]
struct ChartMeta {
    /// 예고 시간(초). 이 값만큼 **노트 시각 이전부터** 진동이 시작됩니다.
    #[serde(default)]
    seconds_per_beat: f32,
}

#[derive(Deserialize)]
struct ChartNote {
    time: f32,
}

#[derive(Deserialize)]
struct ChartFile {
    #[serde(default)]
    meta: ChartMeta,
    #[serde(default)]
    notes: Vec<ChartNote>,
}

/// 파싱이 끝난, 게임이 실제로 사용하는 곡 정보입니다.
struct Song {
    title: [&'static str; 3],
    audio: &'static [u8],
    /// 예고 시간(초)
    lead: f32,
    /// 노트 시각(초). 오름차순으로 정렬되어 있습니다.
    notes: Vec<f32>,
    /// mp3 헤더에서 계산한 실제 재생 길이(초). 파싱에 실패하면 `None`.
    audio_secs: Option<f32>,
}

impl Song {
    fn title(&self, lang: Language) -> &'static str {
        match lang {
            Language::Ko => self.title[0],
            Language::En => self.title[1],
            Language::Ja => self.title[2],
        }
    }

    /// 결과 화면으로 넘어가는 시각(초)입니다.
    ///
    /// **음악을 중간에 끊지 않고 끝까지 들려준 뒤** 결과를 알려 주기 위해,
    /// mp3 의 실제 재생 길이를 기준으로 삼습니다.
    /// 헤더를 읽지 못했을 때만 마지막 노트 + 여유 시간으로 대체합니다.
    /// 진행 막대의 기준 길이이기도 합니다.
    fn end_secs(&self) -> f32 {
        let after_last_note = self.notes.last().copied().unwrap_or(0.0) + HIT_WINDOW_S;
        match self.audio_secs {
            // 채보가 음원보다 길게 잡혀 있어도 마지막 노트 판정은 끝내고 넘어갑니다.
            Some(secs) => secs.max(after_last_note),
            None => after_last_note + 2.0,
        }
    }
}

/// MP3 프레임 헤더를 훑어 실제 재생 길이(초)를 계산합니다.
///
/// 재생이 끝나는 시점을 알려 주는 API 가 SDK 에 없어서(오디오 완료 이벤트가 애플릿까지
/// 전달되지 않습니다) 음원 길이를 직접 구합니다. CBR/VBR 모두 프레임 단위로 합산하므로
/// 나중에 어떤 mp3 를 넣어도 동작합니다.
fn mp3_duration_secs(data: &[u8]) -> Option<f32> {
    // Layer III 비트레이트 표 (kbps)
    const V1_L3: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    const V2_L3: [u32; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];
    // [MPEG1, MPEG2, MPEG2.5] × 샘플레이트 인덱스
    const RATES: [[u32; 3]; 3] = [
        [44100, 48000, 32000],
        [22050, 24000, 16000],
        [11025, 12000, 8000],
    ];

    let mut i = 0usize;

    // ID3v2 태그는 동기워드처럼 보이는 바이트를 품을 수 있으므로 통째로 건너뜁니다.
    if data.len() > 10 && &data[..3] == b"ID3" {
        let size = (((data[6] & 0x7f) as usize) << 21)
            | (((data[7] & 0x7f) as usize) << 14)
            | (((data[8] & 0x7f) as usize) << 7)
            | ((data[9] & 0x7f) as usize);
        i = 10 + size;
    }

    let mut total_samples: u64 = 0;
    let mut rate_hz: u32 = 0;

    while i + 4 <= data.len() {
        // 프레임 동기워드(11비트 전부 1)를 찾습니다.
        if data[i] != 0xFF || (data[i + 1] & 0xE0) != 0xE0 {
            i += 1;
            continue;
        }

        let ver = (data[i + 1] >> 3) & 0x03; // 3=MPEG1, 2=MPEG2, 0=MPEG2.5, 1=예약
        let layer = (data[i + 1] >> 1) & 0x03; // 1 = Layer III
        let br_idx = ((data[i + 2] >> 4) & 0x0F) as usize;
        let sr_idx = ((data[i + 2] >> 2) & 0x03) as usize;
        let pad = ((data[i + 2] >> 1) & 0x01) as usize;

        if layer != 1 || ver == 1 || sr_idx == 3 || br_idx == 0 || br_idx == 15 {
            i += 1;
            continue;
        }

        let ver_row = match ver {
            3 => 0,
            2 => 1,
            _ => 2,
        };
        let rate = RATES[ver_row][sr_idx];
        let bitrate = if ver == 3 {
            V1_L3[br_idx]
        } else {
            V2_L3[br_idx]
        } * 1000;
        let samples_per_frame: u32 = if ver == 3 { 1152 } else { 576 };
        let frame_len = (samples_per_frame as usize / 8) * bitrate as usize / rate as usize + pad;
        if frame_len == 0 {
            i += 1;
            continue;
        }

        total_samples += samples_per_frame as u64;
        rate_hz = rate;
        i += frame_len;
    }

    if rate_hz == 0 || total_samples == 0 {
        None
    } else {
        Some(total_samples as f32 / rate_hz as f32)
    }
}

/// 채보 JSON 을 파싱합니다. 단일 레인 게임이므로 `lane` 필드는 사용하지 않습니다.
fn load_songs() -> Vec<Song> {
    SONG_ASSETS
        .iter()
        .filter_map(
            |asset| match serde_json::from_str::<ChartFile>(asset.chart_json) {
                Ok(chart) => {
                    let lead = if chart.meta.seconds_per_beat > 0.0 {
                        chart.meta.seconds_per_beat
                    } else {
                        log::warn!(
                            "{}: seconds_per_beat 가 없어 기본값 {}초를 사용합니다.",
                            asset.title[1],
                            DEFAULT_LEAD_S
                        );
                        DEFAULT_LEAD_S
                    };

                    let mut notes: Vec<f32> = chart.notes.iter().map(|n| n.time).collect();
                    notes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                    let audio_secs = mp3_duration_secs(asset.audio);
                    if audio_secs.is_none() {
                        log::warn!(
                            "{}: mp3 길이를 읽지 못했습니다. 마지막 노트 기준으로 종료합니다.",
                            asset.title[1]
                        );
                    }

                    log::info!(
                        "{}: 노트 {}개, 예고 {}초, 음원 {:?}초",
                        asset.title[1],
                        notes.len(),
                        lead,
                        audio_secs
                    );

                    Some(Song {
                        title: asset.title,
                        audio: asset.audio,
                        lead,
                        notes,
                        audio_secs,
                    })
                }
                Err(e) => {
                    log::error!("{} 채보 파싱 실패: {}", asset.title[1], e);
                    None
                }
            },
        )
        .collect()
}

// ---------------------------------------------------------------------------
// 게임 상태
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judge {
    Perfect,
    Good,
    Miss,
}

impl Judge {
    /// 노트 시각과의 오차(초, 절댓값)로 판정을 결정합니다.
    /// 어느 단계에도 못 들면 `None` — 헛손질이라 판정 자체를 하지 않습니다.
    fn from_error(error_secs: f32) -> Option<Judge> {
        HIT_TIERS
            .iter()
            .find(|(_, tolerance, _)| error_secs <= *tolerance)
            .map(|(judge, _, _)| *judge)
    }

    /// 이 판정을 받기 위한 최대 오차(초). Miss 는 경계가 없으므로 `None`.
    /// 게임 로직은 `from_error` 만 쓰므로, 표를 검증하는 테스트에서만 필요합니다.
    #[cfg(test)]
    fn tolerance(self) -> Option<f32> {
        HIT_TIERS
            .iter()
            .find(|(judge, _, _)| *judge == self)
            .map(|(_, tolerance, _)| *tolerance)
    }

    fn score(self) -> u32 {
        HIT_TIERS
            .iter()
            .find(|(judge, _, _)| *judge == self)
            .map(|(_, _, score)| *score)
            .unwrap_or(0)
    }

    /// 콤보가 이어지는 판정인지 여부입니다.
    fn keeps_combo(self) -> bool {
        self != Judge::Miss
    }

    /// 판정 직후 화면 맨 윗줄에 그릴 막대의 가로 비율입니다.
    /// 소리를 못 듣는 상황에서도 방금 판정을 눈으로 구분할 수 있게 합니다.
    fn flash_ratio(self) -> f32 {
        match self {
            Judge::Perfect => 1.0,
            Judge::Good => 0.5,
            Judge::Miss => 1.0 / 6.0,
        }
    }

    /// 가장 좋은 판정의 점수. 정확도 계산의 분모로 씁니다.
    fn best_score() -> u32 {
        HIT_TIERS
            .iter()
            .map(|(_, _, score)| *score)
            .max()
            .unwrap_or(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameState {
    /// 곡 선택 화면
    SongSelect,
    /// 연주 중
    Playing,
    /// 결과 화면
    Result,
}

/// 한 프레임에 실제로 그려질 내용입니다.
/// 이전 프레임과 비교해서 달라졌을 때만 다시 그리므로, 90fps 로 매번 갱신하지 않습니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Visual {
    /// 원 내부를 채울 강도. `Intensity::OFF` 면 테두리만 그립니다.
    fill: Intensity,
    /// 진행 막대의 길이(픽셀)
    progress_px: i16,
    /// 판정 직후 잠깐 켜지는 표시
    flash: bool,
}

pub struct RhythmGame {
    songs: Vec<Song>,
    selected: usize,
    state: GameState,
    /// 기능 키로 실시간 전환되는 진동 방식
    vibration_mode: VibrationMode,

    /// 곡 재생을 시작한 모노토닉 시각
    started_at: Duration,
    /// 곡 시작 기준 현재 시간(초)
    now_s: f32,
    /// 아직 판정되지 않은 첫 노트의 인덱스
    next_note: usize,

    visual: Visual,

    score: u32,
    combo: u32,
    max_combo: u32,
    perfect: u32,
    good: u32,
    miss: u32,
    last_judge: Option<Judge>,
    /// 판정 표시를 언제까지 유지할지(곡 기준 초)
    flash_until_s: f32,

    /// 판정된 입력들의 **부호 있는** 타이밍 오차 합(초). 양수 = 게임 시계 기준 늦게 누름.
    /// 남은 오차를 로그로 확인할 때 씁니다.
    timing_error_sum: f32,
    /// 위 합에 포함된 입력 개수 (Miss 는 오차를 정의할 수 없어 제외)
    timing_error_count: u32,

    /// 진행 중인 `/position` 요청 id
    sync_pending: Option<u32>,
    /// 마지막으로 `/position` 을 요청한 모노토닉 시각 (조회 주기 제어용)
    sync_last_poll: Duration,
    /// 현재 진행 중인 `/position` 요청을 **보낸** 시각.
    /// 서버가 위치를 읽은 시점이 이 값에 가까우므로, 시계를 맞출 때의 기준으로 씁니다.
    sync_request_at: Duration,
    /// 게임 시계를 실제 재생 위치에 맞추는 데 성공했는지 여부.
    /// `false` 인 동안에는 시계를 0 에 묶어 두고 판정도 하지 않습니다.
    sync_locked: bool,

    /// 계측용: 직전 프레임의 진동 세기. 꺼짐→켜짐 전환을 잡아내는 데 씁니다.
    /// 진동 단계 = 하드웨어 레벨 (1~7). A/D 키로 조절합니다.
    vibration_level: u8,

    probe_prev_fill: u8,
    /// 계측용: 이번 노트의 진동이 처음 켜진 `now_s`.
    probe_onset_s: Option<f32>,
    /// 재생을 요청한 모노토닉 시각 (동기화 타임아웃 판정용)
    play_requested_at: Duration,
}

impl Default for RhythmGame {
    fn default() -> Self {
        Self {
            songs: Vec::new(),
            selected: 0,
            state: GameState::SongSelect,
            vibration_mode: DEFAULT_VIBRATION_MODE,
            started_at: Duration::ZERO,
            now_s: 0.0,
            next_note: 0,
            visual: Visual::default(),
            score: 0,
            combo: 0,
            max_combo: 0,
            perfect: 0,
            good: 0,
            miss: 0,
            last_judge: None,
            flash_until_s: 0.0,
            timing_error_sum: 0.0,
            timing_error_count: 0,
            sync_pending: None,
            sync_last_poll: Duration::ZERO,
            sync_request_at: Duration::ZERO,
            sync_locked: false,
            vibration_level: DEFAULT_VIBRATION_LEVEL,
            probe_prev_fill: 0,
            probe_onset_s: None,
            play_requested_at: Duration::ZERO,
        }
    }
}

impl RhythmGame {
    fn song(&self) -> Option<&Song> {
        self.songs.get(self.selected)
    }

    /// 실제로 적용되는 예고 시간(초)입니다.
    /// `LEAD_OVERRIDE_MS` 가 설정되어 있으면 채보 값보다 우선합니다.
    fn lead(&self) -> f32 {
        match LEAD_OVERRIDE_MS {
            Some(ms) => ms as f32 / 1000.0,
            None => self.song().map(|s| s.lead).unwrap_or(DEFAULT_LEAD_S),
        }
    }

    /// 아직 치지 않은 다음 노트의 시각(초)
    fn upcoming_note(&self) -> Option<f32> {
        self.song()
            .and_then(|s| s.notes.get(self.next_note))
            .copied()
    }

    fn start_song(&mut self, context: &mut Context, now: Duration) {
        let Some(song) = self.songs.get(self.selected) else {
            return;
        };
        let audio = song.audio;
        let note_count = song.notes.len();

        // 음악 재생을 먼저 요청하고, 그 시점을 곡의 0초로 잡습니다.
        context.audio.play(audio);

        self.started_at = now;
        self.now_s = 0.0;
        self.next_note = 0;
        self.score = 0;
        self.combo = 0;
        self.max_combo = 0;
        self.perfect = 0;
        self.good = 0;
        self.miss = 0;
        self.last_judge = None;
        self.flash_until_s = 0.0;
        self.timing_error_sum = 0.0;
        self.timing_error_count = 0;
        self.sync_pending = None;
        self.sync_last_poll = Duration::ZERO;
        self.sync_request_at = Duration::ZERO;
        self.sync_locked = false;
        self.play_requested_at = now;
        self.state = GameState::Playing;

        log::info!("연주 시작: 노트 {}개, 예고 {}초", note_count, self.lead());
    }

    /// 재생 중인 음악을 멈춥니다.
    ///
    /// SDK 에는 정지 API 가 없지만, audio-service 의 `/sound` 핸들러가 새 소리를 재생하기 전에
    /// 효과음 트랙을 먼저 비우기 때문에, 짧은 무음을 흘려보내면 곡이 끊깁니다.
    fn stop_music(&self, context: &mut Context) {
        context.audio.play(SILENCE);
        log::debug!("음악 정지 요청");
    }

    fn apply_judge(&mut self, judge: Judge) {
        match judge {
            Judge::Perfect => self.perfect += 1,
            Judge::Good => self.good += 1,
            Judge::Miss => self.miss += 1,
        }
        if judge.keeps_combo() {
            self.combo += 1;
        } else {
            self.combo = 0;
        }
        self.max_combo = self.max_combo.max(self.combo);
        self.score += judge.score();
        self.last_judge = Some(judge);
        self.flash_until_s = self.now_s + 0.2;

        log::debug!(
            "{:?} @ {:.3}s (score={}, combo={})",
            judge,
            self.now_s,
            self.score,
            self.combo
        );
    }

    /// 가운데 키를 눌렀을 때의 판정입니다.
    fn on_hit(&mut self) {
        let Some(target) = self.upcoming_note() else {
            return;
        };

        // 부호 있는 오차: 양수면 게임 시계 기준으로 늦게 누른 것입니다.
        let signed_error = self.now_s - target;
        let Some(judge) = Judge::from_error(signed_error.abs()) else {
            // 아직 예고조차 시작되지 않았거나 이미 놓친 노트 — 헛손질은 감점 없이 무시합니다.
            return;
        };

        self.timing_error_sum += signed_error;
        self.timing_error_count += 1;

        self.next_note += 1;
        self.apply_judge(judge);
    }

    /// audio-service 의 `GET /position` 을 폴링해 게임 시계를 실제 재생 위치에 맞춥니다.
    ///
    /// 재생이 시작되기 전에는 `/position` 이 0 을 돌려주므로, 0 보다 커지는 순간이
    /// 곧 "음악이 실제로 흘러나오기 시작한 시점"입니다. 그때 `started_at` 을
    /// `지금 - 재생위치` 로 잡으면 디코딩 지연이 얼마였든 정확히 흡수됩니다.
    fn poll_audio_sync(&mut self, context: &mut Context, now: Duration) {
        // 1) 도착한 응답 처리
        if let Some(id) = self.sync_pending
            && let Some((status, body)) = context.http.take_response(id)
        {
            self.sync_pending = None;
            if status == 200
                && let Ok(res) = serde_json::from_slice::<PositionResponse>(&body)
                && res.seconds > 0.0
            {
                // 기준 시각은 응답을 **받은** 때가 아니라 요청을 **보낸** 때입니다.
                //
                // 서버는 요청을 받자마자 원자 변수 하나를 읽어 돌려주므로, 그 값이 가리키는
                // 시점은 요청 직후입니다. 반면 응답이 애플릿까지 오는 데는 왕복 시간에 더해
                // 런타임이 HttpResponse 이벤트를 다음 프레임에 전달하는 지연까지 얹힙니다.
                // 응답 수신 시각을 쓰면 그만큼 `started_at` 이 뒤로 밀려 게임이 음악보다 늦어집니다.
                let sampled_at = self.sync_request_at;
                let measured_start =
                    sampled_at.saturating_sub(Duration::from_secs_f64(res.seconds));
                if !self.sync_locked {
                    self.started_at = measured_start;
                    self.sync_locked = true;
                    let delay_ms = measured_start
                        .saturating_sub(self.play_requested_at)
                        .as_secs_f32()
                        * 1000.0;
                    let rtt_ms = now.saturating_sub(sampled_at).as_secs_f32() * 1000.0;
                    log::info!(
                        "오디오 동기화 완료: 재생이 요청보다 {delay_ms:.0}ms 늦게 시작됨 (응답 왕복 {rtt_ms:.0}ms)"
                    );
                } else {
                    let drift =
                        sampled_at.saturating_sub(self.started_at).as_secs_f64() - res.seconds;
                    if drift.abs() as f32 > RESYNC_THRESHOLD_S {
                        self.started_at = measured_start;
                        log::info!("오디오 재동기화: {:.0}ms 보정", drift * 1000.0);
                    }
                }
            }
        }

        // 2) 안전장치 — audio-service 가 없거나 응답이 없으면 요청 시점 기준으로 진행합니다.
        if !self.sync_locked
            && now.saturating_sub(self.play_requested_at) > Duration::from_millis(SYNC_TIMEOUT_MS)
        {
            log::warn!("재생 위치를 확인하지 못했습니다. 요청 시점 기준으로 진행합니다.");
            self.started_at = self.play_requested_at;
            self.sync_locked = true;
        }

        // 3) 다음 조회 요청
        let interval = if self.sync_locked {
            RESYNC_INTERVAL_MS
        } else {
            SYNC_POLL_MS
        };
        if self.sync_pending.is_none()
            && now.saturating_sub(self.sync_last_poll) >= Duration::from_millis(interval)
        {
            self.sync_last_poll = now;
            match context.http.fetch_async("GET", POSITION_URL, &[], &[]) {
                Ok(id) => {
                    self.sync_pending = Some(id);
                    self.sync_request_at = now;
                }
                Err(code) => log::debug!("재생 위치 조회 요청 실패 (코드 {code})"),
            }
        }
    }

    /// 결과 화면으로 넘어갈 때가 되었는지 판단합니다.
    ///
    /// 마지막 노트를 판정했더라도 **음악이 끝날 때까지 기다립니다.**
    /// 곡을 중간에 잘라내지 않고 끝까지 들려준 뒤 결과를 알려 주기 위해서입니다.
    fn is_finished(&self) -> bool {
        self.song()
            .map(|s| self.next_note >= s.notes.len() && self.now_s >= s.end_secs())
            .unwrap_or(true)
    }

    /// 판정 시간을 넘긴 노트를 Miss 처리합니다.
    fn reap_missed_notes(&mut self) {
        while let Some(target) = self.upcoming_note() {
            if self.now_s - target <= HIT_WINDOW_S {
                break;
            }
            self.next_note += 1;
            self.apply_judge(Judge::Miss);
        }
    }

    /// 현재 시각에서 원을 어떤 강도로 채울지 계산합니다.
    ///
    /// 예고 구간은 정확히 `[T - lead, T]` 입니다. 노트 시각에 최고조에 도달한 뒤 바로 꺼집니다.
    fn vibration_fill(&self) -> Intensity {
        self.vibration_fill_with(self.vibration_mode)
    }

    /// 기능 키: 진동 방식을 다음 것으로 넘기고 음성으로 알려 줍니다.
    ///
    /// 연주 도중에도 바로 바뀌므로, 같은 노트를 두 방식으로 번갈아 느껴 볼 수 있습니다.
    /// 음악(효과음 트랙)과 TTS 는 트랙이 달라서 안내 음성이 곡을 끊지 않습니다.
    fn cycle_vibration_mode(&mut self, context: &mut Context) {
        self.vibration_mode = self.vibration_mode.next();

        let label = self.vibration_mode.label(context.language);
        // 시연 중 같은 안내를 반복할 수 있도록 forced 로 보냅니다.
        context
            .audio
            .speak_text_with_option(label, SpeechOption::forced());
        log::info!("진동 방식 전환: {:?}", self.vibration_mode);
    }

    /// 모드를 직접 지정하는 버전입니다. (테스트에서 세 모드를 모두 검증하기 위해 분리)
    fn vibration_fill_with(&self, mode: VibrationMode) -> Intensity {
        let Some(target) = self.upcoming_note() else {
            return Intensity::OFF;
        };
        let lead = self.lead();
        if lead <= 0.0 {
            return Intensity::OFF;
        }

        let (lo, hi) = mode.level_range();
        let level = self.vibration_level.clamp(lo, hi);

        // 창은 **항상 예고 그대로**입니다. 방식이나 단계에 따라 늘리지 않습니다.
        //
        // 점멸의 낮은 단계는 반주기가 예고보다 길어 창 안에 주기가 담기지 않습니다.
        // 1단계(1Hz)는 반주기가 500ms 라 예고 200ms 가 통째로 한 반주기에 들어가고,
        // 펌웨어가 절대 시각 기준으로 위상을 잡으므로(`firmware/src/braille_display.rs`)
        // 어느 반주기에 걸릴지는 노트마다 다릅니다 — **진동이 아예 안 뜨는 노트가 생깁니다.**
        //
        // 그게 그 단계의 실제 한계이므로 숨기지 않고 그대로 보여 줍니다.
        // 창을 늘려 억지로 보이게 하면 단계마다 예고 길이가 달라져 게임이 흔들립니다.
        let window = lead;

        // 진동 전용 시계입니다. 판정 시계(`now_s`)보다 출력 지연만큼 앞서 갑니다.
        // 핀 값을 바꿔도 손끝에 닿기까지 시간이 걸리므로, 그만큼 미리 내보내야
        // 실제로 느껴지는 시점이 음악과 맞습니다.
        let cue_now = self.now_s + TACTILE_OUTPUT_DELAY_MS as f32 / 1000.0;

        // 예고 구간은 정확히 `[T - lead, T]` 입니다.
        // 노트 시각을 지나면 최고조를 유지하지 않고 **즉시 꺼집니다.**
        // 진동이 뚝 끊기는 그 순간이 곧 "지금 누르세요" 신호라, 피크를 손끝으로 집어낼 수 있습니다.
        // (최고조를 판정 끝까지 유지하면 꽉 찬 상태가 220ms 이어져 피크가 뭉개집니다.)
        // 시작과 끝에 서로 다른 시계를 씁니다.
        //
        // * **시작**은 보정된 시계(`cue_now`) 기준 — `now_s = T - 예고 - 보정` 에 켜집니다.
        //   전송·PWM 지연을 타고 손끝에 닿을 때쯤이면 정확히 `T - 예고` 가 됩니다.
        // * **끝**은 판정 시계(`now_s`) 기준 — `now_s = T` 에 꺼집니다.
        //   보정해서 미리 끄면 정작 쳐야 할 순간에 진동이 이미 없습니다.
        //
        // 그래서 켜져 있는 구간은 `[T - 예고 - 보정, T]`, 길이는 `예고 + 보정` 입니다.
        let since_start = cue_now - (target - window);
        if since_start < 0.0 || self.now_s > target {
            return Intensity::OFF;
        }
        // 창 내내 이 레벨 하나로 고정입니다. 올라가거나 내려가지 않습니다.
        // 두 모드의 차이는 "같은 레벨을 무엇으로 표현하느냐" 뿐입니다.
        let value = level_to_value(level);

        match mode {
            // 듀티비 — 약 62.5Hz 반송파의 켜짐 비율을 바꿉니다.
            VibrationMode::DutyRatio => Intensity::new(value),
            // 점멸 주기 — 같은 레벨을 하드웨어 깜빡임 주기로 넘깁니다.
            // 1~6 = 1·2·4·8·16·32Hz, 7 = 항상 켜짐.
            VibrationMode::BlinkPeriod => Intensity::new_blink(value),
        }
    }

    /// 곡 선택 화면에서 **계속** 내보낼 진동입니다. 노트 창과 무관합니다.
    fn vibration_preview(&self) -> Intensity {
        let (lo, hi) = self.vibration_mode.level_range();
        let value = level_to_value(self.vibration_level.clamp(lo, hi));
        match self.vibration_mode {
            VibrationMode::DutyRatio => Intensity::new(value),
            VibrationMode::BlinkPeriod => Intensity::new_blink(value),
        }
    }

    /// A/D 키: 진동 단계를 현재 방식의 범위 안에서 조절하고 음성으로 알려 줍니다.
    fn adjust_vibration_level(&mut self, context: &mut Context, delta: i8) {
        let (lo, hi) = self.vibration_mode.level_range();
        let next = (self.vibration_level.clamp(lo, hi) as i8 + delta).clamp(lo as i8, hi as i8) as u8;
        if next == self.vibration_level {
            return;
        }
        self.vibration_level = next;

        let label = match context.language {
            Language::Ko => format!("{next}단계"),
            Language::Ja => format!("{next}段階"),
            _ => format!("level {next}"),
        };
        context
            .audio
            .speak_text_with_option(&label, SpeechOption::forced());

        match (self.vibration_mode, blink_frequency_hz(next)) {
            (VibrationMode::BlinkPeriod, Some(hz)) => {
                let lead_ms = self.lead() * 1000.0;
                let half_ms = 500.0 / hz;
                // 창이 경계를 하나도 안 넘을 확률 = 통째로 켜지거나 꺼질 확률
                let solid = ((half_ms - lead_ms) / half_ms).max(0.0) * 100.0;
                log::info!(
                    "진동 단계 {next} — 점멸 {hz}Hz(반주기 {half_ms:.0}ms),                      예고 {lead_ms:.0}ms 에 평균 전환 {:.1}회{}",
                    lead_ms / half_ms,
                    if solid > 0.0 {
                        format!(" ← 노트의 약 {solid:.0}% 는 깜빡임 없이 통째로 켜지거나 꺼집니다")
                    } else {
                        String::new()
                    }
                );
            }
            (VibrationMode::BlinkPeriod, None) => {
                log::info!("진동 단계 {next} — 점멸 모드지만 정적입니다 (0=꺼짐, 7=항상 켜짐)")
            }
            (VibrationMode::DutyRatio, _) => log::info!(
                "진동 단계 {next}/{MAX_VIBRATION_LEVEL} — 듀티비, 강도값 {}",
                level_to_value(next)
            ),
        }
    }

    /// 이번 프레임에 그려야 할 내용을 계산합니다.
    fn compute_visual(&self, width: i16) -> Visual {
        match self.state {
            GameState::Playing => {
                let total = self.song().map(|s| s.end_secs()).unwrap_or(1.0).max(0.001);
                Visual {
                    fill: self.vibration_fill(),
                    progress_px: ((self.now_s / total).clamp(0.0, 1.0) * width as f32) as i16,
                    flash: self.now_s < self.flash_until_s,
                }
            }
            _ => Visual::default(),
        }
    }

    /// 화면 중앙 원의 바깥 지름입니다.
    fn circle_diameter(size: Size) -> i16 {
        // 화면에 들어가는 선에서 `VIBRATING_DISC_DIAMETER` 를 씁니다.
        // 그 상수는 전체 프레임 전송 임계값(384핀)과 묶여 있으니 주석을 보세요.
        VIBRATING_DISC_DIAMETER
            .min(size.width.min(size.height) - 4)
            .max(2)
    }

    /// 판정된 입력들의 평균 타이밍 오차(ms). 입력이 없으면 `None`.
    ///
    /// **양수 = 게임 시계보다 늦게 누름.** 플레이어는 귀로 듣는 음악에 맞춰 누르므로,
    /// 이 값은 곧 `context.audio.play()` 호출 시점과 실제로 소리가 나기 시작한 시점의
    /// 차이(= 오디오 출력 지연)에 가깝습니다.
    ///
    /// 따라서 보정값은 부호를 뒤집으면 됩니다: `AUDIO_OFFSET_MS = -(평균 오차)`.
    /// 지연은 기기마다 다르므로(PC 웹 시뮬레이터와 실기가 다릅니다) 각각 재서 맞춰야 합니다.
    fn mean_timing_error_ms(&self) -> Option<i32> {
        if self.timing_error_count == 0 {
            return None;
        }
        let mean_s = self.timing_error_sum / self.timing_error_count as f32;
        Some((mean_s * 1000.0).round() as i32)
    }

    /// 정확도 = 실제로 얻은 점수 / 전부 최고 판정이었을 때의 점수.
    /// 점수표(`HIT_TIERS`)에서 파생되므로 단계를 바꿔도 따로 고칠 필요가 없습니다.
    fn accuracy(&self) -> f32 {
        let judged = self.perfect + self.good + self.miss;
        if judged == 0 {
            return 0.0;
        }
        let best = judged * Judge::best_score();
        self.score as f32 / best as f32
    }
}

impl Applet for RhythmGame {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        self.songs = load_songs();
        self.selected = 0;
        self.state = GameState::SongSelect;
        log::info!(
            "rhythm-game 시작: 곡 {}개, 화면 {:?}, 진동 모드 = {:?} (기능 키로 전환)",
            self.songs.len(),
            context.window.get_size(),
            self.vibration_mode
        );
        Ok(())
    }

    /// 애플릿이 종료될 때(런처 복귀, 전원 키, ExitApp 등) 런타임이 호출합니다.
    /// 음악이 계속 흘러나오지 않도록 여기서 반드시 끊습니다.
    fn on_stop(&mut self, context: &mut Context) -> Result<()> {
        self.stop_music(context);
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let now = context.time.get_monotonic_time();
        let width = context.window.get_size().width;
        let mut needs_redraw = false;

        // --- 입력 ---------------------------------------------------------
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state != KeyState::Pressed {
                continue;
            }

            // --- 진동 조절 키 (왼쪽 키패드, 연주 중에만) ---------------------
            //
            // 웹 시뮬레이터 라벨 기준으로 S / A / D 입니다.
            // (`runtime-web/src/runner.rs` 매핑: S=Down, A=Left, D=Right, 모두 왼쪽)
            //
            // **왼쪽 키패드 = 진동 조절**, 오른쪽 키패드 = 화면 이동으로 나눕니다.
            // 곡 선택 화면에서도 조절할 수 있어야 Blink8 처럼 느긋하게 비교할 수 있고,
            // 곡 고르기는 오른쪽 방향키로 그대로 됩니다. 메뉴 키는 양쪽 다 나가기입니다.
            if event.side == KeypadSide::Left && event.code != KeyCode::Menu {
                match event.code {
                    // S — 진동 방식 전환 (듀티비 ↔ 점멸 주기)
                    KeyCode::Down => {
                        self.cycle_vibration_mode(context);
                        needs_redraw = true;
                        continue;
                    }
                    // A — 단계 줄이기
                    KeyCode::Left => {
                        self.adjust_vibration_level(context, -1);
                        needs_redraw = true;
                        continue;
                    }
                    // D — 단계 늘리기
                    KeyCode::Right => {
                        self.adjust_vibration_level(context, 1);
                        needs_redraw = true;
                        continue;
                    }
                    _ => {}
                }
            }

            match (self.state, event.code) {
                // 곡 선택: 좌/우로 곡을 넘기면 on_speech 가 곡 이름을 읽어 줍니다.
                (GameState::SongSelect, KeyCode::Left | KeyCode::Up) => {
                    if !self.songs.is_empty() {
                        self.selected = (self.selected + self.songs.len() - 1) % self.songs.len();
                        needs_redraw = true;
                    }
                }
                (GameState::SongSelect, KeyCode::Right | KeyCode::Down) => {
                    if !self.songs.is_empty() {
                        self.selected = (self.selected + 1) % self.songs.len();
                        needs_redraw = true;
                    }
                }
                // Enter(가운데 키)로 연주 시작
                (GameState::SongSelect, KeyCode::Center) => {
                    self.start_song(context, now);
                    needs_redraw = true;
                }

                // 연주 중: Space(가운데 키)로 타격
                (GameState::Playing, KeyCode::Center) => {
                    if let Some(t) = self.upcoming_note() {
                        log::info!(
                            "[계측] 타격 now={:.3} 노트={:.3} 오차={:+.0}ms",
                            self.now_s,
                            t,
                            (self.now_s - t) * 1000.0
                        );
                    }
                    self.on_hit();
                    needs_redraw = true;
                }
                // 연주 중 메뉴 키는 연주를 중단하고 곡 선택으로 되돌아갑니다.
                (GameState::Playing, KeyCode::Menu) => {
                    self.stop_music(context);
                    self.state = GameState::SongSelect;
                    needs_redraw = true;
                }

                (GameState::Result, KeyCode::Center) => {
                    self.state = GameState::SongSelect;
                    needs_redraw = true;
                }
                (GameState::Result | GameState::SongSelect, KeyCode::Menu) => {
                    // ExitApp 을 반환해도 런타임이 on_stop 을 불러 주지만,
                    // 종료 경로가 어떻든 음악이 남지 않도록 여기서도 확실히 끊습니다.
                    self.stop_music(context);
                    return Ok(UpdateResult::ExitApp);
                }
                _ => {}
            }
        }

        // --- 시간 진행 ----------------------------------------------------
        if self.state == GameState::Playing {
            self.poll_audio_sync(context, now);

            if !self.sync_locked {
                // 아직 음악이 실제로 나오기 전입니다. 시계를 0 에 묶어 두어
                // 디코딩이 끝나기 전에 노트가 지나가 버리는 일을 막습니다.
                self.now_s = 0.0;
            } else {
                self.now_s = now.saturating_sub(self.started_at).as_secs_f32()
                    + AUDIO_OFFSET_MS as f32 / 1000.0;
                self.reap_missed_notes();
            }

            if self.sync_locked && self.is_finished() {
                // 음악이 자연스럽게 끝난 뒤이므로 따로 끊지 않습니다.
                self.state = GameState::Result;
                needs_redraw = true;
                log::info!(
                    "연주 종료: score={}, max_combo={}, P/G/M = {}/{}/{}",
                    self.score,
                    self.max_combo,
                    self.perfect,
                    self.good,
                    self.miss
                );
                if let Some(err_ms) = self.mean_timing_error_ms() {
                    log::info!(
                        "평균 타이밍 오차 {err_ms}ms (양수=늦게 누름). \
                         현재 AUDIO_OFFSET_MS={AUDIO_OFFSET_MS}, 권장값={}",
                        AUDIO_OFFSET_MS - err_ms as i64
                    );
                }
            }
        }

        // 진동 위상이나 진행 막대가 바뀌었을 때만 다시 그립니다.
        let visual = self.compute_visual(width);
        if visual != self.visual {
            self.visual = visual;
            needs_redraw = true;
        }

        // --- 계측: 진동이 언제 켜지고 꺼지는지 게임 시계 기준으로 기록 ---------
        if self.state == GameState::Playing && self.sync_locked {
            let cur = self.visual.fill.value;
            let target = self.upcoming_note();
            if self.probe_prev_fill == 0 && cur > 0 {
                self.probe_onset_s = Some(self.now_s);
                if let Some(t) = target {
                    log::info!(
                        "[계측] 진동 켜짐 now={:.3} 노트={:.3} 남은시간={:.0}ms (예고={:.0}ms)",
                        self.now_s,
                        t,
                        (t - self.now_s) * 1000.0,
                        self.lead() * 1000.0
                    );
                }
            } else if self.probe_prev_fill > 0 && cur == 0 {
                let onset = self.probe_onset_s.take().unwrap_or(self.now_s);
                log::info!(
                    "[계측] 진동 꺼짐 now={:.3} 지속={:.0}ms",
                    self.now_s,
                    (self.now_s - onset) * 1000.0
                );
            }
            self.probe_prev_fill = cur;
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        let size = canvas.get_size();
        if size.width <= 0 || size.height <= 0 {
            return Ok(());
        }

        let center = Point::new(size.width / 2, size.height / 2);
        let diameter = Self::circle_diameter(size);

        match self.state {
            GameState::SongSelect => {
                // 원을 지금 설정 그대로 **계속** 진동시킵니다. 끄지 않습니다.
                //
                // 연주 중 예고는 길어야 1.1초라 느린 주기는 한 주기도 담기지 않습니다.
                // 여기서는 계속 켜 두므로 S / A·D 로 방식과 단계를 천천히 비교할 수 있습니다.
                // 손가락 올릴 위치를 찾는 역할도 겸합니다.
                // 0단계는 값이 0 이라 아무것도 그리지 않습니다.
                // 대신 테두리 같은 걸 올리면 "진동 0인데 뭔가 뜬다"가 되어 헷갈립니다.
                let intensity = self.vibration_preview();
                if intensity.value > 0 {
                    // `draw_circle` 이 아니라 `fill_circle` 입니다 — blink 보존 때문입니다.
                    fill_circle(canvas, center, diameter, intensity);
                }

                // 곡 개수만큼 점을 찍고 현재 선택된 곡만 강하게 표시합니다.
                let count = self.songs.len().max(1) as i16;
                let gap = 3;
                let total_w = (count - 1) * gap;
                let start_x = center.x - total_w / 2;
                for i in 0..count {
                    let strong = i as usize == self.selected;
                    canvas.set_pin(
                        Point::new(start_x + i * gap, size.height - 2),
                        if strong {
                            Intensity::MAX
                        } else {
                            Intensity::new(80)
                        },
                    );
                }
            }
            GameState::Playing => {
                // 진동하는 안쪽 원. 선형 모드에서는 애플릿이 직접 켜고 끄고,
                // 하드웨어 모드에서는 blink 강도를 그대로 넘겨 장치가 깜빡이게 합니다.
                // 테두리를 따로 그리지 않고 **원 전체를 하나의 균일한 강도**로 채웁니다.
                // 정적인 테두리 + 강해지는 중심 조합은 "가운데만 세진다"는 느낌을 줍니다.
                let fill = self.visual.fill;
                if fill.value > 0 {
                    // `draw_circle` 이 아니라 `fill_circle` 입니다 — blink 보존 때문입니다.
                    fill_circle(canvas, center, Self::circle_diameter(size), fill);
                }

                // 곡 진행 막대 (맨 아랫줄)
                if self.visual.progress_px > 0 {
                    canvas.draw_line(
                        Point::new(0, size.height - 1),
                        Point::new(self.visual.progress_px - 1, size.height - 1),
                        Style::with_stroke(Intensity::new(120), 1),
                    );
                }

                // 판정 직후 표시 (맨 윗줄). Perfect 는 길게, Miss 는 짧게.
                if self.visual.flash
                    && let Some(judge) = self.last_judge
                {
                    let len = (size.width as f32 * judge.flash_ratio()) as i16;
                    canvas.draw_line(
                        Point::new(0, 0),
                        Point::new(len.max(1) - 1, 0),
                        Style::with_stroke(Intensity::MAX, 1),
                    );
                }
            }
            GameState::Result => {
                canvas.draw_circle(center, diameter, Style::with_stroke(Intensity::new(90), 1));
                // 정확도를 가로 막대 길이로 표현합니다.
                let len = (self.accuracy() * size.width as f32) as i16;
                canvas.draw_line(
                    Point::new(0, size.height - 1),
                    Point::new(len.max(1) - 1, size.height - 1),
                    Style::with_stroke(Intensity::MAX, 1),
                );
            }
        }

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        // 반환값이 직전과 달라졌을 때만 실제로 음성이 나갑니다.
        match self.state {
            GameState::SongSelect => {
                let Some(song) = self.song() else {
                    return SpeechResult::text(match context.language {
                        Language::Ko => "재생할 수 있는 곡이 없습니다.",
                        Language::Ja => "再生できる曲がありません。",
                        _ => "No playable songs.",
                    });
                };
                let title = song.title(context.language);
                SpeechResult::text(match context.language {
                    Language::Ko => format!(
                        "{}번, {}. 시작하려면 가운데 키를 누르세요.",
                        self.selected + 1,
                        title
                    ),
                    Language::Ja => format!(
                        "{}番、{}。開始するには中央キーを押してください。",
                        self.selected + 1,
                        title
                    ),
                    _ => format!(
                        "Track {}, {}. Press the center key to start.",
                        self.selected + 1,
                        title
                    ),
                })
            }
            // 연주 중에는 음악과 겹치므로 말하지 않습니다.
            GameState::Playing => SpeechResult::None,
            GameState::Result => SpeechResult::text(match context.language {
                Language::Ko => format!(
                    "연주 종료. 점수 {}점, 정확도 {}퍼센트, 최대 콤보 {}. 퍼펙트 {}, 굿 {}, 미스 {}. 곡 선택으로 돌아가려면 가운데 키를 누르세요.",
                    self.score,
                    (self.accuracy() * 100.0) as u32,
                    self.max_combo,
                    self.perfect,
                    self.good,
                    self.miss
                ),
                Language::Ja => format!(
                    "演奏終了。スコア{}点、正確度{}パーセント、最大コンボ{}。",
                    self.score,
                    (self.accuracy() * 100.0) as u32,
                    self.max_combo
                ),
                _ => format!(
                    "Finished. Score {}, accuracy {} percent, max combo {}. Perfect {}, good {}, miss {}. Press the center key to pick another track.",
                    self.score,
                    (self.accuracy() * 100.0) as u32,
                    self.max_combo,
                    self.perfect,
                    self.good,
                    self.miss
                ),
            }),
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "리듬 게임입니다. 곡 선택 화면에서 좌우 키로 곡을 고르고 가운데 키를 누르면 연주가 시작됩니다. 화면 가운데 원의 진동이 점점 강해지다가 가장 강해지는 순간에 가운데 키를 누르세요. 왼쪽 키패드로 진동을 조절합니다. 아래 키로 방식을 바꾸고, 왼쪽 키와 오른쪽 키로 단계를 0부터 7까지 조절합니다. 두 방식 모두 같은 범위입니다. 곡 선택 화면에서는 원이 그 설정대로 계속 진동하므로 천천히 비교해 볼 수 있습니다. 곡은 오른쪽 방향키로 고릅니다. 메뉴 키를 누르면 연주를 중단하거나 애플릿을 종료합니다."
                .to_string(),
            en: "Rhythm game. On the song select screen use left and right to choose a track, then press the center key to start. The circle in the middle vibrates more and more strongly; press the center key at its peak. The left keypad adjusts the vibration: down switches the style, left and right adjust the level from 0 to 7. Both styles use the same range. On the song select screen the circle keeps vibrating with that setting so you can compare at your own pace. Pick songs with the right arrow keys. Press menu to stop or exit."
                .to_string(),
            ja: "リズムゲームです。曲選択画面で左右キーで曲を選び、中央キーで演奏を開始します。中央の円の振動が徐々に強くなり、最も強くなった瞬間に中央キーを押してください。左キーパッドで振動を調整します。下キーで方式、左キーと右キーで段階を0から7まで調整します。どちらの方式も同じ範囲です。曲選択画面では円がその設定で振動し続けるのでゆっくり比較できます。曲は右の方向キーで選びます。メニューキーで中断または終了します。"
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
    use sdk::api::keypad::KeypadEvent;
    use super::*;
    use sdk::api::display::Canvas;

    /// 진동 전용 시계는 판정 시계(`now_s`)보다 `TACTILE_OUTPUT_DELAY_MS` 만큼 앞섭니다.
    /// 테스트에서 "진동 기준으로 시각 t" 를 만들려면 `now_s` 를 그만큼 되돌려 놓아야 합니다.
    const CUE_ADVANCE_S: f32 = TACTILE_OUTPUT_DELAY_MS as f32 / 1000.0;

    /// 왼쪽 키패드의 키를 한 번 누르고 `on_update` 를 한 바퀴 돌립니다.
    fn press_left(game: &mut RhythmGame, context: &mut Context, code: KeyCode) {
        context.keypad.push_event_front(KeypadEvent {
            code,
            state: KeyState::Pressed,
            side: KeypadSide::Left,
        });
        game.on_update(context).expect("on_update 실패");
    }

    fn set_cue_time(game: &mut RhythmGame, cue_time: f32) {
        game.now_s = cue_time - CUE_ADVANCE_S;
    }

    fn loaded_game() -> RhythmGame {
        let game = RhythmGame {
            songs: load_songs(),
            ..RhythmGame::default()
        };
        assert!(!game.songs.is_empty(), "채보를 하나도 읽지 못했습니다");
        game
    }

    /// 음악이 실제로 시작되기 전에는 시계가 0 에 묶여 있어야 합니다.
    /// 그렇지 않으면 디코딩이 끝나기도 전에 앞부분 노트들이 Miss 로 흘러갑니다.
    #[test]
    fn clock_is_frozen_until_audio_actually_starts() {
        let mut game = loaded_game();
        let mut context = Context::new();

        game.start_song(&mut context, Duration::ZERO);
        assert_eq!(game.state, GameState::Playing);
        assert!(
            !game.sync_locked,
            "아직 재생 위치를 확인하지 못한 상태여야 합니다"
        );

        // 이 시점에 on_update 가 여러 번 돌아도 시간이 흐르면 안 됩니다.
        for _ in 0..5 {
            game.on_update(&mut context).expect("on_update 실패");
        }
        assert_eq!(game.now_s, 0.0, "동기화 전에는 시계가 멈춰 있어야 합니다");
        assert_eq!(game.miss, 0, "동기화 전에 노트가 사라지면 안 됩니다");
        assert_eq!(game.next_note, 0);
    }

    /// audio-service 가 없어도 게임이 영원히 멈춰 있으면 안 됩니다.
    /// 일정 시간이 지나면 요청 시점 기준으로 진행을 시작해야 합니다.
    #[test]
    fn falls_back_when_the_audio_service_never_responds() {
        let mut game = loaded_game();
        let mut context = Context::new();

        game.start_song(&mut context, Duration::ZERO);
        assert!(!game.sync_locked);

        // 타임아웃 직전
        game.poll_audio_sync(
            &mut context,
            Duration::from_millis(SYNC_TIMEOUT_MS) - Duration::from_millis(1),
        );
        assert!(!game.sync_locked, "타임아웃 전에는 기다려야 합니다");

        // 타임아웃 이후
        game.poll_audio_sync(
            &mut context,
            Duration::from_millis(SYNC_TIMEOUT_MS) + Duration::from_millis(1),
        );
        assert!(game.sync_locked, "타임아웃 후에는 진행을 시작해야 합니다");
        assert_eq!(game.started_at, game.play_requested_at);
    }

    /// 판정표 자체가 앞뒤가 맞는지 — 오차는 커지고 점수는 작아지는 순서여야 합니다.
    #[test]
    fn judge_tiers_are_consistent() {
        assert!(!HIT_TIERS.is_empty());
        for pair in HIT_TIERS.windows(2) {
            let (a_judge, a_tol, a_score) = pair[0];
            let (b_judge, b_tol, b_score) = pair[1];
            assert!(
                a_tol < b_tol,
                "{a_judge:?}({a_tol}) 가 {b_judge:?}({b_tol}) 보다 너그럽습니다"
            );
            assert!(
                a_score > b_score,
                "{a_judge:?}({a_score}점) 이 {b_judge:?}({b_score}점) 보다 낮습니다"
            );
        }
        assert!(
            HIT_TIERS.iter().all(|(judge, _, _)| *judge != Judge::Miss),
            "Miss 는 표에 들어가면 안 됩니다"
        );
        assert_eq!(
            HIT_WINDOW_S,
            HIT_TIERS[HIT_TIERS.len() - 1].1,
            "칠 수 있는 한계는 표의 가장 너그러운 값이어야 합니다"
        );
        assert_eq!(Judge::Miss.score(), 0);
        assert_eq!(Judge::Miss.tolerance(), None);
        assert!(!Judge::Miss.keeps_combo());
    }

    /// 각 단계의 경계에서 판정이 정확히 갈리는지 확인합니다.
    #[test]
    fn judge_boundaries() {
        let perfect = Judge::Perfect.tolerance().unwrap();
        let good = Judge::Good.tolerance().unwrap();

        assert_eq!(Judge::from_error(0.0), Some(Judge::Perfect));
        assert_eq!(
            Judge::from_error(perfect),
            Some(Judge::Perfect),
            "경계 포함"
        );
        assert_eq!(Judge::from_error(perfect + 0.001), Some(Judge::Good));
        assert_eq!(Judge::from_error(good), Some(Judge::Good), "경계 포함");
        assert_eq!(
            Judge::from_error(good + 0.001),
            None,
            "한계를 넘으면 판정 없음"
        );
    }

    /// 예전에 있던 사각지대(칠 수 있는 한계 ~ Miss 확정 시각 사이)가 사라졌는지 확인합니다.
    /// 칠 수 있는 경계를 넘긴 노트는 그 즉시 Miss 로 확정되어야 합니다.
    #[test]
    fn no_dead_zone_after_the_hit_window() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];

        // 경계 바로 안쪽: 아직 Miss 가 아니고, 누르면 판정이 됩니다.
        game.now_s = target + HIT_WINDOW_S - 0.001;
        game.reap_missed_notes();
        assert_eq!(game.miss, 0, "아직 칠 수 있는 시점입니다");
        game.on_hit();
        assert_eq!(game.good, 1, "경계 안쪽이므로 Good 이어야 합니다");

        // 다음 노트를 경계 바로 바깥으로 넘겨 봅니다.
        let second = game.song().unwrap().notes[1];
        game.now_s = second + HIT_WINDOW_S + 0.001;
        game.on_hit();
        assert_eq!(game.good, 1, "한계를 넘긴 입력은 무시되어야 합니다");
        game.reap_missed_notes();
        assert_eq!(game.miss, 1, "같은 시점에 Miss 로 확정되어야 합니다");
    }

    /// 정확도가 점수표와 일치해야 합니다.
    #[test]
    fn accuracy_follows_the_score_table() {
        let mut game = loaded_game();

        game.perfect = 10;
        game.score = 10 * Judge::Perfect.score();
        assert!((game.accuracy() - 1.0).abs() < 1e-6, "전부 Perfect 면 100%");

        game = loaded_game();
        game.miss = 10;
        game.score = 0;
        assert!((game.accuracy() - 0.0).abs() < 1e-6, "전부 Miss 면 0%");

        game = loaded_game();
        game.good = 10;
        game.score = 10 * Judge::Good.score();
        let expected = Judge::Good.score() as f32 / Judge::Perfect.score() as f32;
        assert!(
            (game.accuracy() - expected).abs() < 1e-6,
            "전부 Good 이면 {expected}, 실제 {}",
            game.accuracy()
        );
    }

    /// `LEAD_OVERRIDE_MS` 가 채보의 `seconds_per_beat` 보다 우선 적용되어야 합니다.
    #[test]
    fn lead_override_wins_over_the_chart() {
        let game = loaded_game();
        let chart_lead = game.song().unwrap().lead;
        assert_eq!(chart_lead, 0.3, "채보 원본 값은 그대로 보존되어야 합니다");

        match LEAD_OVERRIDE_MS {
            Some(ms) => {
                let expected = ms as f32 / 1000.0;
                assert!(
                    (game.lead() - expected).abs() < 1e-6,
                    "예고 시간이 {expected}초여야 하는데 {}초입니다",
                    game.lead()
                );
                assert!(
                    (0.05..=1.00).contains(&game.lead()),
                    "예고 시간이 상식적인 범위를 벗어났습니다: {}초",
                    game.lead()
                );
            }
            None => assert_eq!(game.lead(), chart_lead),
        }
    }

    /// mp3 헤더에서 실제 재생 길이를 읽어야 합니다.
    /// (파이썬으로 프레임을 세어 확인한 값: 44.1kHz, 2170 프레임, 56.686초)
    #[test]
    fn mp3_duration_is_parsed() {
        let secs = mp3_duration_secs(SONG_ASSETS[0].audio).expect("mp3 길이를 읽지 못했습니다");
        assert!(
            (secs - 56.686).abs() < 0.05,
            "mp3 길이가 예상과 다릅니다: {secs}초"
        );
    }

    /// 마지막 노트(50.311초)가 지나도 음악이 끝나는 56.686초까지는 결과로 넘어가면 안 됩니다.
    #[test]
    fn result_waits_for_the_music_to_end() {
        let mut game = loaded_game();
        game.state = GameState::Playing;

        let last_note = *game.song().unwrap().notes.last().unwrap();
        let end = game.song().unwrap().end_secs();
        assert!(
            end > last_note + 1.0,
            "음원 길이가 아니라 마지막 노트 기준으로 끝나고 있습니다 (end={end}, last={last_note})"
        );

        // 모든 노트를 판정한 상태로 만듭니다.
        game.next_note = game.song().unwrap().notes.len();

        game.now_s = last_note + 0.5;
        assert!(!game.is_finished(), "마지막 노트 직후에 끝나면 안 됩니다");

        game.now_s = end - 0.1;
        assert!(!game.is_finished(), "음악이 아직 남았는데 끝나면 안 됩니다");

        game.now_s = end;
        assert!(game.is_finished(), "음악이 끝나면 결과로 넘어가야 합니다");
    }

    /// 아직 판정할 노트가 남아 있으면 음원이 끝나도 결과로 넘어가지 않습니다.
    #[test]
    fn unjudged_notes_block_the_result_screen() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        game.next_note = 0;
        game.now_s = game.song().unwrap().end_secs() + 5.0;
        assert!(!game.is_finished());
    }

    /// 제공된 JSON 이 기대한 대로 읽히는지 확인합니다.
    #[test]
    fn chart_is_parsed() {
        let game = loaded_game();
        let song = game.song().expect("곡");
        assert_eq!(song.lead, 0.3, "meta.seconds_per_beat 이 예고 시간입니다");
        assert_eq!(song.notes.len(), 36);
        assert!((song.notes[0] - 1.465).abs() < 1e-6);
        // 정렬 보장
        assert!(song.notes.windows(2).all(|w| w[0] <= w[1]));
    }

    /// 예고는 반드시 노트 시각 *이전에* 시작되어야 합니다.
    #[test]
    fn vibration_starts_one_lead_before_the_note() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0]; // 1.465
        let lead = game.lead(); // 0.3

        // 예고 시작 직전에는 원이 비어 있어야 합니다.
        set_cue_time(&mut game, target - lead - 0.01);
        assert_eq!(game.vibration_fill(), Intensity::OFF);

        // 예고 구간 안에서는 한 번이라도 켜지는 순간이 있어야 합니다.
        let mut lit = 0;
        let mut t = target - lead;
        while t < target {
            set_cue_time(&mut game, t);
            if game.vibration_fill().value > 0 {
                lit += 1;
            }
            t += 0.005;
        }
        assert!(lit > 0, "예고 구간에서 진동이 전혀 관측되지 않았습니다");
    }

    /// 기능 키를 누르면 진동 방식이 순환하고, 세 번 누르면 처음으로 돌아와야 합니다.
    /// 실제 `on_update` 경로를 그대로 태워서 키 매핑까지 함께 검증합니다.
    ///
    /// 시뮬레이터 라벨로 S = 왼쪽 Down 입니다.
    #[test]
    fn s_key_cycles_vibration_modes() {
        let mut game = loaded_game();
        let mut context = Context::new();
        game.state = GameState::Playing;
        assert_eq!(game.vibration_mode, VibrationMode::DutyRatio);

        press_left(&mut game, &mut context, KeyCode::Down);
        assert_eq!(game.vibration_mode, VibrationMode::BlinkPeriod);

        press_left(&mut game, &mut context, KeyCode::Down);
        assert_eq!(
            game.vibration_mode,
            VibrationMode::DutyRatio,
            "두 번 누르면 처음 방식으로 돌아와야 합니다"
        );
    }

    /// A = 왼쪽 Left 로 단계를 내리고, D = 왼쪽 Right 로 올립니다.
    /// 단계 번호가 곧 하드웨어 레벨이므로 범위는 1~7 입니다.
    #[test]
    fn a_and_d_adjust_the_vibration_level() {
        let mut game = loaded_game();
        let mut context = Context::new();
        game.state = GameState::Playing;
        assert_eq!(game.vibration_level, DEFAULT_VIBRATION_LEVEL);

        // 최소까지 내립니다.
        for expected in (MIN_VIBRATION_LEVEL..DEFAULT_VIBRATION_LEVEL).rev() {
            press_left(&mut game, &mut context, KeyCode::Left);
            assert_eq!(game.vibration_level, expected);
        }

        // 최소에서 더 눌러도 0 으로 내려가지 않습니다. 0 은 "꺼짐"이라 설정값이 못 됩니다.
        press_left(&mut game, &mut context, KeyCode::Left);
        assert_eq!(game.vibration_level, MIN_VIBRATION_LEVEL);

        // 다시 최대까지 올립니다.
        for expected in (MIN_VIBRATION_LEVEL + 1)..=MAX_VIBRATION_LEVEL {
            press_left(&mut game, &mut context, KeyCode::Right);
            assert_eq!(game.vibration_level, expected);
        }

        // 최대에서 더 눌러도 넘어가지 않습니다.
        press_left(&mut game, &mut context, KeyCode::Right);
        assert_eq!(game.vibration_level, MAX_VIBRATION_LEVEL);
    }

    /// 설정한 단계가 그대로 하드웨어 레벨로 나와야 합니다.
    /// 1단계면 레벨 1, 7단계면 레벨 7 — 예고 내내 고정입니다.
    #[test]
    fn the_step_number_is_the_hardware_level() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        for level in MIN_VIBRATION_LEVEL..=MAX_VIBRATION_LEVEL {
            game.vibration_level = level;

            // 예고 구간 전체를 훑어도 레벨이 변하지 않아야 합니다.
            let mut t = target - lead - CUE_ADVANCE_S;
            while t < target {
                game.now_s = t;
                let v = game.vibration_fill().value;
                let got = (v as u16 * 7 + 127) / 255;
                assert_eq!(
                    got, level as u16,
                    "{level}단계인데 레벨 {got} 이 나왔습니다 (now_s={t})"
                );
                t += 0.005;
            }
        }
    }

    /// 왼쪽 키패드는 **어느 화면에서든** 진동 조절입니다.
    /// 곡 선택 화면에서도 조절돼야 Blink8 처럼 느긋하게 비교할 수 있습니다.
    #[test]
    fn left_keypad_adjusts_vibration_on_the_song_select_screen() {
        let mut game = loaded_game();
        let mut context = Context::new();
        assert_eq!(game.state, GameState::SongSelect);

        game.vibration_level = 4;
        press_left(&mut game, &mut context, KeyCode::Left);
        assert_eq!(game.vibration_level, 3, "곡 선택 화면에서도 단계가 내려가야 합니다");

        press_left(&mut game, &mut context, KeyCode::Right);
        assert_eq!(game.vibration_level, 4, "곡 선택 화면에서도 단계가 올라가야 합니다");

        let before = game.vibration_mode;
        press_left(&mut game, &mut context, KeyCode::Down);
        assert_ne!(game.vibration_mode, before, "곡 선택 화면에서도 방식이 바뀌어야 합니다");
    }

    /// 곡 고르기는 **오른쪽** 방향키로 그대로 되어야 합니다.
    #[test]
    fn the_right_keypad_still_picks_songs() {
        let mut game = loaded_game();
        let mut context = Context::new();
        assert_eq!(game.state, GameState::SongSelect);

        let before = game.selected;
        context.keypad.push_event_front(KeypadEvent {
            code: KeyCode::Right,
            state: KeyState::Pressed,
            side: KeypadSide::Right,
        });
        game.on_update(&mut context).expect("on_update 실패");

        // 곡이 하나뿐이면 제자리로 돌아오므로, 단계가 안 바뀌었는지로 확인합니다.
        assert_eq!(
            game.vibration_level, DEFAULT_VIBRATION_LEVEL,
            "오른쪽 방향키는 진동 단계를 건드리면 안 됩니다"
        );
        assert_eq!(
            game.selected,
            (before + 1) % game.songs.len().max(1),
            "오른쪽 방향키로 곡이 넘어가야 합니다"
        );
    }

    /// **회귀 테스트.** 점멸 모드로 그린 핀은 화면까지 `blink` 를 달고 가야 합니다.
    ///
    /// `graphics` 확장의 `draw_circle`/`draw_rectangle` 은 내부적으로 `Gray8` 을 거치고
    /// `Intensity::from(color.luma())` 로 되돌리기 때문에 `blink` 가 조용히 사라집니다.
    /// 그 경로로 그리면 점멸 모드가 그냥 듀티 모드가 되어, 손끝으로만 알 수 있습니다.
    /// 그래서 `fill_circle` 로 `set_pin` 을 직접 호출합니다.
    #[test]
    fn blink_flag_survives_all_the_way_to_the_canvas() {
        let size = Size::new(48, 32);

        for (state, label) in [
            (GameState::SongSelect, "곡 선택 격자"),
            (GameState::Playing, "연주 중 원"),
        ] {
            let mut game = loaded_game();
            game.state = state;
            game.vibration_mode = VibrationMode::BlinkPeriod;
            game.vibration_level = 4;

            if state == GameState::Playing {
                // 진동이 켜져 있는 순간으로 맞춥니다.
                let target = game.song().unwrap().notes[0];
                game.now_s = target - 0.01;
                game.visual = game.compute_visual(size.width);
                assert!(game.visual.fill.blink, "{label}: 계산 단계에서 이미 blink 가 없습니다");
            }

            let mut canvas = Canvas::new(size);
            game.on_draw(&mut canvas).expect("on_draw 실패");

            // 곡 표시용 점과 곡 진행 막대는 진동이 아니라 정적 표시이므로
            // 맨 아래 두 줄은 검사에서 뺍니다.
            let scan_bottom = size.height - 2;

            let mut lit = 0usize;
            let mut blinking = 0usize;
            for y in 0..scan_bottom {
                for x in 0..size.width {
                    let pin = canvas.get_pin(Point::new(x, y));
                    if pin.value > 0 {
                        lit += 1;
                        if pin.blink {
                            blinking += 1;
                        }
                    }
                }
            }

            assert!(lit > 0, "{label}: 그려진 핀이 없습니다");
            assert_eq!(
                blinking, lit,
                "{label}: 켜진 {lit}핀 중 {blinking}핀만 blink 입니다.                  graphics 확장의 draw_* 를 쓰면 Gray8 을 거치며 blink 가 버려집니다."
            );
        }
    }

    /// 듀티비 모드는 반대로 `blink` 가 붙으면 안 됩니다.
    #[test]
    fn duty_mode_never_sets_the_blink_flag() {
        let size = Size::new(48, 32);
        let mut game = loaded_game();
        game.vibration_mode = VibrationMode::DutyRatio;

        let mut canvas = Canvas::new(size);
        game.on_draw(&mut canvas).expect("on_draw 실패");

        for y in 0..size.height {
            for x in 0..size.width {
                let pin = canvas.get_pin(Point::new(x, y));
                assert!(
                    !pin.blink,
                    "듀티비 모드인데 ({x},{y}) 핀에 blink 가 붙었습니다"
                );
            }
        }
    }

    /// 두 방식 모두 하드웨어 전 범위(0~7)를 쓸 수 있어야 합니다.
    ///
    /// 점멸의 낮은 단계는 예고 창 안에 주기가 담기지 않지만, 확인용으로 막지 않습니다.
    #[test]
    fn both_modes_expose_the_full_level_range() {
        for mode in [VibrationMode::DutyRatio, VibrationMode::BlinkPeriod] {
            assert_eq!(
                mode.level_range(),
                (MIN_VIBRATION_LEVEL, MAX_VIBRATION_LEVEL),
                "{mode:?} 가 전 범위를 쓰지 못합니다"
            );
        }

        let mut game = loaded_game();
        let mut context = Context::new();
        game.vibration_mode = VibrationMode::BlinkPeriod;
        game.vibration_level = MAX_VIBRATION_LEVEL;

        // 0 까지 내려갑니다.
        for expected in (MIN_VIBRATION_LEVEL..MAX_VIBRATION_LEVEL).rev() {
            press_left(&mut game, &mut context, KeyCode::Left);
            assert_eq!(game.vibration_level, expected);
        }
        press_left(&mut game, &mut context, KeyCode::Left);
        assert_eq!(game.vibration_level, MIN_VIBRATION_LEVEL, "0 아래로는 안 내려갑니다");

        // 7 까지 올라갑니다.
        for expected in (MIN_VIBRATION_LEVEL + 1)..=MAX_VIBRATION_LEVEL {
            press_left(&mut game, &mut context, KeyCode::Right);
            assert_eq!(game.vibration_level, expected);
        }
        press_left(&mut game, &mut context, KeyCode::Right);
        assert_eq!(game.vibration_level, MAX_VIBRATION_LEVEL, "7 위로는 안 올라갑니다");
    }

    /// 방식을 바꿔도 단계가 유지되어야 합니다. 두 방식의 범위가 같기 때문입니다.
    #[test]
    fn switching_mode_keeps_the_level() {
        let mut game = loaded_game();
        let mut context = Context::new();
        game.vibration_level = 2;

        press_left(&mut game, &mut context, KeyCode::Down);
        assert_eq!(game.vibration_mode, VibrationMode::BlinkPeriod);
        assert_eq!(game.vibration_level, 2, "방식을 바꿔도 단계는 그대로여야 합니다");

        press_left(&mut game, &mut context, KeyCode::Down);
        assert_eq!(game.vibration_mode, VibrationMode::DutyRatio);
        assert_eq!(game.vibration_level, 2);
    }

    /// 나가기는 **양쪽** Menu 모두에서 되어야 합니다.
    #[test]
    fn either_menu_key_leaves_the_song() {
        for side in [KeypadSide::Left, KeypadSide::Right] {
            let mut game = loaded_game();
            let mut context = Context::new();
            game.state = GameState::Playing;

            context.keypad.push_event_front(KeypadEvent {
                code: KeyCode::Menu,
                state: KeyState::Pressed,
                side,
            });
            game.on_update(&mut context).expect("on_update 실패");

            assert_eq!(
                game.state,
                GameState::SongSelect,
                "{side:?} Menu 로 연주에서 빠져나올 수 있어야 합니다"
            );
        }
    }

    /// 전환된 방식이 실제 렌더링에 반영되는지 확인합니다.
    #[test]
    fn switching_mode_changes_the_pin_behaviour() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        game.now_s = target - game.lead() / 2.0; // 예고 구간 한가운데

        game.vibration_mode = VibrationMode::DutyRatio;
        let swell = game.vibration_fill();
        assert!(
            !swell.blink,
            "Swell 은 PWM 강도라 blink 가 꺼져 있어야 합니다"
        );

        game.vibration_mode = VibrationMode::BlinkPeriod;
        let steps = game.vibration_fill();
        assert!(steps.blink, "HardwareSteps 는 blink 가 켜져 있어야 합니다");
    }

    /// 기본 모드(Swell): 부팅/종료 애니메이션처럼 PWM 강도가 단조 증가해야 합니다.
    /// 깜빡임 플래그가 켜지면 안 됩니다 — 켜지는 순간 느낌이 완전히 달라집니다.
    #[test]
    fn swell_ramps_up_monotonically() {
        assert_eq!(
            RhythmGame::default().vibration_mode,
            VibrationMode::DutyRatio,
            "기본 모드가 바뀌었습니다"
        );

        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        // 누적 오차로 마지막 표본이 노트 시각에 못 미치지 않도록 i/STEPS 로 직접 계산합니다.
        const STEPS: u32 = 32;
        let mut prev = 0u8;
        for i in 0..=STEPS {
            let t = target - lead + lead * (i as f32 / STEPS as f32);
            set_cue_time(&mut game, t);
            let fill = game.vibration_fill();
            assert!(!fill.blink, "Swell 모드는 깜빡임을 쓰지 않아야 합니다");
            assert!(
                fill.value >= prev,
                "강도가 감소했습니다: {prev} -> {} (t={t})",
                fill.value
            );
            prev = fill.value;
        }

        assert_eq!(prev, 255, "노트 시각에는 최대 강도여야 합니다");
    }

    /// 1단계 설정에서는 켜져 있는 내내 세기가 고정이어야 합니다.
    /// 세기가 변하면 그만큼 핀을 다시 보내야 하므로, 전송 예산의 전제가 깨집니다.
    #[test]
    fn swell_holds_a_single_level_while_on() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        let mut seen = std::collections::BTreeSet::new();
        // 켜져 있어야 하는 구간 전체: [T - 예고 - 보정, T]
        let mut t = target - lead - CUE_ADVANCE_S;
        while t < target {
            game.now_s = t;
            let v = game.vibration_fill().value;
            assert!(v > 0, "켜져 있어야 하는 구간인데 꺼졌습니다 (now_s={t})");
            seen.insert((v as u16 * 7 + 127) / 255);
            t += 0.001;
        }

        assert_eq!(
            seen.iter().copied().collect::<Vec<_>>(),
            vec![7],
            "켜져 있는 내내 최대 단계 하나만 나와야 합니다"
        );
    }

    /// 진동의 **시작**은 판정 시계보다 출력 지연만큼 앞서야 합니다.
    /// 그래야 UART·PWM·핀 응답을 거쳐 손끝에 닿는 시점이 `T - 예고` 가 됩니다.
    #[test]
    fn vibration_starts_ahead_by_the_output_delay() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();
        let on_at = target - lead - CUE_ADVANCE_S;

        // 켜지기 직전
        game.now_s = on_at - 0.002;
        assert_eq!(
            game.vibration_fill(),
            Intensity::OFF,
            "`T - 예고 - 보정` 이전에는 꺼져 있어야 합니다"
        );

        // 켜지는 순간
        game.now_s = on_at + 0.002;
        assert_eq!(
            game.vibration_fill().value,
            255,
            "`T - 예고 - 보정` 부터 켜져야 합니다"
        );
    }

    /// 끄는 시점은 보정하지 **않습니다.** 판정 시계로 정확히 `T` 에 꺼집니다.
    ///
    /// 미리 끄면 정작 쳐야 할 순간에 진동이 이미 사라져 있습니다.
    /// 켜져 있는 구간은 `[T - 예고 - 보정, T]`, 길이는 `예고 + 보정` 입니다.
    #[test]
    fn vibration_stops_at_the_note_without_compensation() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        // 노트 직전: 아직 최대 강도
        game.now_s = target - 0.002;
        assert_eq!(
            game.vibration_fill().value,
            255,
            "노트 직전까지는 켜져 있어야 합니다"
        );

        // 노트 시각 직후: 즉시 꺼짐
        game.now_s = target + 0.002;
        assert_eq!(
            game.vibration_fill(),
            Intensity::OFF,
            "노트를 지나면 바로 꺼져야 합니다"
        );

        // 켜져 있던 총 길이 = 예고 + 보정
        let on_at = target - lead - CUE_ADVANCE_S;
        assert!(
            ((target - on_at) - (lead + CUE_ADVANCE_S)).abs() < 1e-6,
            "지속 시간이 예고 + 보정이어야 합니다"
        );

        // 아직 칠 수 있는 구간이지만 진동은 없습니다.
        game.now_s = target + HIT_WINDOW_S / 2.0;
        assert_eq!(game.vibration_fill(), Intensity::OFF);
        assert!(
            Judge::from_error(HIT_WINDOW_S / 2.0).is_some(),
            "진동이 꺼져도 판정은 아직 열려 있어야 합니다"
        );
    }

    /// 듀티비 모드의 각 단계가 실제로 몇 % 듀티가 되는지 고정합니다.
    ///
    /// 애플릿 → 런타임 → 펌웨어의 세 변환을 모두 재현합니다. 어느 한쪽이 바뀌면
    /// 여기서 걸립니다. (사람이 손으로 계산하다 틀리는 걸 막기 위한 테스트입니다)
    #[test]
    fn duty_levels_are_spaced_by_12_5_percent() {
        /// `runtime-native/src/hal/display/mod.rs` 의 `map_intensity_to_4bit`
        fn to_4bit(value: u8) -> u8 {
            ((value as u16 * 7 + 127) / 255) as u8
        }

        /// `firmware/src/braille_display.rs` 의 PWM 판정.
        ///
        /// `counter` 는 `PWM_STEP`(16) 씩 더해지는 `u8` 이라 0,16,…,240 의 16칸입니다.
        /// 코드 1~6 은 `코드 * 32 > counter`, 0 은 항상 꺼짐, 7 은 항상 켜짐입니다.
        fn duty_percent(code: u8) -> f32 {
            match code {
                0 => 0.0,
                7 => 100.0,
                c => {
                    let high = (0..16).filter(|k| (c as u16) * 32 > k * 16).count();
                    high as f32 / 16.0 * 100.0
                }
            }
        }

        let duties: Vec<f32> = (0..=MAX_VIBRATION_LEVEL)
            .map(|level| duty_percent(to_4bit(level_to_value(level))))
            .collect();

        assert_eq!(
            duties,
            vec![0.0, 12.5, 25.0, 37.5, 50.0, 62.5, 75.0, 100.0],
            "듀티 간격이 12.5% 가 아닙니다"
        );

        // 0~6 단계는 정확히 12.5% 씩 올라갑니다.
        for w in duties[..=6].windows(2) {
            assert!(
                (w[1] - w[0] - 12.5).abs() < 1e-3,
                "{w:?} 사이 간격이 12.5% 가 아닙니다"
            );
        }

        // 7단계만 어긋납니다. 펌웨어가 코드 7 을 항상 켜짐으로 특수 처리하기 때문입니다.
        assert_eq!(
            duties[7] - duties[6],
            25.0,
            "7단계는 펌웨어 특수 처리로 75% → 100% 로 건너뜁니다"
        );
    }

    /// 0단계는 **어느 화면에서도** 핀이 하나도 안 떠야 합니다.
    /// (곡 표시용 점과 진행 막대는 진동이 아니므로 제외)
    #[test]
    fn level_zero_lights_no_pins() {
        let size = Size::new(48, 32);

        for mode in [VibrationMode::DutyRatio, VibrationMode::BlinkPeriod] {
            // 곡 선택 화면
            let mut game = loaded_game();
            game.vibration_mode = mode;
            game.vibration_level = 0;

            let mut canvas = Canvas::new(size);
            game.on_draw(&mut canvas).expect("on_draw 실패");
            for y in 0..size.height - 2 {
                for x in 0..size.width {
                    assert_eq!(
                        canvas.get_pin(Point::new(x, y)).value,
                        0,
                        "{mode:?} 0단계인데 곡 선택 화면 ({x},{y}) 에 핀이 떴습니다"
                    );
                }
            }

            // 연주 중, 예고가 한창일 때
            let mut game = loaded_game();
            game.vibration_mode = mode;
            game.vibration_level = 0;
            game.state = GameState::Playing;
            let target = game.song().unwrap().notes[0];
            game.now_s = target - 0.01;
            game.visual = game.compute_visual(size.width);
            assert_eq!(
                game.visual.fill.value, 0,
                "{mode:?} 0단계인데 진동 세기가 0 이 아닙니다"
            );

            let mut canvas = Canvas::new(size);
            game.on_draw(&mut canvas).expect("on_draw 실패");
            for y in 0..size.height - 2 {
                for x in 0..size.width {
                    assert_eq!(
                        canvas.get_pin(Point::new(x, y)).value,
                        0,
                        "{mode:?} 0단계인데 연주 화면 ({x},{y}) 에 핀이 떴습니다"
                    );
                }
            }
        }
    }

    /// 창은 **방식·단계와 무관하게 항상 예고 그대로**여야 합니다.
    ///
    /// 점멸의 낮은 단계는 주기가 창에 안 담겨 노트마다 떴다 안 떴다 하는데,
    /// 그게 그 단계의 실제 한계이므로 창을 늘려 숨기지 않습니다.
    /// 늘리면 단계마다 예고 길이가 달라져 게임 타이밍이 흔들립니다.
    #[test]
    fn the_window_is_always_the_lead_regardless_of_mode_or_level() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();
        let on_at = target - lead - CUE_ADVANCE_S;

        for mode in [VibrationMode::DutyRatio, VibrationMode::BlinkPeriod] {
            // 0단계는 꺼짐이라 켜고 꺼짐을 볼 수 없습니다.
            for level in 1..=MAX_VIBRATION_LEVEL {
                game.vibration_mode = mode;
                game.vibration_level = level;

                game.now_s = on_at - 0.002;
                assert_eq!(
                    game.vibration_fill_with(mode),
                    Intensity::OFF,
                    "{mode:?} {level}단계: 예고 시작 전에는 꺼져 있어야 합니다"
                );

                game.now_s = on_at + 0.002;
                assert!(
                    game.vibration_fill_with(mode).value > 0,
                    "{mode:?} {level}단계: 예고가 시작되면 켜져야 합니다"
                );

                game.now_s = target + 0.002;
                assert_eq!(
                    game.vibration_fill_with(mode),
                    Intensity::OFF,
                    "{mode:?} {level}단계: 노트를 지나면 꺼져야 합니다"
                );
            }
        }
    }

    /// 듀티비 모드는 단계와 무관하게 창이 예고 그대로여야 합니다.
    #[test]
    fn duty_mode_never_widens_the_window() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        // 0단계는 꺼짐이라 켜고 꺼짐을 볼 수 없습니다. 1단계부터 봅니다.
        for level in 1..=MAX_VIBRATION_LEVEL {
            game.vibration_level = level;
            game.now_s = target - lead - CUE_ADVANCE_S - 0.01;
            assert_eq!(
                game.vibration_fill_with(VibrationMode::DutyRatio),
                Intensity::OFF,
                "{level}단계: 예고 시작 전에는 꺼져 있어야 합니다"
            );
            game.now_s = target - lead - CUE_ADVANCE_S + 0.01;
            assert!(
                game.vibration_fill_with(VibrationMode::DutyRatio).value > 0,
                "{level}단계: 예고 시작 후에는 켜져 있어야 합니다"
            );
        }
    }

    /// 점멸 주기 모드는 같은 단계를 깜빡임 플래그로 내보내야 합니다.
    /// 단계가 올라가면 강도값도 단조 증가합니다.
    #[test]
    fn blink_period_mode_maps_each_level_to_a_blink_rate() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        game.now_s = target - game.lead();

        let mut values = Vec::new();
        for level in MIN_VIBRATION_LEVEL..=MAX_VIBRATION_LEVEL {
            game.vibration_level = level;
            let fill = game.vibration_fill_with(VibrationMode::BlinkPeriod);
            assert!(fill.blink, "점멸 모드는 깜빡임 플래그를 써야 합니다");
            values.push(fill.value);
        }

        assert_eq!(values.len(), 8, "0~7 단계 여덟 개여야 합니다");
        assert_eq!(*values.last().unwrap(), 255, "7단계는 최대여야 합니다");
        assert!(
            values.windows(2).all(|w| w[0] < w[1]),
            "단계가 올라가면 강도값도 올라가야 합니다: {values:?}"
        );

        // 듀티비 모드는 같은 단계를 깜빡임 없이 냅니다.
        let duty = game.vibration_fill_with(VibrationMode::DutyRatio);
        assert!(!duty.blink, "듀티비 모드는 깜빡임 플래그를 쓰지 않아야 합니다");
        assert_eq!(duty.value, *values.last().unwrap(), "같은 단계는 같은 강도값");
    }

    #[test]
    fn hit_windows_and_miss() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];

        // 정확히 맞춤 → Perfect
        game.now_s = target;
        game.on_hit();
        assert_eq!(game.perfect, 1);
        assert_eq!(game.combo, 1);
        assert_eq!(game.next_note, 1);

        // 두 번째 노트를 조금 늦게 → Good
        let second = game.song().unwrap().notes[1];
        let perfect_tol = Judge::Perfect.tolerance().unwrap();
        let good_tol = Judge::Good.tolerance().unwrap();
        game.now_s = second + (perfect_tol + good_tol) / 2.0;
        game.on_hit();
        assert_eq!(game.good, 1);
        assert_eq!(game.combo, 2);

        // 세 번째 노트는 그냥 흘려보냄 → Miss, 콤보 초기화
        let third = game.song().unwrap().notes[2];
        game.now_s = third + HIT_WINDOW_S + 0.01;
        game.reap_missed_notes();
        assert_eq!(game.miss, 1);
        assert_eq!(game.combo, 0);
        assert_eq!(game.max_combo, 2);
    }

    /// `cargo test -p rhythm-game -- --nocapture snapshot` 으로 화면을 눈으로 확인합니다.
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

    #[test]
    fn snapshot_screens() {
        let size = Size::new(48, 32);
        let mut game = loaded_game();

        println!("\n=== 곡 선택 ===\n{}", render_ascii(&game, size));

        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        // 진동이 켜져 있는 순간을 찾아서 찍습니다.
        let mut t = target - lead;
        while t < target && game.vibration_fill().value == 0 {
            t += 0.002;
            game.now_s = t;
        }
        game.visual = game.compute_visual(size.width);
        println!(
            "=== 연주 중 (노트 {:.3}s, 현재 {:.3}s, 진동 켜짐) ===\n{}",
            target,
            game.now_s,
            render_ascii(&game, size)
        );

        // 원 테두리는 어느 화면에서나 보여야 합니다.
        let frame = render_ascii(&game, size);
        assert!(
            frame.chars().filter(|&c| c != '.' && c != '\n').count() > 20,
            "원이 그려지지 않았습니다"
        );
    }
}
