// extensions/component/src/item_selector/bar.rs

use super::config::{LayoutStrategy, SelectorBehavior, SelectorOrientation, Style};
use graphics::Graphics;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use views::canvas::PixelBuffer;
use views::viewport::Viewport;
use crate::core::{Widget, WidgetUpdateResult};

/// 스크롤 애니메이션 감속을 위한 보간 계수 (0.0 ~ 1.0)
const SCROLL_LERP_FACTOR: f32 = 0.2;

/// 가로 방향 배치를 정의하는 구체 구조체입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Horizontal;

impl SelectorOrientation for Horizontal {
    fn is_horizontal(&self) -> bool {
        true
    }
}

/// 세로 방향 배치를 정의하는 구체 구조체입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vertical;

impl SelectorOrientation for Vertical {
    fn is_vertical(&self) -> bool {
        true
    }
}

/// 1차원 선형 아이템 목록(가로/세로 Bar 형태)을 관리하는 컴포넌트입니다.
pub struct ItemBar<'a> {
    /// 컴포넌트에 표시할 문자열 목록
    pub items: Vec<&'a str>,
    /// 배치 및 탐색 방향 (가로 또는 세로)을 나타내는 트레잇 오브젝트
    pub orientation: Box<dyn SelectorOrientation>,
    /// 동작 및 스타일 설정 플래그 (스크롤 크기, 레이아웃 전략, 선택 효과 등)
    pub behavior: SelectorBehavior,
    /// 현재 방향키로 강조 및 선택된 아이템의 인덱스
    pub selected_index: usize,
    /// 화면 스크롤 애니메이션 처리를 위한 뷰포트
    viewport: Viewport,
    /// 아이템들을 가상으로 그릴 내부 오프스크린 픽셀 버퍼
    canvas: PixelBuffer,
    /// 현재 부드러운 애니메이션 중인 스크롤 실수값 (픽셀 기준)
    pub current_scroll: f32,
    /// 최종 도달해야 할 스크롤 실수 목표값 (픽셀 기준)
    pub target_scroll: f32,
    /// 각 아이템 박스의 가로 크기 (자동 계산에 의해 변경될 수 있음)
    pub item_width: i16,
    /// 각 아이템 박스의 세로 크기 (자동 계산에 의해 변경될 수 있음)
    pub item_height: i16,
    /// 아이템 박스들 사이의 물리적인 간격 (자동 계산에 의해 변경될 수 있음)
    pub spacing: i16,
    pub bounds: BoundsRect,
}

impl<'a> ItemBar<'a> {
    /// 새로운 ItemBar 인스턴스를 초기화합니다.
    pub fn new(
        items: Vec<&'a str>,
        orientation: Box<dyn SelectorOrientation>,
        behavior: SelectorBehavior,
    ) -> Self {
        Self {
            items,
            orientation,
            behavior,
            selected_index: 0,
            viewport: Viewport::new(Point::new(0, 0), 1, 1),
            canvas: PixelBuffer::new(1, 1),
            current_scroll: 0.0,
            target_scroll: 0.0,
            item_width: 0,
            item_height: 0,
            spacing: 0,
            bounds: BoundsRect::default(),
        }
    }

