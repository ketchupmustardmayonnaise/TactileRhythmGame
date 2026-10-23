//! TactileRhythm — 점자 디스플레이용 2키 촉각 리듬 애플릿
//!
//! 화면 좌우에 큰 원이 하나씩 있고, 노트 시각이 다가오면 해당 쪽 원이 진동합니다.
//! 왼쪽 원은 `Space`, 오른쪽 원은 `Enter` 로 칩니다.
//!
//! 두 가지 모드가 있습니다.
//! * **리듬게임** — 예고(2단계) → 타격 대기(HOLDING). 치면 진동이 끊깁니다.
//! * **감상** — 입력 없이, 노트 시각을 한가운데 두고 전후로 진동만 흘려 보냅니다.
//!
//! 모드·프리셋·곡 변경은 **대기 화면에서만** 됩니다. 연주 중에는 타격 키만 받습니다.
//!
//! 바꾸고 싶은 값은 전부 아래 "설정" 구역에 모아 두었습니다.

use std::time::Duration;

use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadEvent, KeypadPopResult, KeypadSide};
use sdk::applet::{LocalizedString, SpeechOption, SpeechResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;
use serde::Deserialize;

// ===========================================================================
// 설정 — 바꾸고 싶은 것은 전부 여기에 있습니다
// ===========================================================================

/// 키 하나를 좌/우 키패드까지 포함해 가리킵니다.
///
/// 웹 시뮬레이터 라벨과의 대응은 `runtime-web/src/runner.rs` 를 보세요.
/// 왼쪽 키패드: W=Up, S=Down, A=Left, D=Right, Space=Center, Q=Menu, E=Function
/// 오른쪽 키패드: 방향키, Enter=Center
#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    code: KeyCode,
    side: KeypadSide,
}

impl Key {
    const fn new(code: KeyCode, side: KeypadSide) -> Self {
        Self { code, side }
    }
    fn matches(self, event: &KeypadEvent) -> bool {
        event.code == self.code && event.side == self.side
    }
}

// --- 키 배치 ---------------------------------------------------------------

/// 왼쪽 원 타격
const KEY_HIT_LEFT: Key = Key::new(KeyCode::Center, KeypadSide::Left);
/// 오른쪽 원 타격
const KEY_HIT_RIGHT: Key = Key::new(KeyCode::Center, KeypadSide::Right);
/// 모드 전환 (리듬게임 ↔ 감상) — **대기 화면에서만.**
const KEY_MODE: Key = Key::new(KeyCode::Function, KeypadSide::Left);

/// 진동 프리셋 — 누를 때마다 다음 것으로 넘어가고 끝에서 처음으로 돌아옵니다.
/// **대기 화면에서만.**
const KEY_PRESET_NEXT: Key = Key::new(KeyCode::Down, KeypadSide::Left);
/// 다음 곡 — 누를 때마다 다음 곡으로 넘어가고 끝에서 처음으로 돌아옵니다.
/// **대기 화면에서만.**
const KEY_SONG_NEXT: Key = Key::new(KeyCode::Up, KeypadSide::Left);

/// 타격 키와 다른 키가 겹치면 **컴파일이 실패합니다.**
///
/// 겹쳐도 오류가 안 나고 조용히 한쪽이 안 먹히기 때문에, 빌드 때 잡습니다.
const _NO_KEY_CONFLICT: () = {
    let hits = [KEY_HIT_LEFT, KEY_HIT_RIGHT];
    // 타격 키는 `KEY_START` 와 같아도 됩니다. 연주 중에만 타격으로 흘러가기 때문입니다.
    // 반면 아래 키들은 대기 화면에서 타격 키보다 **뒤에** 검사되므로, 겹치면 죽습니다.
    let others = [KEY_MODE, KEY_PRESET_NEXT, KEY_SONG_NEXT];
    let mut i = 0;
    while i < hits.len() {
        let mut j = 0;
        while j < others.len() {
            assert!(
                !(hits[i].code as u8 == others[j].code as u8
                    && hits[i].side as u8 == others[j].side as u8),
                "타격 키가 다른 키와 겹칩니다. 겹친 키는 타격으로만 동작하고 본래 기능은 죽습니다."
            );
            j += 1;
        }
        i += 1;
    }
};

/// 연주 시작 / 결과 화면에서 대기로 돌아가기. **양쪽 키패드의 가운데 키.**
const KEY_START: KeyCode = KeyCode::Center;

/// 연주 중단 / 애플릿 종료. 양쪽 키패드 모두 받습니다.
const KEY_EXIT: KeyCode = KeyCode::Menu;

/// 연주 시작 전 카운트다운에 읽을 문구. 한 칸씩 간격을 두고 하나씩 읽습니다.
const COUNTDOWN: &[&str] = &["3", "2", "1"];

/// 카운트다운 한 칸의 길이(ms)
const COUNTDOWN_STEP_MS: u64 = 1_000;

/// 연주할 때 음악의 **트랙 음량** (0.0 ~ 1.0).
///
/// audio-service 는 `최종 = 샘플 × 트랙음량 × 마스터음량` 으로 섞습니다.
/// 마스터는 기기 설정(소리 크기)이 정하므로, 애플릿은 트랙 음량만 정하면 됩니다.
/// 1.0 = "이 소리를 줄이지 않음" 이고, 기기 설정은 그 위에 그대로 적용됩니다.
///
/// `context.audio.play()` 를 쓰지 않는 이유: 런타임이 **기기 설정값을 트랙 음량으로
/// 한 번 더** 실어 보내서 음량이 제곱으로 작아집니다
/// (`runtime-native/src/host/audio.rs`). 설정이 10% 면 실제로는 1% 가 됩니다.
const MUSIC_VOLUME: f32 = 1.0;

/// 대기 화면에서 곡을 고를 때 배경으로 틀어 줄 **트랙 음량**.
///
/// 어떤 곡인지 알아들을 만큼은 되고, 안내 음성과 연주를 덮지는 않을 정도로 잡습니다.
/// `MUSIC_VOLUME` 보다 작아야 미리듣기가 본 연주보다 커지는 일이 없습니다.
const PREVIEW_VOLUME: f32 = 0.25;

/// audio-service 의 소리 재생 주소
const SOUND_URL: &str = "http://127.0.0.1:3005/sound";

// --- 타이밍 ----------------------------------------------------------------

/// 예고 시간(ms).
///
/// * 리듬게임 — 노트 시각 `t` 기준 `[t - 이 값, t]` 을 **절반으로 갈라** 2단계로 진동합니다.
/// * 감상 — `t` 를 한가운데 두고 `[t - 이 값/2, t + 이 값/2]` 를 2단계로 진동합니다.
///
/// 채보의 `meta.seconds_per_beat` 대신 이 값을 씁니다.
const CUE_MS: u32 = 220;

/// 리듬게임 모드에서 타격을 기다리며 진동을 유지하는 최대 시간(ms).
///
/// 노트 시각부터 시작해, 사용자가 치거나 이 시간이 지나면 꺼집니다.
const HOLD_MAX_MS: u32 = 500;

/// 진동을 **미리 내보내는** 시간(ms).
///
/// 원 하나가 448핀이라 바뀔 때마다 런타임이 전체 프레임(773바이트)을 보내고,
/// 115,200baud 에서 그 전송에 약 67ms 가 걸립니다. 그래서 "이 시각에 손끝이
/// 바뀌었으면" 하는 시점보다 이만큼 앞서 데이터를 띄웁니다.
///
/// **끄는 것은 미리 보내지 않습니다.** 끌 때는 정확히 그 시각에 내보냅니다
/// (`VibrationPlan::value_at` 참고).
const PRE_SEND_MS: u32 = 67;

/// 대기 화면에서 예고 패턴을 한 번 보여 준 뒤, 다음 반복까지 쉬는 시간(ms).
///
/// 쉼 없이 이어 붙이면 패턴이 아니라 그냥 계속 윙윙대는 느낌이 됩니다.
/// `tactile-experiment` 의 대기 화면과 같은 방식입니다.
const STANDBY_PAUSE_MS: u32 = 1_000;

// --- 진동 프리셋 -----------------------------------------------------------
//
// 숫자는 **4비트 전송 코드**입니다. 0~7 은 듀티비(0%, 12.5% … 100%),
// 8~15 는 하드웨어 점멸(8=꺼짐, 9~14=1·2·4·8·16·32Hz, 15=항상 켜짐)입니다.
// `code_to_intensity` 가 이 숫자를 그대로 하드웨어 코드로 보냅니다.

/// 리듬게임 프리셋: `[예고 1단계, 예고 2단계, HOLDING]`
const RHYTHM_PRESETS: &[[u8; 3]] = &[[3, 7, 0], [3, 7, 2], [1, 5, 7]];

/// 감상 프리셋: `[앞 절반, 뒤 절반]`
const LISTEN_PRESETS: &[[u8; 2]] = &[[1, 5], [6, 2]];

// --- 화면 ------------------------------------------------------------------

/// 원의 지름(핀). 화면 절반 너비와 높이에 맞춰 자동으로 줄어듭니다.
///
/// **이 값을 줄이면 안 됩니다.** 원 하나가 384핀을 넘어야 런타임이 차이 전송 대신
/// 전체 프레임을 보내고, 그래야 갱신 시간이 핀 개수와 무관하게 67ms 로 고정됩니다
/// (`runtime-native/src/hal/display/braille_display.rs` 의 `is_full_frame`).
/// 48x32 화면에서 지름 24 면 원 하나가 448핀입니다.
const CIRCLE_DIAMETER: i16 = 24;

// --- 안내 음성 -------------------------------------------------------------

/// 3개 국어 문자열. 음성과 도움말에 씁니다.
struct L10n {
    ko: &'static str,
    en: &'static str,
    ja: &'static str,
}

impl L10n {
    const fn new(ko: &'static str, en: &'static str, ja: &'static str) -> Self {
        Self { ko, en, ja }
    }
    fn get(&self, lang: Language) -> &'static str {
        match lang {
            Language::Ko => self.ko,
            Language::En => self.en,
            Language::Ja => self.ja,
        }
    }
}

