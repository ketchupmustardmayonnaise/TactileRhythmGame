// applets/demo-app/src/lib.rs

use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, BoundsRect, Size};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::applet::Applet;
use sdk::event::UpdateResult;
use sdk::tts_segment;
use sdk::{Language, Result};

use widget::item_selector::{ItemGrid, LayoutStrategy, SelectorBehavior};
use widget::core::Widget;

/// 화면 동작 모드를 나타냅니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 8단계 일반 그라데이션 (Static)
    Static8,
    /// 8단계 깜빡임 그라데이션 (Blink)
    Blink8,
}

impl Mode {
    /// 해당 모드에서 사용할 상자(격자) 개수
    pub fn total_steps(&self) -> usize {
        8
    }

    /// 음성 출력 가이드용 세그먼트 리스트 반환
    pub fn to_segments(&self, lang: Language) -> Vec<AudioSegment> {
        let text = match lang {
            Language::Ko => match self {
                Mode::Static8 => "8단계 일반",
                Mode::Blink8 => "8단계 깜빡임",
            },
            Language::En => match self {
                Mode::Static8 => "Level 8 Static",
                Mode::Blink8 => "Level 8 Blink",
            },
            Language::Ja => match self {
                Mode::Static8 => "8段階一般",
                Mode::Blink8 => "8段階点滅",
            },
        };
        tts_segment!(text)
            .into_iter()
            .map(AudioSegment::Text)
            .collect()
    }
}

const OUT_LINE_DOT_SIZE: i16 = 1; // 외곽선 도트 사이즈
const OUT_LINE_DOT_STEP: i16 = 15; // 외곽선 이동속도
const OUT_LINE_DOT_GAP: i16 = 3; // 외곽선 도트 간격
const BOX_AREA_GAP: i16 = 2; // 외곽선과 진동 박스 사이 간격
const INNER_BORDER_GAP: i16 = 1; // 내부 테두리 간격

/// 데모 애플릿을 통합 제어하며, 현재 활성화된 화면 모드 객체에 동작 및 출력을 위임하는 메인 구조체입니다.
pub struct DemoApp {
    /// 2차원 바둑판 격자 컴포넌트
    grid: ItemGrid,
    /// 현재 동작 중인 화면 모드 (Static8 / Blink8)
    pub mode: Mode,
    /// 테두리 회전 애니메이션 연동을 위한 매 프레임 증가 틱 값
    tick: i16,
}

impl Default for DemoApp {
    fn default() -> Self {
        DemoApp::new(Mode::Blink8)
    }
}

impl DemoApp {
    /// 특정 화면 모드로 프리셋 지정된 새로운 인스턴스를 반환합니다.
    pub fn new(mode: Mode) -> Self {
        let total = mode.total_steps();

        // 그리드 사양: 고정 크기(14x14) 자동 레이아웃
        let behavior = SelectorBehavior {
            scroll_canvas_size: None,
            layout_strategy: LayoutStrategy::FixedItemSize {
                width: 14,
                height: 14,
            },
            selection_style: None, // 포커스 셀 테두리 표시 비활성화
        };

        let grid = ItemGrid::new(2, 4, total, behavior);

        Self {
            grid,
            mode,
            tick: 0,
        }
    }

    /// 화면 모드가 바뀌었을 때 형태를 재조정하고 정중앙 정렬하는 헬퍼 함수
    fn update_mode(&mut self, mode: Mode, width: i16, height: i16) {
        self.mode = mode;
        let total = mode.total_steps();

        self.grid.rows = 2;
        self.grid.cols = 4;
        self.grid.total_items = total;
        self.grid.selected_index = 0;

        let margin = (INNER_BORDER_GAP + OUT_LINE_DOT_SIZE + BOX_AREA_GAP) * 2;
        let available_w = width - margin;
        let available_h = height - margin;

        // 반환받은 중심 좌표에 테두리 두께 절반 마진(4px)을 등분 추가하여 테두리 안쪽 정가운데에 배치합니다.
        self.grid.set_bounds(BoundsRect::new(Point::new(margin / 2, margin / 2), Size::new(available_w, available_h)));
        
        let mut intensities = Vec::new();
        for idx in 0..self.grid.total_items {
            let val = ((idx as u16) * 255 / 7) as u8;
            intensities.push(match self.mode {
                Mode::Blink8 => Intensity::new_blink(val),
                Mode::Static8 => Intensity::new(val),
            });
        }
        self.grid.set_intensities(intensities);
    }
}

impl Applet for DemoApp {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let width = context.window.width();
        let height = context.window.height();

        self.update_mode(self.mode, width, height);
        self.tick = 0;

