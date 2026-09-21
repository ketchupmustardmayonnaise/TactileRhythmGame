use std::time::Duration;

use physics::{InputFlags, MovementController, PhysicsState, StepAndAccelerateMode};
use sdk::Language;
use sdk::api::context::Context;
use sdk::api::display::Point;
use views::canvas::PixelBuffer;

use crate::config::GameConfig;
use crate::speak::SpeakEvent;
use crate::types::{EditorMode, FunctionKeyFlags, KeyBuffer};
use views::viewport::Viewport;

/// 게임의 상태를 관리하는 구조체.
///
/// # 조작 방법
/// - **방향키**: 커서 이동 (가속도 적용)
/// - **Center (누름)**: 현재 모드에 따라 그리기 또는 지우기 수행 (일반 모드 제외)
/// - **Center + End**: 모드 순환 (그리기 -> 지우기 -> 일반)
/// - **Center + Left/Right**: 이동 모드 변경 (Standard ↔ Inertia)
/// - **Home**: 일반 모드로 즉시 복귀
///
/// 디스플레이, 키패드 및 커서의 현재 위치를 포함합니다.
pub struct Game {
    pub viewport: Viewport, // 뷰포트 객체 (화면 영역 및 좌표 변환 담당)
    pub config: GameConfig, // 게임 설정 (물리, 렌더링, 시스템 등)
    pub mode: EditorMode,
    pub need_redraw: bool,
    pub canvas: PixelBuffer, // 화면과 동일한 크기의 8비트 픽셀 버퍼
    pub backup_canvas: Option<PixelBuffer>, // 더블 클릭 시 복원을 위한 캔버스 백업
    pub fn_key_state: FunctionKeyFlags, // 현재 기능키 상태 비트마스크
    pub prev_fn_key_state: FunctionKeyFlags, // 이전 프레임 기능키 상태 (에지 검출용)
    pub input_state: InputFlags, // 현재 눌린 방향키 상태 비트마스크
    pub pressed_keys: KeyBuffer, // 눌린 키의 순서를 추적하는 버퍼
    pub physics_state: PhysicsState, // 물리 상태 (위치, 속도, 경계)
    pub movement_controller: MovementController, // 이동 모드 컨트롤러
    pub last_px: i16,        // 이전 프레임의 정수 픽셀 좌표
    pub last_py: i16,        // 이전 프레임의 정수 픽셀 좌표
    pub cursor_blink_timer: Duration, // 커서 깜박임 타이머
    pub cursor_visible: bool, // 커서 표시 여부

    pub last_time: Duration,
    pub accumulator: Duration,
    pub language: Language,
}

impl Game {
    /// 새로운 게임 인스턴스를 생성합니다.
    /// 디스플레이와 키패드를 초기화하고, 커서를 중앙에 배치한 후 초기 화면을 그립니다.
    pub fn new(config: GameConfig) -> Self {
        let viewport = Viewport::new(Point::new(0, 0), 0, 0);

        // 초기화 시점에서는 화면 크기를 모르므로 임시 크기로 생성합니다.
        // 이후 lib.rs의 on_window_event에서 실제 화면 크기로 다시 생성됩니다.
        let canvas = PixelBuffer::new(1, 1);

        let physics_state = PhysicsState::new(0, 0);
        let movement_controller = MovementController::new(Box::new(StepAndAccelerateMode::new()));

        Self {
            viewport,
            config,
            mode: EditorMode::Draw, // 기본 모드는 '그리기'
            need_redraw: false,
            canvas,
            backup_canvas: None,
            fn_key_state: FunctionKeyFlags::empty(),
            prev_fn_key_state: FunctionKeyFlags::empty(),
            input_state: InputFlags::empty(),
            pressed_keys: KeyBuffer::new(),
            physics_state,
            movement_controller,
            last_px: 0,
            last_py: 0,
            cursor_blink_timer: Duration::ZERO,
            cursor_visible: true,
            last_time: Duration::ZERO,
            accumulator: Duration::ZERO,
            language: Language::default(),
        }
    }

    /// 편집 모드를 변경하고 변경된 모드를 음성으로 안내합니다.
    pub fn set_mode(&mut self, context: &mut Context, new_mode: EditorMode) {
        log::info!("모드 변경: {:?}", new_mode);
        // 모드 변경 및 이벤트 발생 (Speaker가 중복 필터링 수행)
        self.mode = new_mode;
        context
            .audio
            .speak(&SpeakEvent::ToolChange(self.mode), self.language);
        self.need_redraw = true;
    }

    const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(300);

    /// 커서 깜박임 타이머를 업데이트하고 표시 여부를 전환합니다.
    pub fn update_cursor_blink(&mut self, frame_time: Duration) {
        if !self.input_state.is_empty() {
            // 방향키 입력 중일 때는 커서를 계속 표시하고 타이머 초기화
            if !self.cursor_visible {
                self.cursor_visible = true;
                self.need_redraw = true;
            }
            self.cursor_blink_timer = Duration::ZERO;
        } else {
            // 이동 중이 아닐 때만 타이머를 증가시키고 깜박임 처리
            self.cursor_blink_timer += frame_time;
            if self.cursor_blink_timer >= Self::CURSOR_BLINK_INTERVAL {
                self.cursor_blink_timer -= Self::CURSOR_BLINK_INTERVAL;
                self.cursor_visible = !self.cursor_visible;
                self.need_redraw = true; // 깜박임 상태가 바뀌었으므로 화면을 강제로 갱신
            }
        }
    }
}