const TEXT_MODE_RHYTHM: L10n = L10n::new("리듬 게임 모드", "Rhythm mode", "リズムゲームモード");
const TEXT_MODE_LISTEN: L10n = L10n::new("감상 모드", "Listening mode", "鑑賞モード");
const TEXT_PRESET: L10n = L10n::new("진동 프리셋", "Vibration preset", "振動プリセット");
const TEXT_NO_SONG: L10n = L10n::new(
    "재생할 수 있는 곡이 없습니다.",
    "No songs available.",
    "再生できる曲がありません。",
);

// ===========================================================================
// 설정 끝 — 아래는 구현입니다
// ===========================================================================

// --- 하드웨어로 나가는 값 ---------------------------------------------------

/// 4비트 전송 코드를 `Intensity` 로 바꿉니다.
///
/// 런타임이 `map_intensity_to_4bit` 으로 되돌릴 때 같은 코드가 나오도록 맞춥니다
/// (`runtime-native/src/hal/display/mod.rs`).
/// * 0~7 — 듀티비. `value = 코드 * 255 / 7`
/// * 8~15 — 점멸. 같은 식에 `blink` 플래그를 세웁니다 (런타임이 `8 + 레벨` 로 만듭니다).
fn code_to_intensity(code: u8) -> Intensity {
    let (level, blink) = if code >= 8 { (code - 8, true) } else { (code, false) };
    let value = (level.min(7) as u16 * 255 / 7) as u8;
    if blink {
        Intensity::new_blink(value)
    } else {
        Intensity::new(value)
    }
}

/// `blink` 플래그를 보존하면서 채워진 원을 그립니다.
///
/// **`graphics` 확장의 `draw_circle` 을 쓰면 안 됩니다.** 그 경로는 `embedded-graphics`
/// 의 `Gray8`(8비트 밝기 하나)을 거치고 `Intensity::from(color.luma())` 로 되돌리기
/// 때문에 `blink` 가 조용히 사라집니다 (`extensions/graphics/src/lib.rs`).
fn fill_circle(canvas: &mut dyn DisplayInterface, center: Point, diameter: i16, intensity: Intensity) {
    let top_left_x = center.x - diameter / 2;
    let top_left_y = center.y - diameter / 2;
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

/// 화면에 들어가는 선에서의 원 지름입니다. 좌우 두 개가 겹치지 않아야 합니다.
fn circle_diameter(size: Size) -> i16 {
    CIRCLE_DIAMETER.min(size.width / 2).min(size.height).max(2)
}

// --- 모드 ------------------------------------------------------------------

/// 연주 방식입니다. 대기 화면에서 `KEY_MODE` 로 전환합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayMode {
    /// 예고 2단계 → HOLDING. 사용자가 칩니다.
    Rhythm,
    /// 입력 없이 노트 시각 전후로 진동만 흘려 보냅니다.
    Listen,
}

impl PlayMode {
    fn next(self) -> Self {
        match self {
            PlayMode::Rhythm => PlayMode::Listen,
            PlayMode::Listen => PlayMode::Rhythm,
        }
    }
    fn label(self) -> &'static L10n {
        match self {
            PlayMode::Rhythm => &TEXT_MODE_RHYTHM,
            PlayMode::Listen => &TEXT_MODE_LISTEN,
        }
    }
    /// 이 모드의 프리셋 개수
    fn preset_count(self) -> usize {
        match self {
            PlayMode::Rhythm => RHYTHM_PRESETS.len(),
            PlayMode::Listen => LISTEN_PRESETS.len(),
        }
    }
}

/// 좌우 두 원 중 어느 쪽인지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// 한 쪽 원이 맡은 노트들과 진행 상황입니다. 좌우가 서로 독립적으로 돕니다.
#[derive(Default)]
struct SideState {
    /// 이 쪽이 맡은 노트 시각(초). 오름차순.
    notes: Vec<f32>,
    /// 아직 판정되지 않은 첫 노트의 인덱스
    next: usize,
}

impl SideState {
    fn upcoming(&self) -> Option<f32> {
        self.notes.get(self.next).copied()
    }
    fn reset(&mut self) {
        self.next = 0;
    }
}

// --- 진동 일정 --------------------------------------------------------------

/// 한 노트에 대한 `(렌더링 시각, 4비트 코드)` 일정입니다.
///
/// **렌더링 시각**은 "손끝이 이때 바뀌었으면" 하는 시점입니다.
/// 실제 전송 시각은 `value_at` 이 `PRE_SEND_MS` 만큼 앞당겨 계산합니다.
type Plan = [(f32, u8); 4];

/// 일정과 현재 시각으로 지금 내보낼 코드를 고릅니다.
///
/// **켜는 변화는 미리, 끄는 변화는 제시간에** 내보냅니다.
/// 전송에 67ms 가 걸리므로 켜는 쪽은 그만큼 앞당겨야 제때 손끝에 닿습니다.
/// 반대로 끄는 쪽을 앞당기면 쳐야 할 순간에 진동이 이미 사라져 버립니다.
fn value_at(plan: &Plan, now: f32) -> u8 {
    let pre = PRE_SEND_MS as f32 / 1000.0;
    let mut code = 0;
    for &(at, c) in plan {
        let send_at = if c != 0 { at - pre } else { at };
        if now >= send_at {
            code = c;
        } else {
            break;
        }
    }
    code
}

// --- 오디오 동기화 설정 ------------------------------------------------------

/// 재생 중인 음악을 끊기 위해 쓰는 아주 짧은 무음 클립입니다.
const SILENCE: &[u8] = include_bytes!("audio/silence.mp3");

/// audio-service 의 재생 위치 조회 주소
const POSITION_URL: &str = "http://127.0.0.1:3005/position";
/// 재생이 시작되기 전 폴링 주기(ms)
const SYNC_POLL_MS: u64 = 40;
/// 재생 시작 후 드리프트 확인 주기(ms)
const RESYNC_INTERVAL_MS: u64 = 5_000;
/// 이만큼 어긋났을 때만 다시 맞춥니다(초)
const RESYNC_THRESHOLD_S: f32 = 0.05;
/// audio-service 가 응답하지 않을 때 포기하는 시간(ms)
const SYNC_TIMEOUT_MS: u64 = 5_000;
/// 게임 시계를 통째로 미는 보정값(ms). 판정이 한쪽으로 쏠릴 때만 씁니다.
const AUDIO_OFFSET_MS: i64 = 0;

#[derive(Deserialize)]
struct PositionResponse {
    seconds: f64,
}

/// 예고 시간을 읽지 못했을 때 쓰는 기본값(초)
const DEFAULT_LEAD_S: f32 = 0.3;

// --- 판정 ------------------------------------------------------------------

/// 노트 시각과의 오차(초)에 따른 판정과 점수입니다. 반드시 오름차순이어야 합니다.
///
/// 앞쪽은 이 허용치가 아니라 **예고 시작 시각**이 정합니다 — 진동이 나가기도 전에
/// 누른 것은 느끼고 친 게 아니라 찍은 것이므로 판정하지 않습니다
/// (`TactileRhythm::hit_opens_at`).
const HIT_TIERS: [(Judge, f32, u32); 2] = [(Judge::Perfect, 0.3, 100), (Judge::Good, 0.5, 50)];

/// 노트를 칠 수 있는 마지막 경계(초) = 표의 가장 너그러운 허용 오차.
const HIT_WINDOW_S: f32 = HIT_TIERS[HIT_TIERS.len() - 1].1;

// --- 곡 --------------------------------------------------------------------

struct SongAsset {
    /// [ko, en, ja] 순서의 곡 제목
    title: [&'static str; 3],
    chart_json: &'static str,
    audio: &'static [u8],
}

/// 곡을 추가하려면 `src/audio/` 에 mp3 + json 을 넣고 여기에 한 줄 추가하면 됩니다.
const SONG_ASSETS: &[SongAsset] = &[
    SongAsset {
        title: ["꼬마 눈사람", "Little Snowman", "小さな雪だるま"],
        chart_json: include_str!("audio/little_snowman.json"),
        audio: include_bytes!("audio/little_snowman.mp3"),
    },
    SongAsset {
        title: ["작별", "Old Long Syne", "過ぎ去った日々"],
        chart_json: include_str!("audio/old_long_since.json"),
        audio: include_bytes!("audio/old_long_since.mp3"),
    },
];

#[derive(Deserialize, Default)]
struct ChartMeta {
    #[serde(default)]
    seconds_per_beat: f32,
}

#[derive(Deserialize)]
struct ChartNote {
    time: f32,
    /// 1 = 왼쪽, 2 = 오른쪽. 채보가 한 레인뿐이면 무시하고 번갈아 배정합니다.
    #[serde(default)]
    lane: u8,
}

#[derive(Deserialize)]
struct ChartFile {
    #[serde(default)]
    meta: ChartMeta,
    #[serde(default)]
    notes: Vec<ChartNote>,
}

/// 파싱이 끝난, 애플릿이 실제로 사용하는 곡 정보입니다.
struct Song {
    title: [&'static str; 3],
    audio: &'static [u8],
    /// 채보가 알려 준 예고 시간(초). 지금은 `CUE_MS` 가 우선합니다.
    #[allow(dead_code)]
    lead: f32,
    /// 왼쪽 원이 맡은 노트 시각(초)
    left_notes: Vec<f32>,
    /// 오른쪽 원이 맡은 노트 시각(초)
    right_notes: Vec<f32>,
    /// mp3 헤더에서 계산한 실제 재생 길이(초)
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

    /// 결과 화면으로 넘어가는 시각(초). 음악을 끝까지 들려준 뒤에 넘어갑니다.
    fn end_secs(&self) -> f32 {
        let last = self
            .left_notes
            .last()
            .copied()
            .unwrap_or(0.0)
            .max(self.right_notes.last().copied().unwrap_or(0.0));
        let after_last_note = last + HIT_WINDOW_S;
        match self.audio_secs {
            Some(secs) => secs.max(after_last_note),
            None => after_last_note + 2.0,
        }
    }
}

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