    /// 디스플레이 해상도 정보와 레이아웃 전략(LayoutStrategy)에 맞춰
    /// 아이템 상자 크기, 여백, 시작 위치를 동적으로 자동 계산하고 캔버스/뷰포트를 셋업합니다.
    fn init_layout(&mut self) {
        let display_width = self.bounds.width();
        let display_height = self.bounds.height();
        let mut start_x = 2;
        let mut start_y = 2;
        let n = self.items.len() as i16;

        if self.items.is_empty() {
            log::error!("{}", crate::item_selector::error::SelectorError::EmptyItems);
            return;
        }

        // 1. 레이아웃 전략에 따른 연산 분기
        match self.behavior.layout_strategy {
            LayoutStrategy::FixedItemSize { width, height } => {
                if width <= 0 || height <= 0 {
                    log::error!(
                        "{}",
                        crate::item_selector::error::SelectorError::InvalidItemSize
                    );
                    self.item_width = width.max(1);
                    self.item_height = height.max(1);
                } else {
                    self.item_width = width;
                    self.item_height = height;
                }

                if self.orientation.is_horizontal() {
                    let required = self.item_width * n;
                    if required > display_width {
                        log::error!(
                            "{}",
                            crate::item_selector::error::SelectorError::InsufficientSpace {
                                shortage_width: required - display_width,
                                shortage_height: 0,
                            }
                        );
                        self.spacing = 2; // 대체 여백
                    } else if n > 1 {
                        self.spacing = (display_width - required) / (n - 1);
                        start_x = ((display_width - required) % (n - 1)) / 2;
                    } else {
                        self.spacing = 0;
                        start_x = (display_width - required) / 2;
                    }
                    start_y = (display_height - self.item_height) / 2;
                } else {
                    let required = self.item_height * n;
                    if required > display_height {
                        log::error!(
                            "{}",
                            crate::item_selector::error::SelectorError::InsufficientSpace {
                                shortage_width: 0,
                                shortage_height: required - display_height,
                            }
                        );
                        self.spacing = 2; // 대체 여백
                    } else if n > 1 {
                        self.spacing = (display_height - required) / (n - 1);
                        start_y = ((display_height - required) % (n - 1)) / 2;
                    } else {
                        self.spacing = 0;
                        start_y = (display_height - required) / 2;
                    }
                    start_x = (display_width - self.item_width) / 2;
                }
            }
            LayoutStrategy::FixedSpacing { spacing } => {
                self.spacing = spacing.max(0);

                if self.orientation.is_horizontal() {
                    let spacing_total = self.spacing * (n - 1).max(0);
                    let available = display_width - spacing_total;
                    let calculated_w = if n > 0 { available / n } else { 0 };

                    if calculated_w <= 0 {
                        log::error!(
                            "{}",
                            crate::item_selector::error::SelectorError::InsufficientSpace {
                                shortage_width: 1 - calculated_w,
                                shortage_height: 0,
                            }
                        );
                        self.item_width = 4; // 대체 최소 크기
                    } else {
                        self.item_width = calculated_w;
                    }
                    self.item_height = (display_height - 4).max(2);
                    start_x = (display_width - (self.item_width * n + spacing_total)) / 2;
                    start_y = 2;
                } else {
                    let spacing_total = self.spacing * (n - 1).max(0);
                    let available = display_height - spacing_total;
                    let calculated_h = if n > 0 { available / n } else { 0 };

                    if calculated_h <= 0 {
                        log::error!(
                            "{}",
                            crate::item_selector::error::SelectorError::InsufficientSpace {
                                shortage_width: 0,
                                shortage_height: 1 - calculated_h,
                            }
                        );
                        self.item_height = 4; // 대체 최소 크기
                    } else {
                        self.item_height = calculated_h;
                    }
                    self.item_width = (display_width - 4).max(2);
                    start_y = (display_height - (self.item_height * n + spacing_total)) / 2;
                    start_x = 2;
                }
            }
            LayoutStrategy::Manual {
                width,
                height,
                spacing,
            } => {
                self.item_width = width.max(1);
                self.item_height = height.max(1);
                self.spacing = spacing.max(0);
                start_x = 2;
                start_y = 2;
            }
        }

        let item_step = if self.orientation.is_horizontal() {
            self.item_width + self.spacing
        } else {
            self.item_height + self.spacing
        };

        // 2. 스크롤 캔버스 크기 지정 및 물리값 초기화
        let (canvas_w, canvas_h, init_scroll) =
            if let Some((cw, ch)) = self.behavior.scroll_canvas_size {
                let limit_scroll = if self.orientation.is_horizontal() {
                    self.calculate_scroll_offset_x(display_width, item_step)
                } else {
                    self.calculate_scroll_offset_y(display_height, item_step)
                };
                (cw as usize, ch as usize, limit_scroll)
            } else {
                (display_width as usize, display_height as usize, 0.0)
            };

        self.current_scroll = init_scroll;
        self.target_scroll = init_scroll;

        let viewport_center = if self.orientation.is_horizontal() {
            Point::new(display_width / 2 + init_scroll as i16, display_height / 2)
        } else {
            Point::new(display_width / 2, display_height / 2 + init_scroll as i16)
        };

        self.canvas = PixelBuffer::new(canvas_w, canvas_h);
        self.viewport = Viewport::new(viewport_center, display_width, display_height);

        // 3. 가상 캔버스에 각 아이템 상자를 기본 테두리(1픽셀) 형태로 그리기
        let max_intensity = Intensity::MAX;

        for idx in 0..self.items.len() {
            let (item_x, item_y) = if self.orientation.is_horizontal() {
                (start_x + idx as i16 * item_step, start_y)
            } else {
                (start_x, start_y + idx as i16 * item_step)
            };

            for j in item_y..item_y + self.item_height {
                for i in item_x..item_x + self.item_width {
                    let is_border = j == item_y
                        || j == item_y + self.item_height - 1
                        || i == item_x
                        || i == item_x + self.item_width - 1;

                    if is_border
                        && i >= 0
                        && j >= 0
                        && i < self.canvas.width as i16
                        && j < self.canvas.height as i16
                    {
                        self.canvas.set_pixel(i as usize, j as usize, max_intensity);
                    }
                }
            }
        }
    }

