// extensions/component/src/item_selector/grid.rs

use super::config::{LayoutStrategy, SelectorBehavior, SelectorOrientation, Style};
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use crate::core::{Widget, WidgetUpdateResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid;

impl SelectorOrientation for Grid {
    fn is_grid(&self) -> bool {
        true
    }
}

/// 2차원 바둑판식 아이템 배열(격자 형태 Grid)을 관리하고 렌더링하는 컴포넌트입니다.
pub struct ItemGrid {
    /// 격자의 줄 수(Rows)
    pub rows: usize,
    /// 격자의 칸 수(Columns)
    pub cols: usize,
    /// 표시할 총 아이템(셀) 수
    pub total_items: usize,
    /// 동작 및 스타일 설정 플래그 (레이아웃 전략, 선택 효과 등)
    pub behavior: SelectorBehavior,
    /// 현재 강조 및 선택된 아이템의 인덱스 (0부터 total_items - 1)
    pub selected_index: usize,
    /// 각 격자 셀의 가로 너비 (자동 계산에 의해 변경될 수 있음)
    pub cell_width: i16,
    /// 각 격자 셀의 세로 높이 (자동 계산에 의해 변경될 수 있음)
    pub cell_height: i16,
    /// 격자 셀들 간의 간격 (자동 계산에 의해 변경될 수 있음)
    pub spacing: i16,
    pub bounds: BoundsRect,
    start_x: i16,
    start_y: i16,
    pub cell_intensities: Vec<Intensity>,
}

impl ItemGrid {
    /// 새로운 ItemGrid 인스턴스를 초기화합니다.
    pub fn new(rows: usize, cols: usize, total_items: usize, behavior: SelectorBehavior) -> Self {
        Self {
            rows,
            cols,
            total_items,
            behavior,
            selected_index: 0,
            cell_width: 0,
            cell_height: 0,
            spacing: 0,
            bounds: BoundsRect::default(),
            start_x: 2,
            start_y: 2,
            cell_intensities: vec![Intensity::MAX; total_items],
        }
    }

    /// 셀의 밝기를 한꺼번에 설정합니다.
    pub fn set_intensities(&mut self, intensities: Vec<Intensity>) {
        if intensities.len() == self.total_items {
            self.cell_intensities = intensities;
        }
    }

    /// 상, 하, 좌, 우 방향키 조작에 대응하여 2차원 격자 인덱스를 안전하게 변경합니다.
    fn handle_key(&mut self, key_code: KeyCode) -> bool {
        if self.behavior.selection_style.is_none() {
            return false; // 선택 효과 비활성화 시 입력 무시
        }

        let current_row = self.selected_index / self.cols;
        let current_col = self.selected_index % self.cols;
        let mut next_row = current_row;
        let mut next_col = current_col;

        match key_code {
            KeyCode::Left => {
                if current_col > 0 {
                    next_col -= 1;
                }
            }
            KeyCode::Right => {
                if current_col + 1 < self.cols {
                    next_col += 1;
                }
            }
            KeyCode::Up => {
                if current_row > 0 {
                    next_row -= 1;
                }
            }
            KeyCode::Down => {
                if current_row + 1 < self.rows {
                    next_row += 1;
                }
            }
            _ => return false,
        }

        let next_index = next_row * self.cols + next_col;
        if next_index < self.total_items {
            self.selected_index = next_index;
            true
        } else {
            false
        }
    }

    /// 디스플레이 크기와 레이아웃 전략을 수신하여
    /// 최적의 셀 크기(cell_width, cell_height), 여백(spacing), 그리기 시작 위치를 계산하고 오프셋을 설정합니다.
    fn auto_layout(&mut self) {
        let display_width = self.bounds.width();
        let display_height = self.bounds.height();
        if self.total_items == 0 {
            log::error!("{}", crate::item_selector::error::SelectorError::EmptyItems);
            self.start_x = 2;
            self.start_y = 2;
            return;
        }

        match self.behavior.layout_strategy {
            LayoutStrategy::FixedItemSize { width, height } => {
                if width <= 0 || height <= 0 {
                    log::error!(
                        "{}",
                        crate::item_selector::error::SelectorError::InvalidItemSize
                    );
                    self.cell_width = width.max(1);
                    self.cell_height = height.max(1);
                } else {
                    self.cell_width = width;
                    self.cell_height = height;
                }

                let mut total_w = self.cell_width * self.cols as i16;
                let mut total_h = self.cell_height * self.rows as i16;

                let min_spacing = 2;
                let spacing_w = min_spacing * (self.cols as i16 - 1).max(0);
                let spacing_h = min_spacing * (self.rows as i16 - 1).max(0);

                let mut shortage_w = 0;
                let mut shortage_h = 0;
                if total_w + spacing_w > display_width {
                    shortage_w = total_w + spacing_w - display_width;
                }
                if total_h + spacing_h > display_height {
                    shortage_h = total_h + spacing_h - display_height;
                }

                if shortage_w > 0 || shortage_h > 0 {
                    log::error!(
                        "{}",
                        crate::item_selector::error::SelectorError::InsufficientSpace {
                            shortage_width: shortage_w,
                            shortage_height: shortage_h,
                        }
                    );

                    // Fallback: 가용 공간 내에 맞게 박스 크기 자동 축소 (최소 간격 2 보장)
                    let available_w = (display_width - spacing_w).max(1);
                    let available_h = (display_height - spacing_h).max(1);

                    self.cell_width = self
                        .cell_width
                        .min(if self.cols > 0 {
                            available_w / self.cols as i16
                        } else {
                            available_w
                        })
                        .max(1);
                    self.cell_height = self
                        .cell_height
                        .min(if self.rows > 0 {
                            available_h / self.rows as i16
                        } else {
                            available_h
                        })
                        .max(1);

                    total_w = self.cell_width * self.cols as i16;
                    total_h = self.cell_height * self.rows as i16;
                }

                let empty_w = display_width - total_w;
                let empty_h = display_height - total_h;

                let spacing_x = if self.cols > 1 {
                    empty_w / (self.cols as i16 - 1)
                } else {
                    0
                };
                let spacing_y = if self.rows > 1 {
                    empty_h / (self.rows as i16 - 1)
                } else {
                    0
                };

                // 정사각형 조밀도 유지를 위해 최소값 선택 적용
                self.spacing = spacing_x.min(spacing_y).max(0);

                // 그리드에서 실제로 사용하게 될 총 픽셀 가로/세로 길이를 계산합니다.
                // 이미 (cols/rows as i16 - 1).max(0)의 결과 타입이 i16이므로 중복된 as i16 캐스팅을 제거합니다.
                let used_w = total_w + self.spacing * (self.cols as i16 - 1).max(0);
                let used_h = total_h + self.spacing * (self.rows as i16 - 1).max(0);

                self.start_x = (display_width - used_w) / 2;
                self.start_y = (display_height - used_h) / 2;
            }
            LayoutStrategy::FixedSpacing { spacing } => {
                self.spacing = spacing.max(0);

                let spacing_w_total = self.spacing * (self.cols as i16 - 1).max(0);
                let spacing_h_total = self.spacing * (self.rows as i16 - 1).max(0);

                let available_w = display_width - spacing_w_total;
                let available_h = display_height - spacing_h_total;

                let calc_w = if self.cols > 0 {
                    available_w / self.cols as i16
                } else {
                    0
                };
                let calc_h = if self.rows > 0 {
                    available_h / self.rows as i16
                } else {
                    0
                };

                if calc_w <= 0 || calc_h <= 0 {
                    log::error!(
                        "{}",
                        crate::item_selector::error::SelectorError::InsufficientSpace {
                            shortage_width: if calc_w <= 0 { 1 - calc_w } else { 0 },
                            shortage_height: if calc_h <= 0 { 1 - calc_h } else { 0 },
                        }
                    );
                    self.cell_width = 4;
                    self.cell_height = 4; // 최소 Fallback
                } else {
                    self.cell_width = calc_w;
                    self.cell_height = calc_h;
                }

                let used_w = self.cell_width * self.cols as i16 + spacing_w_total;
                let used_h = self.cell_height * self.rows as i16 + spacing_h_total;

                self.start_x = (display_width - used_w) / 2;
                self.start_y = (display_height - used_h) / 2;
            }
            LayoutStrategy::Manual {
                width,
                height,
                spacing,
            } => {
                self.cell_width = width.max(1);
                self.cell_height = height.max(1);
                self.spacing = spacing.max(0);
                self.start_x = 2;
                self.start_y = 2;
            }
        }
    }

    /// 계산된 레이아웃 정보를 바탕으로 격자 박스들을 그리고, 선택된 항목에 특화된 피드백(두께, 반전)을 적용합니다.
    fn draw_internal(
        &self,
        display: &mut dyn DisplayInterface,
        start_x: i16,
        start_y: i16,
    ) {
        let max_intensity = Intensity::MAX;

        for i in 0..self.total_items {
            let row = i / self.cols;
            let col = i % self.cols;

            let x = self.bounds.x() + start_x + col as i16 * (self.cell_width + self.spacing);
            let y = self.bounds.y() + start_y + row as i16 * (self.cell_height + self.spacing);

            let is_selected = self.behavior.selection_style.is_some() && (i == self.selected_index);
            let intensity = self.cell_intensities[i];

            if is_selected {
                let style = self.behavior.selection_style.unwrap();
                let has_border = style.border_thickness().is_some();
                let is_inverted = style.is_inverted();
                let t = if has_border {
                    style.normalized_border()
                } else {
                    0
                };

                // 1. 테두리 그리기
                if has_border {
                    for j in y..y + self.cell_height {
                        for i_pin in x..x + self.cell_width {
                            let offset_x = i_pin - x;
                            let offset_y = j - y;

                            let is_thick_border = offset_x < t
                                || offset_x >= self.cell_width - t
                                || offset_y < t
                                || offset_y >= self.cell_height - t;

                            if is_thick_border {
                                display.set_pin(Point::new(i_pin, j), max_intensity);
                            }
                        }
                    }
                }

                // 2. 내부 영역 칠하기 (반전 또는 원래 강도)
                let inner_x = x + t;
                let inner_y = y + t;
                let inner_w = self.cell_width - (t * 2);
                let inner_h = self.cell_height - (t * 2);

                if inner_w > 0 && inner_h > 0 {
                    for j in inner_y..inner_y + inner_h {
                        for i_pin in inner_x..inner_x + inner_w {
                            if is_inverted {
                                let inverted = if intensity.value > 0 {
                                    Intensity::OFF
                                } else {
                                    Intensity::MAX
                                };
                                display.set_pin(Point::new(i_pin, j), inverted);
                            } else {
                                display.set_pin(Point::new(i_pin, j), intensity);
                            }
                        }
                    }
                }
            } else {
                // 일반 비선택 상자: 원래 크기로 전체를 해당 강도로 균등하게 칠함
                for j in y..y + self.cell_height {
                    for i_pin in x..x + self.cell_width {
                        display.set_pin(Point::new(i_pin, j), intensity);
                    }
                }
            }
        }
    }
}

impl Widget for ItemGrid {
    fn bounds(&self) -> BoundsRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: BoundsRect) {
        self.bounds = bounds;
        self.auto_layout();
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
            for event in unhandled_events.into_iter().rev() {
                context.keypad.push_event_front(event);
            }
        }

        if needs_redraw {
            Ok(WidgetUpdateResult::NeedsRedraw)
        } else {
            Ok(WidgetUpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, display: &mut dyn DisplayInterface) -> sdk::error::Result<()> {
        self.draw_internal(display, self.start_x, self.start_y);
        Ok(())
    }
}