    /// 가장 좋은 판정의 점수. 정확도 계산의 분모로 씁니다.
    fn best_score() -> u32 {
        HIT_TIERS
            .iter()
            .map(|(_, _, score)| *score)
            .max()
            .unwrap_or(1)
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


/// 채보를 읽어 노트를 좌우로 나눕니다.
///
/// 채보에 `lane` 이 둘 이상 있으면 그대로 따릅니다(작은 번호가 왼쪽).
/// 한 레인뿐인 채보(지금 `little_snowman.json` 이 그렇습니다)는 **번갈아** 배정해
/// 2키 모드가 실제로 양손을 쓰게 만듭니다.
fn load_songs() -> Vec<Song> {
    SONG_ASSETS
        .iter()
        .filter_map(
            |asset| match serde_json::from_str::<ChartFile>(asset.chart_json) {
                Ok(chart) => {
                    let lead = if chart.meta.seconds_per_beat > 0.0 {
                        chart.meta.seconds_per_beat
                    } else {
                        DEFAULT_LEAD_S
                    };

                    let mut notes: Vec<(f32, u8)> =
                        chart.notes.iter().map(|n| (n.time, n.lane)).collect();
                    notes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

                    let mut lanes: Vec<u8> = notes.iter().map(|(_, l)| *l).collect();
                    lanes.sort_unstable();
                    lanes.dedup();
                    let multi_lane = lanes.len() >= 2;

                    let (mut left, mut right) = (Vec::new(), Vec::new());
                    for (i, (time, lane)) in notes.iter().enumerate() {
                        let to_left = if multi_lane {
                            *lane == lanes[0]
                        } else {
                            i % 2 == 0
                        };
                        if to_left {
                            left.push(*time);
                        } else {
                            right.push(*time);
                        }
                    }

                    let audio_secs = mp3_duration_secs(asset.audio);
                    log::info!(
                        "{}: 노트 {}개 (왼쪽 {} / 오른쪽 {}), 배정 {}, 음원 {:?}초",
                        asset.title[1],
                        notes.len(),
                        left.len(),
                        right.len(),
                        if multi_lane { "채보 lane" } else { "번갈아" },
                        audio_secs
                    );

                    Some(Song {
                        title: asset.title,
                        audio: asset.audio,
                        lead,
                        left_notes: left,
                        right_notes: right,
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
// 애플릿 상태
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameState {
    /// 대기 화면 — 곡·모드·프리셋을 여기서만 바꿉니다.
    Standby,
    /// 시작 직전 카운트다운 — `COUNTDOWN` 을 한 칸씩 읽습니다.
    Countdown,
    /// 연주 중
    Playing,
    /// 결과 화면
    Result,
}

/// 카운트다운 한 틱의 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountdownStep {
    /// 이번 칸에 들어섰으니 이 문구를 읽어야 합니다.
    Speak(&'static str),
    /// 아직 같은 칸 안입니다. 할 일 없음.
    Wait,
    /// 다 셌습니다. 연주를 시작하세요.
    Done,
}

/// 한 프레임에 실제로 그려질 내용. 달라졌을 때만 다시 그립니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Visual {
    /// 왼쪽 원에 내보낼 4비트 코드
    left: u8,
    /// 오른쪽 원에 내보낼 4비트 코드
    right: u8,
}

pub struct TactileRhythm {
    songs: Vec<Song>,
    selected: usize,
    state: GameState,

    /// 연주 방식 (대기 화면에서만 전환)
    play_mode: PlayMode,
    /// 리듬게임 프리셋 인덱스
    preset_rhythm: usize,
    /// 감상 프리셋 인덱스
    preset_listen: usize,

    left: SideState,
    right: SideState,

    /// 곡 재생을 시작한 모노토닉 시각
    started_at: Duration,
    /// 곡 시작 기준 현재 시간(초)
    now_s: f32,
    /// 대기 화면 미리듣기로 보낸 요청 id. 응답을 버리려고 들고 있습니다.
    preview_request: Option<u32>,

    /// 카운트다운을 시작한 모노토닉 시각
    countdown_at: Duration,
    /// 카운트다운에서 이미 읽은 칸 수
    countdown_spoken: u8,

    /// 모노토닉 시각(초). 곡과 무관하게 계속 흐릅니다.
    /// 대기 화면의 진동 미리보기가 이 시계로 돕니다.
    wall_s: f32,

    visual: Visual,

    score: u32,
    combo: u32,
    max_combo: u32,
    perfect: u32,
    good: u32,
    miss: u32,

    /// 판정된 입력들의 부호 있는 타이밍 오차 합(초). 양수 = 늦게 누름.
    timing_error_sum: f32,
    timing_error_count: u32,

    sync_pending: Option<u32>,
    sync_last_poll: Duration,
    sync_request_at: Duration,
    sync_locked: bool,
    play_requested_at: Duration,
}

impl Default for TactileRhythm {
    fn default() -> Self {
        Self {
            songs: Vec::new(),
            selected: 0,
            state: GameState::Standby,
            play_mode: PlayMode::Rhythm,
            preset_rhythm: 0,
            preset_listen: 0,
            left: SideState::default(),
            right: SideState::default(),
            started_at: Duration::ZERO,
            now_s: 0.0,
            preview_request: None,
            countdown_at: Duration::ZERO,
            countdown_spoken: 0,
            wall_s: 0.0,
            visual: Visual::default(),
            score: 0,
            combo: 0,
            max_combo: 0,
            perfect: 0,
            good: 0,
            miss: 0,
            timing_error_sum: 0.0,
            timing_error_count: 0,
            sync_pending: None,
            sync_last_poll: Duration::ZERO,
            sync_request_at: Duration::ZERO,
            sync_locked: false,
            play_requested_at: Duration::ZERO,
        }
    }
}

impl TactileRhythm {
    fn song(&self) -> Option<&Song> {
        self.songs.get(self.selected)
    }

    fn side(&self, side: Side) -> &SideState {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut SideState {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    /// 예고 시간(초). 두 모드가 같은 값을 쓰지만 의미가 다릅니다.
    /// * 리듬게임 — `[t - cue, t]`
    /// * 감상 — `[t - cue/2, t + cue/2]`
    fn cue_s(&self) -> f32 {
        CUE_MS as f32 / 1000.0
    }

    /// 지금 고른 프리셋을 코드 배열로 돌려줍니다.
    fn preset(&self) -> [u8; 3] {
        match self.play_mode {
            PlayMode::Rhythm => {
                let p = RHYTHM_PRESETS[self.preset_rhythm.min(RHYTHM_PRESETS.len() - 1)];
                [p[0], p[1], p[2]]
            }
            PlayMode::Listen => {
                let p = LISTEN_PRESETS[self.preset_listen.min(LISTEN_PRESETS.len() - 1)];
                [p[0], p[1], 0]
            }
        }
    }

    fn preset_index(&self) -> usize {
        match self.play_mode {
            PlayMode::Rhythm => self.preset_rhythm,
            PlayMode::Listen => self.preset_listen,
        }
    }

    /// 한 노트에 대한 진동 일정을 만듭니다. `(렌더링 시각, 4비트 코드)` 오름차순.
    fn plan_for(&self, t: f32) -> Plan {
        let cue = self.cue_s();
        let p = self.preset();
        match self.play_mode {
            // 예고를 절반으로 갈라 2단계, 그 뒤 HOLDING, 최대 시간이 지나면 꺼짐.
            PlayMode::Rhythm => [
                (t - cue, p[0]),
                (t - cue / 2.0, p[1]),
                (t, p[2]),
                (t + HOLD_MAX_MS as f32 / 1000.0, 0),
            ],
            // 노트 시각을 한가운데 두고 전후 절반씩. HOLDING 없음.
            PlayMode::Listen => [
                (t - cue / 2.0, p[0]),
                (t, p[1]),
                (t + cue / 2.0, 0),
                (t + cue / 2.0, 0),
            ],
        }
    }

    /// 이 쪽 원이 지금 내보낼 4비트 코드입니다.
    fn code_for(&self, side: Side) -> u8 {
        if self.state != GameState::Playing || !self.sync_locked {
            return 0;
        }
        let Some(t) = self.side(side).upcoming() else {
            return 0;
        };
        value_at(&self.plan_for(t), self.now_s)
    }

    /// 이 노트를 칠 수 있게 되는 가장 이른 시각 = **진동이 나가기 시작하는 시점**.
    ///
    /// 허용치(`HIT_TIERS`)가 예고보다 길어도 이 선보다 앞서 누른 것은 판정하지 않습니다.
    /// 아직 아무 신호도 못 느낀 상태이므로 맞춘 게 아니라 찍은 것입니다.
    fn hit_opens_at(&self, target: f32) -> f32 {
        self.plan_for(target)[0].0 - PRE_SEND_MS as f32 / 1000.0
    }

    /// 대기 화면에서 가운데 키를 눌렀을 때. 카운트다운을 시작합니다.
    ///
    /// 음악은 카운트다운이 끝난 뒤에야 `start_song` 에서 시작됩니다.
    fn begin_countdown(&mut self, now: Duration) {
        if self.songs.is_empty() {
            return;
        }
        self.state = GameState::Countdown;
        self.countdown_at = now;
        self.countdown_spoken = 0;
    }

    /// 카운트다운을 한 틱 진행합니다. 시계를 인자로 받아 테스트에서 돌려볼 수 있게 했습니다.
    ///
    /// 칸 경계를 넘을 때마다 `Speak` 를 한 번씩만 돌려줍니다.
    fn countdown_tick(&mut self, now: Duration) -> CountdownStep {
        let elapsed_ms = now.saturating_sub(self.countdown_at).as_millis() as u64;
        let idx = (elapsed_ms / COUNTDOWN_STEP_MS) as usize;

        if idx >= COUNTDOWN.len() {
            return CountdownStep::Done;
        }
        if self.countdown_spoken as usize <= idx {
            self.countdown_spoken = idx as u8 + 1;
            return CountdownStep::Speak(COUNTDOWN[idx]);
        }
        CountdownStep::Wait
    }

    fn start_song(&mut self, context: &mut Context, now: Duration) {
        let Some(song) = self.songs.get(self.selected) else {
            return;
        };
        let audio = song.audio;
        self.left.notes = song.left_notes.clone();
        self.right.notes = song.right_notes.clone();

        Self::post_sound(context, audio, MUSIC_VOLUME);

        self.started_at = now;
        self.play_requested_at = now;
        self.sync_request_at = now;
        self.sync_last_poll = Duration::ZERO;
        self.sync_pending = None;
        self.sync_locked = false;
        self.now_s = 0.0;

        self.left.reset();
        self.right.reset();
        self.score = 0;
        self.combo = 0;
        self.max_combo = 0;
        self.perfect = 0;
        self.good = 0;
        self.miss = 0;
        self.timing_error_sum = 0.0;
        self.timing_error_count = 0;
        self.state = GameState::Playing;

        log::info!(
            "연주 시작: {:?}, 프리셋 {:?}, 예고 {}ms",
            self.play_mode,
            self.preset(),
            CUE_MS
        );
    }

    /// 소리를 보낼 주소. 트랙 음량을 질의 문자열로 싣습니다.
    ///
    /// audio-service 의 `SoundQuery { volume: f32 }` 가 받습니다.
    /// 값이 없으면 서버 기본값 1.0 이 됩니다.
    fn sound_url(volume: f32) -> String {
        format!("{SOUND_URL}?volume={volume}")
    }

    /// audio-service 에 소리를 직접 보냅니다. 성공하면 요청 id 를 돌려줍니다.
    fn post_sound(context: &mut Context, data: &'static [u8], volume: f32) -> Option<u32> {
        let url = Self::sound_url(volume);
        match context.http.fetch_async("POST", &url, &[], data) {
            Ok(id) => Some(id),
            Err(code) => {
                log::warn!("소리 재생 요청 실패 (코드 {code}, 음량 {volume})");
                None
            }
        }
    }

    /// 대기 화면에서 지금 고른 곡을 **작게** 틀어 줍니다.
    ///
    /// `context.audio.play()` 는 시스템 음량을 그대로 쓰므로 쓸 수 없습니다.
    /// audio-service 의 `POST /sound?volume=` 가 재생마다 음량을 받으니 직접 보냅니다.
    /// `/sound` 는 새 소리를 틀기 전에 효과음 트랙을 비우므로, 곡을 연달아 넘겨도
    /// 앞 미리듣기가 겹치지 않습니다.
    fn play_preview(&mut self, context: &mut Context) {
        let Some(song) = self.songs.get(self.selected) else {
            return;
        };
        let audio = song.audio;
        self.preview_request = Self::post_sound(context, audio, PREVIEW_VOLUME);
    }

    fn stop_music(&self, context: &mut Context) {
        // `/sound` 는 새 소리 전에 효과음 트랙을 비우므로, 무음을 한 번 틀면 멈춥니다.
        Self::post_sound(context, SILENCE, MUSIC_VOLUME);
    }

    fn apply_judge(&mut self, judge: Judge) {
        match judge {
            Judge::Perfect => self.perfect += 1,
            Judge::Good => self.good += 1,
            Judge::Miss => self.miss += 1,
        }
        if judge.keeps_combo() {
            self.combo += 1;
            self.max_combo = self.max_combo.max(self.combo);
        } else {
            self.combo = 0;
        }
        self.score += judge.score();
    }

    /// 한 쪽 원의 타격을 처리합니다. 좌우가 서로 독립적입니다.
    fn on_hit(&mut self, side: Side) {
        if self.state != GameState::Playing || self.play_mode != PlayMode::Rhythm {
            return;
        }
        let Some(target) = self.side(side).upcoming() else {
            return;
        };

        // 진동이 나가기도 전에 누른 것은 아예 없던 일로 칩니다.
        // 노트를 소비하지도, 콤보를 끊지도 않습니다.
        if self.now_s < self.hit_opens_at(target) {
            return;
        }

        let signed_error = self.now_s - target;
        let Some(judge) = Judge::from_error(signed_error.abs()) else {
            return;
        };

        self.timing_error_sum += signed_error;
        self.timing_error_count += 1;
        self.side_mut(side).next += 1;
        self.apply_judge(judge);
    }

    /// 지나간 노트를 정리합니다.
    ///
    /// 리듬게임은 허용치를 넘기면 Miss, 감상 모드는 진동 창이 끝나면 조용히 넘깁니다.
    fn reap_notes(&mut self) {
        let cue = self.cue_s();
        for side in [Side::Left, Side::Right] {
            loop {
                let Some(t) = self.side(side).upcoming() else {
                    break;
                };
                match self.play_mode {
                    PlayMode::Rhythm => {
                        if self.now_s - t <= HIT_WINDOW_S {
                            break;
                        }
                        self.side_mut(side).next += 1;
                        self.apply_judge(Judge::Miss);
                    }
                    PlayMode::Listen => {
                        if self.now_s <= t + cue / 2.0 {
                            break;
                        }
                        self.side_mut(side).next += 1;
                    }
                }
            }
        }
    }

    fn is_finished(&self) -> bool {
        match self.song() {
            Some(song) => self.now_s >= song.end_secs(),
            None => true,
        }
    }

    /// 대기 화면에서 두 원에 내보낼 코드입니다.
    ///
    /// `STANDBY_PAUSE_MS` 만큼 쉬고 → 예고 패턴(1단계 → 2단계)을 한 번 보여 주고 →
    /// 다시 쉬기를 반복합니다. 쉼 없이 이어 붙이면 패턴이 아니라 그냥 계속
    /// 윙윙대는 느낌이 되어 두 단계가 구분되지 않습니다.
    ///
    /// 계산은 연주 중과 **같은 경로**(`plan_for` 와 같은 모양의 일정 + `value_at`)를
    /// 탑니다. 그래서 `PRE_SEND_MS` 도 똑같이 적용되고, 여기서 느끼는 것이
    /// 실제 노트에서 느낄 것과 같습니다.
    ///
    /// HOLDING 은 타격에 딸린 것이라 일정에서 빼고 노트 시각에 바로 끕니다.
    fn standby_visual(&self) -> Visual {
        let cue = self.cue_s();
        let pause = STANDBY_PAUSE_MS as f32 / 1000.0;
        let cycle = pause + cue;
        if cycle <= 0.0 {
            return Visual::default();
        }

        // 이번 주기의 가상 노트 시각. 쉬는 시간 뒤에 예고가 꼬박 들어가는 지점입니다.
        let t = cycle;
        let p = self.preset();
        let plan: Plan = [
            (t - cue, p[0]),
            (t - cue / 2.0, p[1]),
            (t, 0),
            (t, 0),
        ];

        let code = value_at(&plan, self.wall_s % cycle);
        Visual {
            left: code,
            right: code,
        }
    }

    fn compute_visual(&self) -> Visual {
        match self.state {
            GameState::Playing => Visual {
                left: self.code_for(Side::Left),
                right: self.code_for(Side::Right),
            },
            GameState::Standby => self.standby_visual(),
            GameState::Countdown | GameState::Result => Visual::default(),
        }
    }

    fn mean_timing_error_ms(&self) -> Option<i32> {
        if self.timing_error_count == 0 {
            return None;
        }
        Some((self.timing_error_sum / self.timing_error_count as f32 * 1000.0).round() as i32)
    }

    fn accuracy(&self) -> f32 {
        let judged = self.perfect + self.good + self.miss;
        if judged == 0 {
            return 0.0;
        }
        self.score as f32 / (judged * Judge::best_score()) as f32 * 100.0
    }

    /// 모드·프리셋·곡을 바꾸고 음성으로 알려 줍니다. 대기 화면에서만 호출됩니다.
    fn announce(&self, context: &mut Context, text: String) {
        context
            .audio
            .speak_text_with_option(&text, SpeechOption::forced());
    }

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

}

impl Applet for TactileRhythm {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        self.songs = load_songs();
        self.selected = 0;
        self.state = GameState::Standby;
        // 열자마자 지금 고른 곡을 작게 틀어 어떤 곡인지 알 수 있게 합니다.
        self.play_preview(context);

        log::info!(
            "TactileRhythm 시작: 곡 {}개, 화면 {:?}, 예고 {}ms, 미리보내기 {}ms",
            self.songs.len(),
            context.window.get_size(),
            CUE_MS,
            PRE_SEND_MS
        );
        Ok(())
    }

    fn on_stop(&mut self, context: &mut Context) -> Result<()> {
        self.stop_music(context);
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let now = context.time.get_monotonic_time();
        self.wall_s = now.as_secs_f32();
        let mut needs_redraw = false;

        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state != KeyState::Pressed {
                continue;
            }

            // 나가기는 어느 화면에서든, 양쪽 키패드 모두 받습니다.
            if event.code == KEY_EXIT {
                self.stop_music(context);
                if self.state == GameState::Playing || self.state == GameState::Countdown {
                    self.state = GameState::Standby;
                    needs_redraw = true;
                    continue;
                }
                return Ok(UpdateResult::ExitApp);
            }

            // 가운데 키 — 양쪽 키패드 모두 받습니다.
            //
            // 대기 화면에서는 카운트다운을 시작하고, 결과 화면에서는 대기로 돌아갑니다.
            // **연주 중에는 여기서 가로채지 않고** 아래 타격 처리로 흘려보냅니다.
            // 같은 키가 화면에 따라 다른 일을 하는 건, 세 화면이 서로 배타적이기 때문입니다.
            if event.code == KEY_START && self.state != GameState::Playing {
                match self.state {
                    GameState::Standby => {
                        // 숫자 읽기가 묻히지 않도록 미리듣기를 먼저 끕니다.
                        self.stop_music(context);
                        self.begin_countdown(now);
                        needs_redraw = true;
                    }
                    GameState::Result => {
                        self.state = GameState::Standby;
                        self.play_preview(context);
                        needs_redraw = true;
                    }
                    _ => {}
                }
                continue;
            }

            // 타격 키. 연주 중에만 먹습니다.
            if KEY_HIT_LEFT.matches(&event) {
                if self.state == GameState::Playing {
                    self.on_hit(Side::Left);
                    needs_redraw = true;
                }
                continue;
            }
            if KEY_HIT_RIGHT.matches(&event) {
                if self.state == GameState::Playing {
                    self.on_hit(Side::Right);
                    needs_redraw = true;
                }
                continue;
            }

            // --- 아래는 전부 대기 화면 전용입니다 -----------------------------
            //
            // 연주 중에 모드나 프리셋이 바뀌면 그 곡의 측정이 통째로 의미를 잃습니다.
            // 그래서 여기서 한 번에 막습니다.
            if self.state != GameState::Standby {
                continue;
            }

            if KEY_MODE.matches(&event) {
                self.play_mode = self.play_mode.next();
                let label = self.play_mode.label().get(context.language).to_string();
                self.announce(context, label);
                log::info!("모드 전환: {:?}", self.play_mode);
                needs_redraw = true;
            } else if KEY_PRESET_NEXT.matches(&event) {
                let count = self.play_mode.preset_count();
                let idx = match self.play_mode {
                    PlayMode::Rhythm => &mut self.preset_rhythm,
                    PlayMode::Listen => &mut self.preset_listen,
                };
                *idx = (*idx + 1) % count;

                let shown = self.preset_index() + 1;
                let preset = self.preset();
                let label = format!("{} {}", TEXT_PRESET.get(context.language), shown);
                self.announce(context, label);
                log::info!(
                    "프리셋 {}/{} = {:?} ({:?})",
                    shown,
                    count,
                    preset,
                    self.play_mode
                );
                needs_redraw = true;
            } else if KEY_SONG_NEXT.matches(&event) {
                if !self.songs.is_empty() {
                    self.selected = (self.selected + 1) % self.songs.len();
                    if let Some(song) = self.songs.get(self.selected) {
                        let title = song.title(context.language).to_string();
                        self.announce(context, title);
                    }
                    // 제목만으로는 어떤 곡인지 모를 수 있으니 작게 틀어 줍니다.
                    self.play_preview(context);
                    needs_redraw = true;
                }
            }
        }

        // 미리듣기 응답은 쓸 데가 없지만, 받아 가지 않으면 런타임에 쌓입니다.
        if let Some(id) = self.preview_request
            && context.http.take_response(id).is_some()
        {
            self.preview_request = None;
        }

        // --- 카운트다운 ------------------------------------------------------
        if self.state == GameState::Countdown {
            match self.countdown_tick(now) {
                CountdownStep::Speak(text) => {
                    context
                        .audio
                        .speak_text_with_option(text, SpeechOption::forced());
                }
                CountdownStep::Done => {
                    self.start_song(context, now);
                    needs_redraw = true;
                }
                CountdownStep::Wait => {}
            }
        }

        // --- 시간 진행 ------------------------------------------------------
        if self.state == GameState::Playing {
            self.poll_audio_sync(context, now);

            if !self.sync_locked {
                // 음악이 아직 안 나왔습니다. 시계를 0 에 묶어 둡니다.
                self.now_s = 0.0;
            } else {
                self.now_s = now.saturating_sub(self.started_at).as_secs_f32()
                    + AUDIO_OFFSET_MS as f32 / 1000.0;
                self.reap_notes();
            }

            if self.sync_locked && self.is_finished() {
                self.state = GameState::Result;
                needs_redraw = true;
                log::info!(
                    "연주 종료: {:?}, score={}, max_combo={}, P/G/M = {}/{}/{}",
                    self.play_mode,
                    self.score,
                    self.max_combo,
                    self.perfect,
                    self.good,
                    self.miss
                );
                if let Some(err_ms) = self.mean_timing_error_ms() {
                    log::info!("평균 타이밍 오차 {err_ms}ms (양수=늦게 누름)");
                }
            }
        }

        let visual = self.compute_visual();
        if visual != self.visual {
            self.visual = visual;
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    /// 좌우 큰 원 두 개만 그립니다.
    ///
    /// 진행 막대 같은 작은 장식을 넣지 않는 이유는 **갱신 시간을 고정하기 위해서**입니다.
    /// 원 하나(448핀)가 바뀌면 런타임이 전체 프레임을 보내 67ms 가 걸리는데,
    /// 작은 요소가 따로 바뀌면 그 사이에 짧은 차이 전송이 끼어들어 타이밍이 흔들립니다.
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        let size = canvas.get_size();
        if size.width <= 0 || size.height <= 0 {
            return Ok(());
        }
        let diameter = circle_diameter(size);
        let cy = size.height / 2;

        for (cx, code) in [
            (size.width / 4, self.visual.left),
            (size.width * 3 / 4, self.visual.right),
        ] {
            let intensity = code_to_intensity(code);
            if intensity.value > 0 {
                fill_circle(canvas, Point::new(cx, cy), diameter, intensity);
            }
        }

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        let lang = context.language;
        match self.state {
            GameState::Standby => {
                let Some(song) = self.song() else {
                    return SpeechResult::text(TEXT_NO_SONG.get(lang).to_string());
                };
                SpeechResult::text(format!(
                    "{}. {}. {} {}.",
                    song.title(lang),
                    self.play_mode.label().get(lang),
                    TEXT_PRESET.get(lang),
                    self.preset_index() + 1
                ))
            }
            // 카운트다운 중에는 숫자 읽기를 방해하지 않도록 조용히 둡니다.
            GameState::Countdown => SpeechResult::None,
            // 연주 중에는 아무 말도 하지 않습니다.
            // 카운트다운 직후에 끼어들어 "삼 이 일 연주 중입니다" 가 되어 버립니다.
            GameState::Playing => SpeechResult::None,
            GameState::Result => SpeechResult::text(match lang {
                Language::Ko => format!(
                    "결과. 정확도 {:.0}퍼센트. 퍼펙트 {}, 굿 {}, 미스 {}. 최대 콤보 {}.",
                    self.accuracy(),
                    self.perfect,
                    self.good,
                    self.miss,
                    self.max_combo
                ),
                Language::Ja => format!(
                    "結果。正確度 {:.0}パーセント。パーフェクト {}、グッド {}、ミス {}。最大コンボ {}。",
                    self.accuracy(),
                    self.perfect,
                    self.good,
                    self.miss,
                    self.max_combo
                ),
                _ => format!(
                    "Result. Accuracy {:.0} percent. Perfect {}, good {}, miss {}. Max combo {}.",
                    self.accuracy(),
                    self.perfect,
                    self.good,
                    self.miss,
                    self.max_combo
                ),
            }),
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "좌우에 원이 하나씩 있습니다. 양쪽 가운데 키로 연주를 시작하면 삼, 이, 일 카운트다운 뒤에 음악이 나옵니다. 왼쪽 원은 왼쪽 가운데 키, 오른쪽 원은 오른쪽 가운데 키로 칩니다. 곡이 끝나면 가운데 키로 대기 화면에 돌아가 다시 할 수 있습니다. 대기 화면에서 오른쪽 좌우 키로 곡을, E로 모드를, W와 S로 진동 프리셋을 바꿉니다. 모드는 리듬 게임과 감상 두 가지입니다. 메뉴 키로 중단하거나 종료합니다."
                .to_string(),
            en: "There is one circle on each side. Press either center key to start; the music begins after a three, two, one countdown. Hit the left circle with the left center key and the right one with the right center key. When the song ends, the center key returns to the standby screen for another go. On the standby screen, the right arrow keys change the song, E switches the mode, and W and S change the vibration preset. The two modes are rhythm game and listening. Press menu to stop or exit."
                .to_string(),
            ja: "左右に円が一つずつあります。両方の中央キーで演奏を開始すると、三、二、一のカウントダウンの後に音楽が流れます。左は左の中央キー、右は右の中央キーで叩きます。曲が終わったら中央キーで待機画面に戻り、もう一度遊べます。待機画面で右の方向キーで曲、Eでモード、WとSで振動プリセットを変更します。モードはリズムゲームと鑑賞の二つです。メニューキーで中断または終了します。"
                .to_string(),
        }
    }
}

/// WASM 런타임 진입점
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(TactileRhythm::default()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::api::display::Canvas;

    const SIZE: Size = Size {
        width: 48,
        height: 32,
    };

    fn game() -> TactileRhythm {
        let mut g = TactileRhythm::default();
        g.songs = load_songs();
        assert!(!g.songs.is_empty(), "채보를 읽지 못했습니다");
        let song = &g.songs[0];
        g.left.notes = song.left_notes.clone();
        g.right.notes = song.right_notes.clone();
        g
    }

    /// 연주 중인 상태로 만들어 둡니다. (시계가 풀려야 진동이 나갑니다)
    fn playing() -> TactileRhythm {
        let mut g = game();
        g.state = GameState::Playing;
        g.sync_locked = true;
        g
    }

    fn press(g: &mut TactileRhythm, ctx: &mut Context, key: Key) {
        ctx.keypad.push_event_front(KeypadEvent {
            code: key.code,
            state: KeyState::Pressed,
            side: key.side,
        });
        g.on_update(ctx).expect("on_update 실패");
    }

    fn lit_pins(g: &TactileRhythm) -> usize {
        let mut canvas = Canvas::new(SIZE);
        g.on_draw(&mut canvas).expect("on_draw 실패");
        (0..SIZE.height)
            .flat_map(|y| (0..SIZE.width).map(move |x| Point::new(x, y)))
            .filter(|&pt| canvas.get_pin(pt).value > 0)
            .count()
    }

    // --- 2키 배치 ----------------------------------------------------------

    /// 원 하나가 바뀔 때마다 **전체 프레임**이 나가야 갱신 시간이 67ms 로 고정됩니다.
    ///
    /// 런타임은 바뀐 핀이 384개 이상일 때만 전체 프레임을 보냅니다
    /// (`runtime-native/src/hal/display/braille_display.rs` 의 `is_full_frame`).
    /// 원이 그보다 작으면 핀 개수에 따라 전송 시간이 들쭉날쭉해집니다.
    #[test]
    fn each_circle_alone_forces_a_full_frame() {
        const FULL_FRAME_THRESHOLD: usize = 48 * 32 / 2 / 2; // 384

        let mut g = playing();
        // 왼쪽만 켠 상태
        g.visual = Visual { left: 7, right: 0 };
        let left_only = lit_pins(&g);
        assert!(
            left_only >= FULL_FRAME_THRESHOLD,
            "원 하나가 {left_only}핀뿐이라 전체 프레임 기준({FULL_FRAME_THRESHOLD})에 못 미칩니다"
        );

        // 오른쪽만 켠 상태도 같아야 합니다.
        g.visual = Visual { left: 0, right: 7 };
        assert_eq!(lit_pins(&g), left_only, "좌우 원의 크기가 달라서는 안 됩니다");

        // 둘 다 켜면 정확히 두 배 — 겹치지 않는다는 뜻입니다.
        g.visual = Visual { left: 7, right: 7 };
        assert_eq!(lit_pins(&g), left_only * 2, "두 원이 겹칩니다");
    }

    /// 왼쪽 원은 Space, 오른쪽 원은 Enter 에만 반응해야 합니다.
    #[test]
    fn each_circle_answers_only_its_own_key() {
        let mut ctx = Context::new();

        // `on_update` 가 키를 처리한 뒤 시계를 다시 쓰므로, 누름마다 새 상태에서 봅니다.
        let mut g = playing();
        let left_note = g.left.notes[0];
        g.now_s = left_note;
        press(&mut g, &mut ctx, KEY_HIT_RIGHT);
        assert_eq!(g.left.next, 0, "오른쪽 키가 왼쪽 노트를 먹었습니다");

        let mut g = playing();
        g.now_s = left_note;
        press(&mut g, &mut ctx, KEY_HIT_LEFT);
        assert_eq!(g.left.next, 1, "왼쪽 키로 왼쪽 노트가 판정되어야 합니다");

        // 반대쪽도 같은 규칙입니다.
        let mut g = playing();
        let right_note = g.right.notes[0];
        g.now_s = right_note;
        press(&mut g, &mut ctx, KEY_HIT_LEFT);
        assert_eq!(g.right.next, 0, "왼쪽 키가 오른쪽 노트를 먹었습니다");

        let mut g = playing();
        g.now_s = right_note;
        press(&mut g, &mut ctx, KEY_HIT_RIGHT);
        assert_eq!(g.right.next, 1, "오른쪽 키로 오른쪽 노트가 판정되어야 합니다");
    }

    /// 채보가 한 레인뿐이면 좌우로 번갈아 배정되어야 합니다.
    #[test]
    fn single_lane_charts_alternate_between_the_two_circles() {
        let g = game();
        let song = &g.songs[0];
        let total = song.left_notes.len() + song.right_notes.len();
        assert!(total > 4, "노트가 너무 적어 확인할 수 없습니다");
        assert!(
            song.left_notes.len().abs_diff(song.right_notes.len()) <= 1,
            "좌우 노트 수가 크게 어긋납니다: {} / {}",
            song.left_notes.len(),
            song.right_notes.len()
        );
        // 번갈아 배정했다면 두 목록을 합쳐 정렬한 것이 원래 순서와 같아야 합니다.
        let mut merged: Vec<f32> = song
            .left_notes
            .iter()
            .chain(song.right_notes.iter())
            .copied()
            .collect();
        merged.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(merged.len(), total);
    }

    // --- 진동 일정 ----------------------------------------------------------

    /// 리듬게임: 예고를 절반으로 갈라 2단계, 그 뒤 HOLDING.
    #[test]
    fn rhythm_plan_has_two_cue_steps_then_holding() {
        let mut g = playing();
        g.play_mode = PlayMode::Rhythm;
        g.preset_rhythm = 2; // [1, 5, 7] — 세 단계가 모두 0 이 아님
        let t = 10.0;
        let cue = g.cue_s();
        let plan = g.plan_for(t);

        assert_eq!(plan[0], (t - cue, 1), "1단계는 예고 시작부터");
        assert_eq!(plan[1], (t - cue / 2.0, 5), "2단계는 예고 한가운데부터");
        assert_eq!(plan[2], (t, 7), "HOLDING 은 노트 시각부터");
        assert_eq!(
            plan[3],
            (t + HOLD_MAX_MS as f32 / 1000.0, 0),
            "HOLDING 은 최대 시간이 지나면 꺼져야 합니다"
        );
    }

    /// 감상: 노트 시각을 한가운데 두고 전후 절반씩, HOLDING 없음.
    #[test]
    fn listen_plan_is_centred_on_the_note() {
        let mut g = playing();
        g.play_mode = PlayMode::Listen;
        g.preset_listen = 0; // [1, 5]
        let t = 10.0;
        let half = g.cue_s() / 2.0;
        let plan = g.plan_for(t);

        assert_eq!(plan[0], (t - half, 1), "앞 절반");
        assert_eq!(plan[1], (t, 5), "뒤 절반은 노트 시각부터");
        assert_eq!(plan[2], (t + half, 0), "창이 끝나면 꺼짐");
        assert_eq!(plan[3].1, 0, "감상 모드에는 HOLDING 이 없습니다");
    }

    /// **켜는 변화는 67ms 미리, 끄는 변화는 제시간에.**
    ///
    /// 사양의 핵심입니다. 전송에 67ms 가 걸리므로 켜는 쪽만 앞당깁니다.
    #[test]
    fn turning_on_is_pre_sent_but_turning_off_is_not() {
        let pre = PRE_SEND_MS as f32 / 1000.0;
        let t = 10.0;
        // (렌더링 시각, 코드)
        let plan: Plan = [(t - 0.22, 3), (t - 0.11, 7), (t, 0), (t + 0.5, 0)];

        // 1단계: t-0.22 에 보이려면 t-0.287 에 나가야 합니다.
        assert_eq!(value_at(&plan, t - 0.22 - pre - 0.001), 0, "너무 이릅니다");
        assert_eq!(value_at(&plan, t - 0.22 - pre + 0.001), 3, "67ms 앞서 나가야 합니다");

        // 2단계: 사양에 적힌 t - (220/2 + 67) 지점입니다.
        assert_eq!(value_at(&plan, t - 0.11 - pre + 0.001), 7);

        // 끄기: 앞당기지 않습니다. t-0.001 에는 아직 7 이어야 합니다.
        assert_eq!(value_at(&plan, t - 0.001), 7, "끄는 것을 미리 보내면 안 됩니다");
        assert_eq!(value_at(&plan, t + 0.001), 0, "끄기는 제시간에");
    }

    /// HOLDING 이 0 인 프리셋([3,7,0])은 노트 시각에 **바로** 꺼져야 합니다.
    /// 0 은 끄는 변화이므로 미리 보내지 않습니다.
    #[test]
    fn zero_holding_turns_off_exactly_at_the_note() {
        let mut g = playing();
        g.play_mode = PlayMode::Rhythm;
        g.preset_rhythm = 0; // [3, 7, 0]
        let t = g.left.notes[0];

        g.now_s = t - 0.001;
        assert_eq!(g.code_for(Side::Left), 7, "노트 직전에는 2단계가 살아 있어야 합니다");
        g.now_s = t + 0.001;
        assert_eq!(g.code_for(Side::Left), 0, "노트 시각에 꺼져야 합니다");
    }

    /// 타격하면 HOLDING 이 즉시 끊겨야 합니다.
    #[test]
    fn hitting_stops_the_holding_vibration() {
        let mut g = playing();
        g.play_mode = PlayMode::Rhythm;
        g.preset_rhythm = 2; // HOLDING = 7
        let t = g.left.notes[0];

        g.now_s = t + 0.05;
        assert_eq!(g.code_for(Side::Left), 7, "타격 전에는 HOLDING 이 돌아야 합니다");

        g.on_hit(Side::Left);
        assert_eq!(g.left.next, 1);
        assert_eq!(g.code_for(Side::Left), 0, "치면 바로 꺼져야 합니다");
    }

    /// 치지 않아도 HOLDING 은 최대 시간에서 멈춰야 합니다.
    #[test]
    fn holding_stops_after_the_maximum() {
        let mut g = playing();
        g.play_mode = PlayMode::Rhythm;
        g.preset_rhythm = 2;
        let t = g.left.notes[0];
        let hold = HOLD_MAX_MS as f32 / 1000.0;

        g.now_s = t + hold - 0.01;
        assert_eq!(g.code_for(Side::Left), 7);
        g.now_s = t + hold + 0.01;
        assert_eq!(g.code_for(Side::Left), 0, "최대 시간이 지나면 꺼져야 합니다");
    }

    /// 진동이 나가기도 전에 누른 것은 판정하지 않습니다.
    #[test]
    fn presses_before_the_cue_goes_out_are_ignored() {
        let mut g = playing();
        g.play_mode = PlayMode::Rhythm;
        let t = g.left.notes[0];
        let opens = g.hit_opens_at(t);

        g.now_s = opens - 0.01;
        g.on_hit(Side::Left);
        assert_eq!(g.left.next, 0, "노트를 소비하면 안 됩니다");
        assert_eq!(g.perfect + g.good + g.miss, 0, "판정이 붙으면 안 됩니다");

        g.now_s = opens + 0.01;
        g.on_hit(Side::Left);
        assert_eq!(g.left.next, 1, "진동이 나간 뒤에는 판정되어야 합니다");
    }

    /// 감상 모드에서는 타격을 받지 않습니다.
    #[test]
    fn listening_mode_ignores_hits() {
        let mut g = playing();
        g.play_mode = PlayMode::Listen;
        g.now_s = g.left.notes[0];
        g.on_hit(Side::Left);
        assert_eq!(g.left.next, 0, "감상 모드는 입력을 받지 않습니다");
        assert_eq!(g.perfect + g.good, 0);
    }

    // --- 모드와 프리셋 -------------------------------------------------------

    /// 모드·프리셋·곡 변경은 **대기 화면에서만** 되어야 합니다.
    #[test]
    fn settings_only_change_on_the_standby_screen() {
        let mut ctx = Context::new();

        // 대기 화면에서는 바뀝니다.
        let mut g = game();
        assert_eq!(g.state, GameState::Standby);
        press(&mut g, &mut ctx, KEY_MODE);
        assert_eq!(g.play_mode, PlayMode::Listen, "대기 화면에서는 모드가 바뀌어야 합니다");
        press(&mut g, &mut ctx, KEY_PRESET_NEXT);
        assert_eq!(g.preset_listen, 1);

        // 연주 중에는 안 바뀝니다.
        let mut g = playing();
        let (mode, preset) = (g.play_mode, g.preset_rhythm);
        press(&mut g, &mut ctx, KEY_MODE);
        press(&mut g, &mut ctx, KEY_PRESET_NEXT);
        press(&mut g, &mut ctx, KEY_SONG_NEXT);
        assert_eq!(g.play_mode, mode, "연주 중에 모드가 바뀌었습니다");
        assert_eq!(g.preset_rhythm, preset, "연주 중에 프리셋이 바뀌었습니다");
    }

    /// 프리셋은 모드마다 따로 기억되고, 끝에서 한 바퀴 돌아야 합니다.
    #[test]
    fn presets_wrap_and_are_remembered_per_mode() {
        let mut ctx = Context::new();
        let mut g = game();

        for expected in 1..RHYTHM_PRESETS.len() {
            press(&mut g, &mut ctx, KEY_PRESET_NEXT);
            assert_eq!(g.preset_rhythm, expected);
        }
        press(&mut g, &mut ctx, KEY_PRESET_NEXT);
        assert_eq!(g.preset_rhythm, 0, "끝에서 처음으로 돌아와야 합니다");

        // 감상 모드로 가도 리듬 프리셋은 그대로 남아 있어야 합니다.
        press(&mut g, &mut ctx, KEY_PRESET_NEXT);
        let kept = g.preset_rhythm;
        assert_eq!(kept, 1);
        press(&mut g, &mut ctx, KEY_MODE);
        assert_eq!(g.play_mode, PlayMode::Listen);
        press(&mut g, &mut ctx, KEY_PRESET_NEXT);
        press(&mut g, &mut ctx, KEY_MODE);
        assert_eq!(g.play_mode, PlayMode::Rhythm);
        assert_eq!(g.preset_rhythm, kept, "모드를 오가면 프리셋이 유지되어야 합니다");
    }

    /// 프리셋 숫자가 그대로 4비트 코드로 나가야 합니다.
    #[test]
    fn preset_numbers_reach_the_wire_unchanged() {
        /// `runtime-native/src/hal/display/mod.rs` 의 `map_intensity_to_4bit`
        fn to_4bit(i: Intensity) -> u8 {
            let level = ((i.value as u16 * 7 + 127) / 255) as u8;
            if i.blink { 8 + level } else { level }
        }
        for code in 0..16u8 {
            assert_eq!(
                to_4bit(code_to_intensity(code)),
                code,
                "코드 {code} 가 전송 과정에서 바뀝니다"
            );
        }
    }

    /// 판정표는 허용치가 오름차순이어야 하고, 경계가 서로 맞물려야 합니다.
    #[test]
    fn judge_tiers_are_consistent() {
        let mut prev = 0.0f32;
        for (judge, tolerance, score) in HIT_TIERS {
            assert!(tolerance > prev, "허용치가 오름차순이어야 합니다: {tolerance}");
            assert!(score > 0, "{judge:?} 의 점수가 0 입니다");
            assert_eq!(
                judge.tolerance(),
                Some(tolerance),
                "{judge:?} 의 허용치 조회가 표와 다릅니다"
            );
            // 경계 바로 안쪽은 이 판정, 바깥쪽은 더 너그러운 판정이거나 없음.
            assert_eq!(Judge::from_error(tolerance - 0.001), Some(judge));
            prev = tolerance;
        }
        assert_eq!(HIT_WINDOW_S, prev, "마지막 허용치가 칠 수 있는 한계여야 합니다");
        assert_eq!(Judge::from_error(prev + 0.001), None, "한계를 넘으면 판정 없음");
        assert_eq!(Judge::Miss.tolerance(), None, "Miss 는 경계가 없습니다");
    }

    /// 대기 화면에서 **양쪽 가운데 키**로 카운트다운이 시작되어야 합니다.
    #[test]
    fn either_center_key_begins_the_countdown() {
        for side in [KeypadSide::Left, KeypadSide::Right] {
            let mut ctx = Context::new();
            let mut g = game();
            assert_eq!(g.state, GameState::Standby);

            press(&mut g, &mut ctx, Key::new(KEY_START, side));
            assert_eq!(
                g.state,
                GameState::Countdown,
                "{side:?} 가운데 키로 카운트다운이 시작되지 않았습니다"
            );
        }
    }

    /// 카운트다운은 한 칸씩 읽고, 다 끝나야 연주가 시작됩니다.
    #[test]
    fn the_countdown_runs_then_starts_the_song() {
        let mut g = game();
        let step = COUNTDOWN_STEP_MS;

        // 카운트다운이 막 시작된 상태로 둡니다.
        // (키로 들어가는 경로는 `either_center_key_begins_the_countdown` 가 봅니다)
        g.state = GameState::Countdown;
        g.countdown_at = Duration::ZERO;
        g.countdown_spoken = 0;

        // 칸을 하나씩 넘기면 그때마다 한 번씩만 읽습니다.
        for i in 0..COUNTDOWN.len() {
            // i 번째 칸 한가운데
            let now = Duration::from_millis(step * i as u64 + step / 2);
            assert_eq!(
                g.countdown_tick(now),
                CountdownStep::Speak(COUNTDOWN[i]),
                "{i}번째 칸에서 읽을 문구가 다릅니다"
            );
            // 같은 칸에서 또 부르면 다시 읽지 않습니다.
            assert_eq!(g.countdown_tick(now), CountdownStep::Wait, "같은 칸을 두 번 읽었습니다");
        }

        // 마지막 칸을 넘기면 끝났다고 알려 줍니다.
        g.countdown_at = Duration::ZERO;
        assert_eq!(
            g.countdown_tick(Duration::from_millis(step * COUNTDOWN.len() as u64 + 10)),
            CountdownStep::Done,
            "카운트다운이 끝나야 합니다"
        );
    }

    /// 카운트다운 중에는 진동이 나가면 안 됩니다.
    #[test]
    fn no_vibration_during_the_countdown() {
        let mut g = game();
        g.state = GameState::Countdown;
        assert_eq!(g.compute_visual(), Visual::default());
    }

    /// 결과 화면에서 가운데 키를 누르면 대기 화면으로 돌아가 새 게임을 할 수 있어야 합니다.
    #[test]
    fn the_center_key_returns_from_the_result_screen() {
        for side in [KeypadSide::Left, KeypadSide::Right] {
            let mut ctx = Context::new();
            let mut g = game();
            g.state = GameState::Result;
            g.score = 1234;

            press(&mut g, &mut ctx, Key::new(KEY_START, side));
            assert_eq!(g.state, GameState::Standby, "{side:?} 로 대기 화면에 못 돌아갔습니다");

            // 거기서 다시 시작할 수 있어야 하고, 점수는 새로 시작할 때 초기화됩니다.
            press(&mut g, &mut ctx, Key::new(KEY_START, side));
            assert_eq!(g.state, GameState::Countdown);
        }
    }

    /// 가운데 키 하나가 화면에 따라 **다른 일**을 해야 합니다.
    ///
    /// 대기에서는 시작, 연주 중에는 타격, 결과에서는 대기로 복귀.
    /// 세 화면이 배타적이라 한 키로 겸할 수 있는데, 입력 처리 순서가 어긋나면
    /// 한쪽이 조용히 죽습니다(전에 실제로 그랬습니다).
    #[test]
    fn the_center_key_means_different_things_per_screen() {
        // 대기 → 카운트다운
        let mut ctx = Context::new();
        let mut g = game();
        press(&mut g, &mut ctx, KEY_HIT_LEFT);
        assert_eq!(g.state, GameState::Countdown, "대기에서 시작이 안 됐습니다");

        // 연주 중 → 타격 (시작으로 가로채이면 안 됩니다)
        let mut g = playing();
        let t = g.left.notes[0];
        g.now_s = t;
        press(&mut g, &mut ctx, KEY_HIT_LEFT);
        assert_eq!(g.state, GameState::Playing, "연주 중에 화면이 바뀌었습니다");
        assert_eq!(g.left.next, 1, "연주 중에 타격이 먹지 않았습니다");

        // 오른쪽도 같은 규칙
        let mut g = playing();
        let t = g.right.notes[0];
        g.now_s = t;
        press(&mut g, &mut ctx, KEY_HIT_RIGHT);
        assert_eq!(g.right.next, 1, "오른쪽 타격이 먹지 않았습니다");

        // 결과 → 대기
        let mut g = game();
        g.state = GameState::Result;
        press(&mut g, &mut ctx, KEY_HIT_RIGHT);
        assert_eq!(g.state, GameState::Standby, "결과에서 대기로 못 돌아갔습니다");
    }

    /// 대기 화면 전용 키들이 타격 키에 가려지지 않아야 합니다.
    ///
    /// `on_update` 가 타격 키를 먼저 검사하므로, 자리가 겹치면 뒤엣것이 영영 안 불립니다.
    /// 빌드 타임 `_NO_KEY_CONFLICT` 와 짝이 되는 실행 검사입니다.
    #[test]
    fn standby_keys_are_not_shadowed_by_the_hit_keys() {
        let mut ctx = Context::new();

        // 곡 변경 키를 눌러도 연주가 시작되면 안 됩니다.
        for key in [KEY_SONG_NEXT, KEY_PRESET_NEXT] {
            let mut g = game();
            press(&mut g, &mut ctx, key);
            assert_eq!(
                g.state,
                GameState::Standby,
                "{:?}/{:?} 가 타격 키에 가려져 연주가 시작됐습니다",
                key.code,
                key.side
            );
        }

        // 모드·프리셋 키도 마찬가지입니다.
        let mut g = game();
        press(&mut g, &mut ctx, KEY_MODE);
        assert_eq!(g.state, GameState::Standby);
        assert_eq!(g.play_mode, PlayMode::Listen, "모드 키가 먹히지 않았습니다");
    }

    /// 연주 중에는 안내 음성이 나오면 안 됩니다.
    /// 카운트다운 직후에 끼어들어 "삼 이 일 연주 중입니다" 가 되어 버립니다.
    #[test]
    fn nothing_is_spoken_while_playing() {
        let ctx = Context::new();
        let mut g = game();
        g.state = GameState::Playing;
        assert!(
            matches!(g.on_speech(&ctx), SpeechResult::None),
            "연주 중에 말을 겁니다"
        );

        // 카운트다운 중에도 조용해야 합니다.
        g.state = GameState::Countdown;
        assert!(matches!(g.on_speech(&ctx), SpeechResult::None));

        // 대기·결과 화면은 반대로 말을 해야 합니다.
        g.state = GameState::Standby;
        assert!(!matches!(g.on_speech(&ctx), SpeechResult::None));
        g.state = GameState::Result;
        assert!(!matches!(g.on_speech(&ctx), SpeechResult::None));
    }

    /// 미리듣기는 **낮은 음량**으로 나가야 합니다.
    ///
    /// `context.audio.play()` 는 시스템 음량을 쓰므로 쓸 수 없고,
    /// audio-service 의 `?volume=` 를 직접 실어 보냅니다. 값이 빠지면
    /// 서버 기본값 1.0 이 되어 곡이 **원래 크기로** 터져 나옵니다.
    #[test]
    fn the_preview_carries_a_quiet_volume() {
        for v in [MUSIC_VOLUME, PREVIEW_VOLUME] {
            let url = TactileRhythm::sound_url(v);
            assert!(url.starts_with(SOUND_URL), "주소가 /sound 가 아닙니다: {url}");
            assert!(
                url.contains(&format!("volume={v}")),
                "음량이 주소에 실리지 않았습니다: {url}"
            );
        }

        // 미리듣기는 본 연주보다 작아야 합니다.
        assert!(
            PREVIEW_VOLUME < MUSIC_VOLUME,
            "미리듣기({PREVIEW_VOLUME})가 연주({MUSIC_VOLUME})보다 큽니다"
        );
        assert!((0.0..=1.0).contains(&PREVIEW_VOLUME));
        assert!((0.0..=1.0).contains(&MUSIC_VOLUME));

        // 연주는 트랙 음량을 줄이지 않아야 합니다.
        // 기기 설정(마스터)이 그 위에 적용되므로, 여기서 또 줄이면 제곱으로 작아집니다.
        assert_eq!(MUSIC_VOLUME, 1.0, "연주 트랙 음량은 1.0 이어야 합니다");
    }

    /// 곡을 넘기면 선택이 바뀌고, 곡이 없을 때 미리듣기가 터지지 않아야 합니다.
    #[test]
    fn changing_the_song_is_safe_without_songs() {
        let mut ctx = Context::new();
        let mut g = game();
        let before = g.selected;
        press(&mut g, &mut ctx, KEY_SONG_NEXT);
        assert_ne!(g.selected, before, "곡이 안 넘어갔습니다");

        // 곡 목록이 비어 있어도 조용히 넘어가야 합니다.
        let mut empty = TactileRhythm::default();
        empty.play_preview(&mut ctx);
        assert!(empty.preview_request.is_none());
    }

    // --- 화면 ---------------------------------------------------------------

    /// 연주 중에는 두 원 말고 아무것도 그리지 않아야 합니다.
    /// 작은 장식이 따로 바뀌면 짧은 차이 전송이 끼어들어 67ms 고정이 깨집니다.
    #[test]
    fn nothing_but_the_two_circles_is_drawn_while_playing() {
        let mut g = playing();
        g.visual = Visual { left: 0, right: 0 };
        assert_eq!(lit_pins(&g), 0, "진동이 없으면 아무것도 뜨면 안 됩니다");
    }

    /// 대기 화면은 **쉬고 → 예고 패턴 한 번 → 쉬고** 를 반복해야 합니다.
    /// 쉼 없이 이어 붙이면 두 단계가 구분되지 않습니다.
    #[test]
    fn standby_plays_the_cue_pattern_with_a_rest_between_repeats() {
        let mut g = game();
        g.play_mode = PlayMode::Rhythm;
        g.preset_rhythm = 2; // [1, 5, 7]
        let cue = g.cue_s();
        let pause = STANDBY_PAUSE_MS as f32 / 1000.0;
        let pre = PRE_SEND_MS as f32 / 1000.0;
        let cycle = pause + cue;

        let at = |g: &mut TactileRhythm, t: f32| {
            g.wall_s = t;
            g.compute_visual().left
        };

        // 쉬는 구간 — 아무것도 안 나갑니다.
        assert_eq!(at(&mut g, 0.0), 0, "주기 시작은 쉬는 구간입니다");
        assert_eq!(at(&mut g, pause - pre - 0.01), 0, "아직 쉬는 중이어야 합니다");

        // 1단계는 전송 시간만큼 앞당겨 나갑니다.
        assert_eq!(at(&mut g, pause - pre + 0.01), 1, "1단계가 미리 나가야 합니다");
        // 2단계도 같은 규칙.
        assert_eq!(at(&mut g, pause + cue / 2.0 - pre + 0.01), 5, "2단계");
        // 노트 시각(= 주기 끝)에 꺼집니다. 끄기는 앞당기지 않습니다.
        assert_eq!(at(&mut g, cycle - 0.01), 5, "끄기를 미리 보내면 안 됩니다");

        // 다음 주기도 같은 모양으로 되풀이됩니다.
        assert_eq!(at(&mut g, cycle + 0.01), 0, "다음 주기는 다시 쉬는 구간");
        assert_eq!(at(&mut g, cycle * 3.0 + pause - pre + 0.01), 1);

        // HOLDING 은 대기 화면에 나오지 않습니다.
        let holding = RHYTHM_PRESETS[2][2];
        for i in 0..200 {
            let t = cycle * i as f32 / 100.0;
            assert_ne!(at(&mut g, t), holding, "HOLDING 이 대기 화면에 나왔습니다 (t={t})");
        }

        // 한 주기에서 쉬는 비중이 절반을 넘어야 "패턴"으로 느껴집니다.
        let lit = (0..1000)
            .filter(|i| at(&mut g, cycle * *i as f32 / 1000.0) != 0)
            .count();
        assert!(lit < 500, "쉬는 시간이 너무 짧습니다: 켜진 비율 {}%", lit / 10);
    }

    /// 감상 모드도 같은 방식으로 두 단계를 보여 줍니다.
    #[test]
    fn standby_previews_listening_mode_too() {
        let mut g = game();
        g.play_mode = PlayMode::Listen;
        g.preset_listen = 1; // [6, 2]
        let cue = g.cue_s();
        let pause = STANDBY_PAUSE_MS as f32 / 1000.0;
        let pre = PRE_SEND_MS as f32 / 1000.0;

        g.wall_s = pause - pre + 0.01;
        assert_eq!(g.compute_visual(), Visual { left: 6, right: 6 });
        g.wall_s = pause + cue / 2.0 - pre + 0.01;
        assert_eq!(g.compute_visual(), Visual { left: 2, right: 2 });
    }

    /// 연주가 시작되기 전(시계가 안 풀린 상태)에는 진동이 나가면 안 됩니다.
    #[test]
    fn no_vibration_before_the_music_actually_starts() {
        let mut g = game();
        g.state = GameState::Playing;
        g.sync_locked = false;
        g.now_s = 0.0;
        assert_eq!(g.code_for(Side::Left), 0);
        assert_eq!(g.code_for(Side::Right), 0);
    }
}

#[cfg(test)]
mod song_assets_check {
    use super::*;

    /// 곡마다 **자기 음원과 자기 채보**가 물려 있어야 합니다.
    ///
    /// `SONG_ASSETS` 에 줄을 복사해 넣다 보면 `include_bytes!` 만 그대로 두기 쉽습니다.
    /// 그러면 채보는 B 곡인데 음악은 A 곡이 나오고, 오류는 안 납니다.
    #[test]
    fn every_song_has_its_own_audio_and_chart() {
        let songs = load_songs();
        assert_eq!(songs.len(), SONG_ASSETS.len(), "채보 파싱에 실패한 곡이 있습니다");

        for (i, a) in SONG_ASSETS.iter().enumerate() {
            for (j, b) in SONG_ASSETS.iter().enumerate() {
                if i >= j {
                    continue;
                }
                assert!(
                    !std::ptr::eq(a.audio, b.audio),
                    "{} 와 {} 가 같은 음원을 가리킵니다",
                    a.title[1],
                    b.title[1]
                );
                assert!(
                    !std::ptr::eq(a.chart_json, b.chart_json),
                    "{} 와 {} 가 같은 채보를 가리킵니다",
                    a.title[1],
                    b.title[1]
                );
            }
        }

        // 음원 길이와 마지막 노트가 얼추 맞는지 — 엉뚱한 음원이 물리면 여기서 걸립니다.
        for (song, asset) in songs.iter().zip(SONG_ASSETS) {
            let last = song
                .left_notes
                .last()
                .copied()
                .unwrap_or(0.0)
                .max(song.right_notes.last().copied().unwrap_or(0.0));
            let secs = song.audio_secs.expect("mp3 길이를 읽지 못했습니다");
            println!(
                "{:<16} 음원 {:>6.1}초, 마지막 노트 {:>6.1}초",
                asset.title[1], secs, last
            );
            assert!(
                last <= secs + 1.0,
                "{}: 마지막 노트({last:.1}초)가 음원({secs:.1}초)보다 뒤입니다 — 음원이 잘못 물렸을 수 있습니다",
                asset.title[1]
            );
        }
    }
}