        // 시작 보이스 안내
        let mut segments = match context.language {
            Language::Ko => {
                tts_segment!("펑션키를 누르면 일반 모드와 깜빡임 모드가 전환됩니다. 현재 모드 ")
            }
            Language::En => {
                tts_segment!("Press function key to toggle static and blink modes. Current mode ")
            }
            Language::Ja => tts_segment!(
                "ファンクションキーを押すと一般モードと点滅モードが切り替わります。現在のモード "
            ),
        }
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();

        segments.extend(self.mode.to_segments(context.language));
        context
            .audio
            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        // 매 프레임마다 tick 값을 증가시켜 이중 회전 테두리 애니메이션 진행
        self.tick = self.tick.wrapping_add(1);

        let width = context.window.width();
        let height = context.window.height();

        // 펑션 키가 눌리면 일반 모드와 깜빡임 모드를 순환 토글 (Static8 <-> Blink8)
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Function => {
                        let next_mode = match self.mode {
                            Mode::Blink8 => Mode::Static8,
                            Mode::Static8 => Mode::Blink8,
                        };
                        self.update_mode(next_mode, width, height);

                        let segments = self.mode.to_segments(context.language);
                        context
                            .audio
                            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                    }
                    _ => {
                        // 방향키 그리드 탐색 이동 기능은 의도적으로 비활성화 및 반응하지 않음
                    }
                }
            }
        }
        let _ = self.grid.on_update(context, false);
        Ok(UpdateResult::NeedsRedraw)
    }

    fn on_draw(&self, display: &mut dyn DisplayInterface) -> Result<()> {
        let size = display.get_size();
        display.clear();

        // 1. 바깥쪽 테두리 설정 및 움직이는 이중 테두리 드로잉
        let borders = [(0, 1), (INNER_BORDER_GAP, -1)];

        for &(gap, direction) in borders.iter() {
            draw_moving_border(
                display,
                gap,
                gap,
                size.width - 2 * gap,
                size.height - 2 * gap,
                self.tick / OUT_LINE_DOT_STEP,
                direction,
                OUT_LINE_DOT_GAP,
                OUT_LINE_DOT_SIZE,
                OUT_LINE_DOT_SIZE,
            );
        }

        // 2. 이중 테두리 안쪽 정가운데 영역에 맞춰 정렬된 격자(ItemGrid) 컴포넌트 렌더링 호출
        let _ = self.grid.on_draw(display);

        Ok(())
    }
}

/// 회전하는 점선 테두리 위의 매핑 점을 원둘레 비율식으로 산출하는 공식 헬퍼 함수
fn get_perimeter_point(x: i16, y: i16, w: i16, h: i16, mut t: i16) -> Point {
    let l = 2 * w + 2 * h - 4;
    t = t.rem_euclid(l);
    if t < w {
        Point::new(x + t, y)
    } else if t < w + h - 1 {
        Point::new(x + w - 1, y + t - w + 1)
    } else if t < 2 * w + h - 2 {
        Point::new(x + w - 1 - (t - (w + h - 2)), y + h - 1)
    } else {
        Point::new(x, y + h - 1 - (t - (2 * w + h - 3)))
    }
}

/// 지정된 오프셋 및 회전 속도에 맞춰 이중 점선 테두리를 교차 회전시키는 드로잉 헬퍼 함수
#[allow(clippy::too_many_arguments)]
fn draw_moving_border(
    display: &mut dyn DisplayInterface,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    tick: i16,
    direction: i16,
    dot_gap: i16,
    dot_w: i16, // 점의 길이
    dot_h: i16, // 점의 두께
) {
    if w <= 1 || h <= 1 {
        return;
    }
    let l = 2 * w + 2 * h - 4;
    let period = dot_gap + dot_w;

    for t in 0..l {
        let phase = (t - tick * direction).rem_euclid(period);
        if phase < dot_w {
            let p = get_perimeter_point(x, y, w, h, t);
            for i in 0..dot_h {
                let new_p = if t < w {
                    // 상단 변, 두께는 하향 갱신
                    Point::new(p.x, p.y + i)
                } else if t < w + h - 1 {
                    // 우측 변, 두께는 좌향 갱신
                    Point::new(p.x - i, p.y)
                } else if t < 2 * w + h - 2 {
                    // 하단 변, 두께는 상향 갱신
                    Point::new(p.x, p.y - i)
                } else {
                    // 좌측 변, 두께는 우향 갱신
                    Point::new(p.x + i, p.y)
                };
                display.set_pin(new_p, Intensity::MAX);
            }
        }
    }
}

/// 런타임 시스템 환경에 진입하기 위한 최상위 동적 라이브러리 심볼 러너를 선언합니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let app = Box::new(DemoApp::default());
    sdk::applet::run(app);
}