    /// 방향키 키패드 입력에 맞춰 선택 인덱스를 갱신하고 스크롤 목표 좌표를 추적합니다.
    fn handle_key(&mut self, key_code: KeyCode) -> bool {
        let display_width = self.bounds.width();
        let display_height = self.bounds.height();
        if self.behavior.selection_style.is_none() {
            return false; // 선택 비활성화 상태
        }

        let mut is_changed = false;
        if self.orientation.is_horizontal() {
            match key_code {
                // 왼쪽 방향키 입력 시: 첫 번째 항목이 아닐 때만 이전 인덱스로 스크롤 이동 처리합니다.
                KeyCode::Left if self.selected_index > 0 => {
                    self.selected_index -= 1;
                    is_changed = true;
                }
                // 오른쪽 방향키 입력 시: 마지막 항목 전일 때만 다음 인덱스로 스크롤 이동 처리합니다.
                KeyCode::Right if self.selected_index + 1 < self.items.len() => {
                    self.selected_index += 1;
                    is_changed = true;
                }
                _ => {}
            }
        } else if self.orientation.is_vertical() {
            match key_code {
                // 위쪽 방향키 입력 시: 첫 번째 항목이 아닐 때만 이전 인덱스로 스크롤 이동 처리합니다.
                KeyCode::Up if self.selected_index > 0 => {
                    self.selected_index -= 1;
                    is_changed = true;
                }
                // 아래쪽 방향키 입력 시: 마지막 항목 전일 때만 다음 인덱스로 스크롤 이동 처리합니다.
                KeyCode::Down if self.selected_index + 1 < self.items.len() => {
                    self.selected_index += 1;
                    is_changed = true;
                }
                _ => {}
            }
        }

        if is_changed && self.behavior.scroll_canvas_size.is_some() {
            let item_step = if self.orientation.is_horizontal() {
                self.item_width + self.spacing
            } else {
                self.item_height + self.spacing
            };

            if self.orientation.is_horizontal() {
                self.target_scroll = self.calculate_scroll_offset_x(display_width, item_step);
            } else {
                self.target_scroll = self.calculate_scroll_offset_y(display_height, item_step);
            }
        }

        is_changed
    }

