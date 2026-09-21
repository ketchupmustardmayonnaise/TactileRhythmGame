use crate::game::DrawingGame;
use crate::types::EditorMode;
use widget::item_selector::{
    Horizontal, ItemBar, LayoutStrategy, SelectorBehavior, SelectorStyle,
};
use widget::core::{Widget, WidgetUpdateResult};
use sdk::Language;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity, Point, Size};

/// 문자열(str)을 인자로 받아 그에 해당하는 메뉴 항목을 상자로 그리고,
/// 안쪽에 상세 설정 값(values)을 작은 상자로 그리는 함수입니다.
/// (음성 출력은 매 프레임 렌더링 시 중복 발생을 막기 위해 이벤트 처리부에서 수행합니다)
const UNSELECTED_BOX_INTENSITY: u8 = 80; // 메뉴 팝업시 배경
const ITEM_HEIGHT: i16 = 5; // 메뉴 항목 상자 높이 (외곽선 1 + 내부 3 + 외곽선 1 = 5)
const ITEM_WIDTH: i16 = 5; // 메뉴 항목 상자의 너비 (외곽선 1 + 내부 3 + 외곽선 1 = 5)
const ITEM_SPACING: i16 = 2; // 메뉴 항목 간의 간격
const MENU_HEIGHT: i16 = ITEM_HEIGHT + 2 + 2; // 메뉴 전체 높이 (상자 + 여백+ 외각선)

pub enum MenuAction {
    None,
    Redraw,
    SelectMode(EditorMode),
    Close,
}

pub struct Menu {
    /// 사용자가 선택할 수 있는 그림판의 에디터 모드 목록
    pub items: Vec<EditorMode>,
    /// 도구 선택을 관리하는 가로형 ItemBar 메뉴 바 컴포넌트
    pub item_bar: ItemBar<'static>,
}

impl Menu {
    /// 메뉴 화면을 최종 렌더링하는 함수입니다.
    pub fn draw(&self, canvas: &mut dyn DisplayInterface, game: &DrawingGame) {
        let display_size = canvas.get_size();
        let menu_width = display_size.width;
        let start_y = display_size.height - MENU_HEIGHT;

        // 1. 메뉴 바깥 영역(배경)에 원래 그려져 있던 픽셀들을 약한 강도로 흐리게 처리합니다.
        let dim_intensity = Intensity::new(UNSELECTED_BOX_INTENSITY);
        for pin in game.viewport.iter_visible_pixels(&game.canvas) {
            if pin.point.y < start_y && pin.intensity != Intensity::MIN {
                canvas.set_pin(pin.point, dim_intensity);
            }
        }

        // 2. 메뉴 영역이 차지할 하단 패널의 배경을 지우고 테두리 외곽선을 그립니다.
        for j in start_y..start_y + MENU_HEIGHT {
            for i in 0..menu_width {
                let is_border =
                    j == start_y || j == start_y + MENU_HEIGHT - 1 || i == 0 || i == menu_width - 1;
                let intensity = if is_border {
                    Intensity::MAX
                } else {
                    Intensity::MIN
                };
                canvas.set_pin(Point::new(i, j), intensity);
            }
        }

        // 3. ItemBar 컴포넌트를 호출하여 스크롤 위치 및 선택 강조 효과를 적용한 메뉴 상자들을 하단 영역(y_offset = start_y)에 그립니다.
        let _ = self.item_bar.on_draw(canvas);
    }

    /// 메뉴가 나타날 때, 아이템 수와 화면 가로 너비에 적합한 가상 캔버스를 초기화하고 뷰포트 스크롤 상태를 연산합니다.
    pub fn init_canvas(&mut self, width: i16, _height: i16, current_mode: EditorMode) {
        let item_step_x = ITEM_WIDTH + ITEM_SPACING;
        let total_width = self.items.len() as i16 * item_step_x + 4; // 4는 좌우 여백
        let canvas_width = total_width.max(width);

        // 현재 설정되어 있는 모드의 인덱스를 찾아 시작 포커스로 지정합니다.
        let selected_index = self
            .items
            .iter()
            .position(|&m| m == current_mode)
            .unwrap_or(0);

        // 가로 방향 메뉴 바 컴포넌트의 가상 캔버스 정보, 수동 배치 전략 및 스타일을 세팅합니다.
        let behavior = SelectorBehavior {
            scroll_canvas_size: Some((canvas_width, MENU_HEIGHT)),
            layout_strategy: LayoutStrategy::Manual {
                width: ITEM_WIDTH,
                height: ITEM_HEIGHT,
                spacing: ITEM_SPACING,
            },
            selection_style: Some(SelectorStyle {
                border_thickness: Some(1),
                invert: true,
            }),
        };

        // 개수 비율을 맞추기 위해 더미 텍스트 배열을 전달합니다.
        let items = vec![""; self.items.len()];

        // 새로운 ItemBar 인스턴스를 가로(Horizontal) 방향 배치 속성으로 생성합니다.
        self.item_bar = ItemBar::new(items, Box::new(Horizontal), behavior);
        self.item_bar.selected_index = selected_index;

        // 레이아웃의 시작 좌표 및 여백을 계산해 초기화합니다.
        let start_y = _height - MENU_HEIGHT;
        self.item_bar.set_bounds(BoundsRect::new(Point::new(0, start_y), Size::new(width, MENU_HEIGHT)));
    }

    /// 키패드 입력을 전달받아 메뉴 항목 간 탐색 및 동작 완료 처리를 수행합니다.
    pub fn on_update(
        &mut self,
        context: &mut Context,
        language: Language,
    ) -> MenuAction {
        let prev_index = self.item_bar.selected_index;
        
        let update_res = self.item_bar.on_update(context, true).unwrap_or(WidgetUpdateResult::Unchanged);
        let mut action = MenuAction::None;
        
        if update_res == WidgetUpdateResult::NeedsRedraw {
            action = MenuAction::Redraw;
            if prev_index != self.item_bar.selected_index {
                let current_mode = self.items[self.item_bar.selected_index];
                context.audio.speak_text(current_mode.text(language));
            }
        }

        while let sdk::api::keypad::KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state != sdk::api::keypad::KeyState::Pressed {
                continue;
            }

            match event.code {
                // 선택 키를 누르면 현재 선택된 기능으로 에디터 모드를 전환하고 음성을 출력합니다.
                sdk::api::keypad::KeyCode::Center => {
                    let current_mode = self.items[self.item_bar.selected_index];
                    context.audio.speak_text(current_mode.text(language));
                    return MenuAction::SelectMode(current_mode);
                }
                // 메뉴 키를 누르면 도구 메뉴를 닫습니다.
                sdk::api::keypad::KeyCode::Menu => {
                    context.audio.speak_text(match language {
                        Language::Ko => "도구 닫기",
                        Language::En => "Close tools",
                        Language::Ja => "ツールを閉じる",
                    });
                    return MenuAction::Close;
                }
                _ => {}
            }
        }
        action
    }
}

impl Default for Menu {
    fn default() -> Self {
        let behavior = SelectorBehavior::default();
        Self {
            // 사용자가 그림판에서 고를 수 있는 도구(에디터 모드) 목록
            items: vec![
                EditorMode::Draw,
                EditorMode::Erase,
                EditorMode::BrushSizeChange,
            ],
            // Default 반환을 위한 임시 ItemBar 컴포넌트 생성
            item_bar: ItemBar::new(Vec::new(), Box::new(Horizontal), behavior),
        }
    }
}
