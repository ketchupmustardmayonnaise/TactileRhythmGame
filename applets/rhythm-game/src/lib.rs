//! 리듬 게임 애플릿
//!
//! 화면 중앙에 원이 하나 있고, 노트 시각이 다가올수록 이 원의 진동이 점점 강해집니다.
//! 기본값은 기기 부팅/종료 애니메이션과 같은 PWM 강도 램프(`VibrationMode::Swell`)입니다.
//! 진동이 최고조에 달하는 순간(= 노트 시각)에 가운데 키(웹 시뮬레이터의 `Space`)를 누르면 됩니다.
//!
//! 흐름
//! 1. 곡 선택 화면 — 좌/우 키로 곡을 고르면 곡 이름을 음성으로 읽어 줍니다.
//! 2. 가운데 키(`Enter`)를 누르면 음악이 재생되면서 게임이 시작됩니다.
//! 3. 노트마다 `예고 시간` 동안 원이 진동하며, 그 끝에서 가운데 키(`Space`)를 누릅니다.
//! 4. 기능 키(`E`)를 누르면 진동 방식 두 가지가 번갈아 전환되고, 바뀐 방식을 음성으로 알려 줍니다.
//!
//! 채보는 `src/audio/*.json` 에서 읽습니다. `meta.seconds_per_beat` 이 예고 시간이고,
//! 예고는 항상 노트 시각 *이전에* 시작합니다. (노트 1.465초, 예고 0.3초 → 1.165초부터 진동)

use std::time::Duration;

use graphics::{Graphics, style::Style};
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
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
    /// 기기 부팅·종료 애니메이션과 같은 느낌. PWM 강도를 0 → 255 로 부드럽게 끌어올립니다.
    /// (`tactile-display-demo` 의 `Static8`, `firmware/src/shutdown_animation.rs` 의 램프와 동일)
    Swell,
    /// 하드웨어 점멸 8단계에 위임합니다. (`tactile-display-demo` 의 `Blink8` 과 동일)
    HardwareSteps,
}

/// 애플릿을 켰을 때의 기본 방식입니다.
const DEFAULT_VIBRATION_MODE: VibrationMode = VibrationMode::Swell;

impl VibrationMode {
    /// 기능 키를 누를 때마다 다른 방식으로 넘어갑니다.
    fn next(self) -> Self {
        match self {
            VibrationMode::Swell => VibrationMode::HardwareSteps,
            VibrationMode::HardwareSteps => VibrationMode::Swell,
        }
    }

    /// 전환 시 음성으로 읽어 줄 이름입니다. 시연 중 지금 무엇을 만지고 있는지 알려 줍니다.
    fn label(self, lang: Language) -> &'static str {
        match (self, lang) {
            (VibrationMode::Swell, Language::Ko) => {
                "세기 차오름. 부팅 애니메이션과 같은 방식입니다."
            }
            (VibrationMode::HardwareSteps, Language::Ko) => {
                "점멸 8단계. 하드웨어가 직접 깜빡입니다."
            }
            (VibrationMode::Swell, Language::Ja) => {
                "強さの立ち上がり。起動アニメーションと同じ方式です。"
            }
            (VibrationMode::HardwareSteps, Language::Ja) => "点滅8段階。ハードウェアが点滅します。",
            (VibrationMode::Swell, _) => "Intensity swell, same as the boot animation.",
            (VibrationMode::HardwareSteps, _) => "Eight blink steps, driven by the hardware.",
        }
    }
}

/// `HardwareSteps` 에서 예고 시간을 나눌 단계 수입니다.
const VIBRATION_STEPS: i32 = 8;

/// 재생 중인 음악을 끊기 위해 쓰는 아주 짧은 무음 클립입니다.
///
/// audio-service 의 `POST /sound` 핸들러는 새 소리를 재생하기 전에 반드시
/// `audio_player.clear(PlaybackCategory::SoundEffect)` 를 먼저 호출합니다.
/// 별도의 정지 API 가 없으므로, 무음을 한 번 재생해서 재생 중인 곡을 비웁니다.
const SILENCE: &[u8] = include_bytes!("audio/silence.mp3");

/// 음악 재생 요청 ~ 실제 소리가 나기까지의 지연 보정값(ms).
///
/// 정적 오디오는 audio-service 를 거쳐 재생되므로 환경마다 지연이 다릅니다.
/// 박자가 일정하게 밀리거나 당겨지면 이 값을 조정하세요. (양수 = 판정을 늦춤)
const AUDIO_OFFSET_MS: i64 = 0;