    /// 스크롤의 물리적 위치(Lerp 연산)를 주기적으로 갱신합니다.
    fn update_scroll(&mut self) -> bool {
        if self.behavior.scroll_canvas_size.is_none() {
            return false;
        }

        let delta = self.target_scroll - self.current_scroll;
        let is_moving = if delta.abs() > 0.1 {
            self.current_scroll += delta * SCROLL_LERP_FACTOR;
            true
        } else {
            if self.current_scroll != self.target_scroll {
                self.current_scroll = self.target_scroll;
                true
            } else {
                false
            }
        };

        if self.orientation.is_horizontal() {
            self.viewport.center.x = self.current_scroll as i16 + self.viewport.width / 2;
        } else {
            self.viewport.center.y = self.current_scroll as i16 + self.viewport.height / 2;
        }

        is_moving
    }

    /// 디스플레이 영역에 뷰포트 기반으로 최종 렌더링하고, 선택 효과 스타일(두께선, 반전)을 적용합니다.
    fn draw_internal(&self, display: &mut dyn DisplayInterface, x_offset: i16, y_offset: i16) {
        let display_size = display.get_size();

        // 1. 기본 캐싱 이미지 전송
        let pins = self
            .viewport
            .iter_visible_pixels(&self.canvas)
            .map(|mut pin| {
                pin.point.x += x_offset;
                pin.point.y += y_offset;
                pin
            });
        display.draw_iter(pins);

        // 2. 선택 효과 스타일이 지정되어 있으면 특수 렌더링 처리
        if let Some(style) = self.behavior.selection_style {
            let item_step = if self.orientation.is_horizontal() {
                self.item_width + self.spacing
            } else {
                self.item_height + self.spacing
            };

            // 월드 기준 시작 마진(start_x, start_y)을 구함
            let start_x = if self.behavior.scroll_canvas_size.is_some() {
                2
            } else {
                match self.behavior.layout_strategy {
                    LayoutStrategy::FixedItemSize { .. } | LayoutStrategy::FixedSpacing { .. } => {
                        let n = self.items.len() as i16;
                        if self.orientation.is_horizontal() {
                            let required = self.item_width * n;
                            if n > 1 {
                                ((display_width_helper(&self.behavior, display_size.width)
                                    - required)
                                    % (n - 1))
                                    / 2
                            } else {
                                (display_size.width - required) / 2
                            }
                        } else {
                            (display_size.width - self.item_width) / 2
                        }
                    }
                    LayoutStrategy::Manual { .. } => 2,
                }
            };
            let start_y = if self.behavior.scroll_canvas_size.is_some() {
                2
            } else {
                match self.behavior.layout_strategy {
                    LayoutStrategy::FixedItemSize { .. } | LayoutStrategy::FixedSpacing { .. } => {
                        let n = self.items.len() as i16;
                        if self.orientation.is_vertical() {
                            let required = self.item_height * n;
                            if n > 1 {
                                ((display_size.height - required) % (n - 1)) / 2
                            } else {
                                (display_size.height - required) / 2
                            }
                        } else {
                            (display_size.height - self.item_height) / 2
                        }
                    }
                    LayoutStrategy::Manual { .. } => 2,
                }
            };

            let (world_x, world_y) = if self.orientation.is_horizontal() {
                (start_x + self.selected_index as i16 * item_step, start_y)
            } else {
                (start_x, start_y + self.selected_index as i16 * item_step)
            };

            let screen_pos = self.viewport.world_to_screen(Point::new(world_x, world_y));
            let target_screen_x = screen_pos.x + x_offset;
            let target_screen_y = screen_pos.y + y_offset;

            // 디스플레이 한계 내부 가드
            if target_screen_x >= 0
                && target_screen_x + self.item_width <= display_size.width
                && target_screen_y >= 0
                && target_screen_y + self.item_height <= display_size.height
            {
                let has_border = style.border_thickness().is_some();
                let is_inverted = style.is_inverted();
                let t = if has_border {
                    style.normalized_border()
                } else {
                    0
                };

                if has_border {
                    let border_intensity = Intensity::MAX;
                    for j in target_screen_y..target_screen_y + self.item_height {
                        for i in target_screen_x..target_screen_x + self.item_width {
                            let offset_x = i - target_screen_x;
                            let offset_y = j - target_screen_y;

                            let is_thick_border = offset_x < t
                                || offset_x >= self.item_width - t
                                || offset_y < t
                                || offset_y >= self.item_height - t;

                            if is_thick_border {
                                display.set_pin(Point::new(i, j), border_intensity);
                            }
                        }
                    }
                }

                if is_inverted {
                    let inner_x = target_screen_x + t;
                    let inner_y = target_screen_y + t;
                    let inner_w = self.item_width - (t * 2);
                    let inner_h = self.item_height - (t * 2);

                    if inner_w > 0 && inner_h > 0 {
                        for j in inner_y..inner_y + inner_h {
                            for i in inner_x..inner_x + inner_w {
                                // 뷰포트 맵핑에 맞춰 기존 캔버스 픽셀을 읽은 뒤 반전
                                let vx = (i - x_offset) + self.viewport.center.x
                                    - self.viewport.width / 2;
                                let vy = (j - y_offset) + self.viewport.center.y
                                    - self.viewport.height / 2;
                                let origin_pixel = self.canvas.get_pixel(vx as usize, vy as usize);

                                let inverted = if origin_pixel.value > 0 {
                                    Intensity::OFF
                                } else {
                                    Intensity::MAX
                                };
                                display.set_pin(Point::new(i, j), inverted);
                            }
                        }
                    }
                }
            }
        }
    }

    fn calculate_scroll_offset_x(&self, display_width: i16, item_step: i16) -> f32 {
        let visible_cols = (display_width / item_step).max(1);
        let selected_col = self.selected_index as i16;
        if selected_col >= visible_cols {
            ((selected_col - visible_cols + 1) * item_step) as f32
        } else {
            0.0
        }
    }

    fn calculate_scroll_offset_y(&self, display_height: i16, item_step: i16) -> f32 {
        let visible_rows = (display_height / item_step).max(1);
        let selected_row = self.selected_index as i16;
        if selected_row >= visible_rows {
            ((selected_row - visible_rows + 1) * item_step) as f32
        } else {
            0.0
        }
    }
}

fn display_width_helper(behavior: &SelectorBehavior, default_w: i16) -> i16 {
    if let Some((cw, _)) = behavior.scroll_canvas_size {
        cw
    } else {
        default_w
    }
}

impl<'a> Widget for ItemBar<'a> {
    fn bounds(&self) -> BoundsRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: BoundsRect) {
        self.bounds = bounds;
        self.init_layout();
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_update(&mut self, context: &mut Context, is_focused: bool) -> sdk::error::Result<WidgetUpdateResult> {
        let mut needs_redraw = false;

        if is_focused {
            let mut unhandled_events = Vec::new();
            while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
                if event.state == KeyState::Pressed {
                    if self.handle_key(event.code) {
                        needs_redraw = true;
                    } else {
                        unhandled_events.push(event);
                    }
                } else {
                    unhandled_events.push(event);
                }
            }
            // 되돌려놓기 (나중에 뽑은 것부터 앞에 넣어야 순서 유지되지만, 
            // 위젯에서는 처리하지 않은 이벤트를 다시 순서대로 넣으려면 역순으로 넣어야 함)
            for event in unhandled_events.into_iter().rev() {
                context.keypad.push_event_front(event);
            }
        }

        if self.update_scroll() {
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(WidgetUpdateResult::NeedsRedraw)
        } else {
            Ok(WidgetUpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, display: &mut dyn DisplayInterface) -> sdk::error::Result<()> {
        self.draw_internal(display, self.bounds.x(), self.bounds.y());
        Ok(())
    }
}