/// Perfect 판정 허용 오차(초)
const PERFECT_S: f32 = 0.10;
/// Good 판정 허용 오차(초)
const GOOD_S: f32 = 0.22;
/// 이 시간이 지나도록 누르지 않으면 Miss 처리합니다.
const MISS_S: f32 = 0.30;

/// 예고 시간을 읽지 못했을 때 사용할 기본값(초)
const DEFAULT_LEAD_S: f32 = 0.3;

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
}

impl Song {
    fn title(&self, lang: Language) -> &'static str {
        match lang {
            Language::Ko => self.title[0],
            Language::En => self.title[1],
            Language::Ja => self.title[2],
        }
    }

    /// 곡의 전체 길이(마지막 노트 + 여유)입니다. 진행 막대 계산에 씁니다.
    fn duration(&self) -> f32 {
        self.notes.last().copied().unwrap_or(0.0) + 2.0
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

                    log::info!(
                        "{}: 노트 {}개, 예고 {}초",
                        asset.title[1],
                        notes.len(),
                        lead
                    );

                    Some(Song {
                        title: asset.title,
                        audio: asset.audio,
                        lead,
                        notes,
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
    fn score(self) -> u32 {
        match self {
            Judge::Perfect => 100,
            Judge::Good => 50,
            Judge::Miss => 0,
        }
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
        }
    }
}

impl RhythmGame {
    fn song(&self) -> Option<&Song> {
        self.songs.get(self.selected)
    }

    /// 예고 시간(초)
    fn lead(&self) -> f32 {
        self.song().map(|s| s.lead).unwrap_or(DEFAULT_LEAD_S)
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

        let diff = (target - self.now_s).abs();
        if diff > GOOD_S {
            // 아직 예고조차 시작되지 않았거나 이미 놓친 노트 — 헛손질은 감점 없이 무시합니다.
            return;
        }

        self.next_note += 1;
        self.apply_judge(if diff <= PERFECT_S {
            Judge::Perfect
        } else {
            Judge::Good
        });
    }

    /// 판정 시간을 넘긴 노트를 Miss 처리합니다.
    fn reap_missed_notes(&mut self) {
        while let Some(target) = self.upcoming_note() {
            if self.now_s - target <= MISS_S {
                break;
            }
            self.next_note += 1;
            self.apply_judge(Judge::Miss);
        }
    }

    /// 현재 시각에서 원을 어떤 강도로 채울지 계산합니다.
    ///
    /// 예고 구간은 `[T - lead, T]` 이며, 판정이 끝나는 `T + GOOD_S` 까지 최고조를 유지합니다.
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

        // 예고 시작 이후 경과한 시간
        let since_start = self.now_s - (target - lead);
        if since_start < 0.0 || self.now_s > target + GOOD_S {
            return Intensity::OFF;
        }
        let progress = (since_start / lead).clamp(0.0, 1.0);

        match mode {
            VibrationMode::Swell => {
                // 깜빡임이 아니라 PWM 강도를 0 → 255 로 부드럽게 끌어올립니다.
                // 펌웨어는 이 값을 약 62.5Hz 반송파의 듀티비로 바꾸므로,
                // 손끝에는 진동이 서서히 차오르는 느낌으로 전달됩니다.
                // 노트 시각을 지난 판정 여유 구간에서는 최대치를 유지합니다.
                Intensity::new((progress * 255.0).round() as u8)
            }
            VibrationMode::HardwareSteps => {
                // 예고 시간을 8등분해 하드웨어 점멸 단계를 올립니다.
                // 0 = 항상 꺼짐, 1~6 = 1·2·4·8·16·32Hz, 7 = 항상 켜짐.
                let step =
                    ((progress * VIBRATION_STEPS as f32) as i32).clamp(0, VIBRATION_STEPS - 1);
                Intensity::new_blink((step * 255 / (VIBRATION_STEPS - 1)) as u8)
            }
        }
    }

    /// 이번 프레임에 그려야 할 내용을 계산합니다.
    fn compute_visual(&self, width: i16) -> Visual {
        match self.state {
            GameState::Playing => {
                let total = self.song().map(|s| s.duration()).unwrap_or(1.0).max(0.001);
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
        (size.width.min(size.height) - 8).clamp(6, 40)
    }

    fn accuracy(&self) -> f32 {
        let total = self.perfect + self.good + self.miss;
        if total == 0 {
            0.0
        } else {
            (self.perfect * 2 + self.good) as f32 / (total * 2) as f32
        }
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

            // 기능 키는 어느 화면에서든(연주 도중에도) 진동 방식을 전환합니다.
            if event.code == KeyCode::Function {
                self.cycle_vibration_mode(context);
                needs_redraw = true;
                continue;
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
            self.now_s =
                now.saturating_sub(self.started_at).as_secs_f32() + AUDIO_OFFSET_MS as f32 / 1000.0;
            self.reap_missed_notes();

            let finished = self
                .song()
                .map(|s| self.next_note >= s.notes.len() && self.now_s > s.duration())
                .unwrap_or(true);
            if finished {
                // 채보가 끝나도 mp3 뒷부분이 남아 있을 수 있습니다.
                // 결과 음성과 겹치지 않도록 여기서 음악을 끊습니다.
                self.stop_music(context);
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
            }
        }

        // 진동 위상이나 진행 막대가 바뀌었을 때만 다시 그립니다.
        let visual = self.compute_visual(width);
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

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        let size = canvas.get_size();
        if size.width <= 0 || size.height <= 0 {
            return Ok(());
        }
        let center = Point::new(size.width / 2, size.height / 2);
        let diameter = Self::circle_diameter(size);

        // 원 테두리는 항상 그려서 위치를 손으로 찾을 수 있게 합니다.
        canvas.draw_circle(center, diameter, Style::with_stroke(Intensity::new(90), 1));

        match self.state {
            GameState::SongSelect => {
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
                let fill = self.visual.fill;
                if fill.value > 0 {
                    canvas.draw_circle(center, (diameter - 6).max(2), Style::with_fill(fill));
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
                    let len = match judge {
                        Judge::Perfect => size.width,
                        Judge::Good => size.width / 2,
                        Judge::Miss => size.width / 6,
                    };
                    canvas.draw_line(
                        Point::new(0, 0),
                        Point::new(len.max(1) - 1, 0),
                        Style::with_stroke(Intensity::MAX, 1),
                    );
                }
            }
            GameState::Result => {
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
            ko: "리듬 게임입니다. 곡 선택 화면에서 좌우 키로 곡을 고르고 가운데 키를 누르면 연주가 시작됩니다. 화면 가운데 원의 진동이 점점 강해지다가 가장 강해지는 순간에 가운데 키를 누르세요. 기능 키를 누르면 진동 방식이 두 가지로 번갈아 바뀝니다. 메뉴 키를 누르면 연주를 중단하거나 애플릿을 종료합니다."
                .to_string(),
            en: "Rhythm game. On the song select screen use left and right to choose a track, then press the center key to start. The circle in the middle vibrates more and more strongly; press the center key at its peak. The function key toggles between two vibration styles. Press menu to stop or exit."
                .to_string(),
            ja: "リズムゲームです。曲選択画面で左右キーで曲を選び、中央キーで演奏を開始します。中央の円の振動が徐々に強くなり、最も強くなった瞬間に中央キーを押してください。ファンクションキーで振動方式が2種類に切り替わります。メニューキーで中断または終了します。"
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

    fn loaded_game() -> RhythmGame {
        let game = RhythmGame {
            songs: load_songs(),
            ..RhythmGame::default()
        };
        assert!(!game.songs.is_empty(), "채보를 하나도 읽지 못했습니다");
        game
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
        game.now_s = target - lead - 0.01;
        assert_eq!(game.vibration_fill(), Intensity::OFF);

        // 예고 구간 안에서는 한 번이라도 켜지는 순간이 있어야 합니다.
        let mut lit = 0;
        let mut t = target - lead;
        while t < target {
            game.now_s = t;
            if game.vibration_fill().value > 0 {
                lit += 1;
            }
            t += 0.005;
        }
        assert!(lit > 0, "예고 구간에서 진동이 전혀 관측되지 않았습니다");
    }

    /// 기능 키를 누르면 진동 방식이 순환하고, 세 번 누르면 처음으로 돌아와야 합니다.
    /// 실제 `on_update` 경로를 그대로 태워서 키 매핑까지 함께 검증합니다.
    #[test]
    fn function_key_cycles_vibration_modes() {
        use sdk::api::keypad::{KeypadEvent, KeypadSide};

        let mut game = loaded_game();
        let mut context = Context::new();
        assert_eq!(game.vibration_mode, VibrationMode::Swell);

        let press_function = |game: &mut RhythmGame, context: &mut Context| {
            context.keypad.push_event_front(KeypadEvent {
                code: KeyCode::Function,
                state: KeyState::Pressed,
                side: KeypadSide::Left,
            });
            game.on_update(context).expect("on_update 실패");
        };

        press_function(&mut game, &mut context);
        assert_eq!(game.vibration_mode, VibrationMode::HardwareSteps);

        press_function(&mut game, &mut context);
        assert_eq!(
            game.vibration_mode,
            VibrationMode::Swell,
            "두 번 누르면 처음 방식으로 돌아와야 합니다"
        );
    }

    /// 전환된 방식이 실제 렌더링에 반영되는지 확인합니다.
    #[test]
    fn switching_mode_changes_the_pin_behaviour() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        game.now_s = target - game.lead() / 2.0; // 예고 구간 한가운데

        game.vibration_mode = VibrationMode::Swell;
        let swell = game.vibration_fill();
        assert!(
            !swell.blink,
            "Swell 은 PWM 강도라 blink 가 꺼져 있어야 합니다"
        );

        game.vibration_mode = VibrationMode::HardwareSteps;
        let steps = game.vibration_fill();
        assert!(steps.blink, "HardwareSteps 는 blink 가 켜져 있어야 합니다");
    }

    /// 기본 모드(Swell): 부팅/종료 애니메이션처럼 PWM 강도가 단조 증가해야 합니다.
    /// 깜빡임 플래그가 켜지면 안 됩니다 — 켜지는 순간 느낌이 완전히 달라집니다.
    #[test]
    fn swell_ramps_up_monotonically() {
        assert_eq!(
            RhythmGame::default().vibration_mode,
            VibrationMode::Swell,
            "기본 모드가 바뀌었습니다"
        );

        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        let mut prev = 0u8;
        let mut samples = 0;
        let mut t = target - lead;
        while t <= target {
            game.now_s = t;
            let fill = game.vibration_fill();
            assert!(!fill.blink, "Swell 모드는 깜빡임을 쓰지 않아야 합니다");
            assert!(
                fill.value >= prev,
                "강도가 감소했습니다: {prev} -> {} (t={t})",
                fill.value
            );
            prev = fill.value;
            samples += 1;
            t += lead / 32.0;
        }

        assert!(samples > 8);
        assert_eq!(prev, 255, "노트 시각에는 최대 강도여야 합니다");

        // 판정 여유 구간에서도 최대치를 유지합니다.
        game.now_s = target + GOOD_S / 2.0;
        assert_eq!(game.vibration_fill().value, 255);
    }

    /// HardwareSteps 모드는 예고 시간을 8등분한 깜빡임 단계를 내보내야 합니다.
    #[test]
    fn hardware_steps_uses_eight_blink_levels() {
        let mut game = loaded_game();
        game.state = GameState::Playing;
        let target = game.song().unwrap().notes[0];
        let lead = game.lead();

        let mut levels = Vec::new();
        for i in 0..VIBRATION_STEPS {
            // 각 구간의 한가운데를 샘플링합니다.
            game.now_s = target - lead + lead * (i as f32 + 0.5) / VIBRATION_STEPS as f32;
            let fill = game.vibration_fill_with(VibrationMode::HardwareSteps);
            assert!(fill.blink, "하드웨어 모드는 깜빡임 플래그를 써야 합니다");
            levels.push(fill.value);
        }

        assert_eq!(levels.len(), 8);
        assert_eq!(levels[0], 0, "첫 단계는 꺼짐이어야 합니다");
        assert_eq!(levels[7], 255, "마지막 단계는 최대여야 합니다");
        assert!(
            levels.windows(2).all(|w| w[0] < w[1]),
            "단계가 단조 증가해야 합니다"
        );
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
        game.now_s = second + (PERFECT_S + GOOD_S) / 2.0;
        game.on_hit();
        assert_eq!(game.good, 1);
        assert_eq!(game.combo, 2);

        // 세 번째 노트는 그냥 흘려보냄 → Miss, 콤보 초기화
        let third = game.song().unwrap().notes[2];
        game.now_s = third + MISS_S + 0.01;
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
