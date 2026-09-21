use widget::item_selector::{
    ItemBar, LayoutStrategy, SelectorBehavior, SelectorStyle, Vertical,
};
use graphics::{Draw, Line, Rectangle, style::Style};
use widget::core::{Widget, WidgetUpdateResult};
use sdk::Applet;
use sdk::Language;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size, BoundsRect};

const AXIS_SHARPNESS: u8 = 255; // 축강도
const GRAPH_SHARPNESS: u8 = 130; //그래프 강도
const ITEM_HEIGHT: i16 = 5; // 메뉴 항목 상자 높이 (외곽선 1 + 내부 3 + 외곽선 1 = 5)
const ITEM_WIDTH: i16 = 5; // 메뉴 항목 상자의 너비 (외곽선 1 + 내부 3 + 외곽선 1 = 5)
const ITEM_SPACING: i16 = 2; // 메뉴 항목 간의 간격
const MENU_PANEL_WIDTH: i16 = ITEM_WIDTH + 2 + 2; // 좌측 메뉴 패널 너비 (상자 + 여백+ 외곽선)

// 그래프 종류를 나타내는 열거형
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum GraphType {
    LinearEquation,
    QuadraticEquation,
    TrigonometricFunction,
    ExponentialFunction,
    BarChart,
    PieChart,
    LineGraph,
}

impl GraphType {
    pub fn text(&self, lang: Language) -> &'static str {
        match lang {
            Language::Ko => self.text_ko(),
            Language::En => self.text_en(),
            _ => self.text_ko(),
        }
    }

    fn text_ko(&self) -> &'static str {
        match self {
            Self::LinearEquation => "선형 방정식",
            Self::QuadraticEquation => "이차 방정식",
            Self::TrigonometricFunction => "삼각 함수",
            Self::ExponentialFunction => "지수 함수",
            Self::BarChart => "막대 차트",
            Self::PieChart => "파이 차트",
            Self::LineGraph => "선 그래프",
        }
    }
    fn text_en(&self) -> &'static str {
        match self {
            Self::LinearEquation => "Linear Equation",
            Self::QuadraticEquation => "Quadratic Equation",
            Self::TrigonometricFunction => "Trigonometric Function",
            Self::ExponentialFunction => "Exponential Function",
            Self::BarChart => "Bar Chart",
            Self::PieChart => "Pie Chart",
            Self::LineGraph => "Line Graph",
        }
    }
}

pub fn draw_graph(canvas: &mut dyn DisplayInterface, graph_type: GraphType, linear_x_offset: i16) {
    canvas.clear(); // 화면을 검은색으로 지웁니다.

    let size = canvas.get_size();
    let get_width = size.width;
    let get_height = size.height;

    let offset_x = MENU_PANEL_WIDTH; // 좌측 메뉴 패널 너비만큼 우측으로 이동
    let offset_y = 1; // 상하 여백을 위한 Y 오프셋
    let width = get_width - offset_x - 2; // 우측 여백 2를 고려한 실제 그리기 너비
    let height = get_height - 2; // 상하 여백 2를 고려한 실제 그리기 높이
    let cx = width / 2;
    let cy = height / 2;
    let screen_cx = offset_x + cx;
    let screen_cy = offset_y + cy;

    // 축을 그리는 헬퍼 클로저
    let mut draw_axes = |x_axis_y: i16, y_axis_x: i16| {
        // X축 그리기 (약한 강도로 표현하여 그래프와 구분)
        Line::new(
            Point::new(offset_x, x_axis_y),
            Point::new(offset_x + width, x_axis_y),
        )
        .style(Style::with_stroke(Intensity::new(AXIS_SHARPNESS), 1))
        .draw(canvas);

        // Y축 그리기
        Line::new(
            Point::new(y_axis_x, offset_y),
            Point::new(y_axis_x, offset_y + height),
        )
        .style(Style::with_stroke(Intensity::new(AXIS_SHARPNESS), 1))
        .draw(canvas);
    };

    match graph_type {
        // 선형 방정식 그래프 그리기 (y = x)
        GraphType::LinearEquation => {
            draw_axes(screen_cy, screen_cx);

            let draw_linear_line = |canvas: &mut dyn DisplayInterface, x_offset: i16, intensity: Intensity| {
                for x in 0..width {
                    let px = offset_x + x;
                    let py = screen_cy - (px - (screen_cx + x_offset));
                    if py >= offset_y && py < offset_y + height {
                        canvas.set_pin(Point::new(px, py), intensity);
                    }
                }
            };

            // 원점(screen_cx, screen_cy)을 지나는 기준선 (그래프 영역 전체, 100% 강도: Intensity::MAX)
            draw_linear_line(canvas, 0, Intensity::MAX);

            // 좌우 키로 x 오프셋이 적용된 선 추가 표현 (그래프 영역 전체, 80% 강도: 204)
            if linear_x_offset != 0 {
                draw_linear_line(canvas, linear_x_offset, Intensity::new(204));
            }
        }
        // 이차 방정식 그래프 그리기 (y = x^2)
        GraphType::QuadraticEquation => {
            draw_axes(offset_y + height - 5, screen_cx); // X축 하단, Y축 중앙
            for x in 0..width {
                let dx = (x as f32 - cx as f32) / (cx as f32); // -1.0 ~ 1.0 범위
                let dy = dx * dx;
                let y = height as f32 - 1.0 - (dy * height as f32);
                if y >= 0.0 && y < height as f32 {
                    canvas.set_pin(
                        Point::new(offset_x + x, offset_y + y as i16),
                        Intensity::new(GRAPH_SHARPNESS),
                    );
                }
            }
        }
        // 삼각 함수 그래프 그리기 (y = sin(x))
        GraphType::TrigonometricFunction => {
            draw_axes(screen_cy, screen_cx); // X축 중앙, Y축 좌측
            for x in 0..width {
                let dx = (x as f32 / width as f32) * std::f32::consts::PI * 2.0;
                let dy = dx.sin();
                let y = cy as f32 - (dy * (cy as f32 - 1.0));
                if y >= 0.0 && y < height as f32 {
                    canvas.set_pin(
                        Point::new(offset_x + x, offset_y + y as i16),
                        Intensity::new(GRAPH_SHARPNESS),
                    );
                }
            }
        }
        // 지수 함수 그래프 그리기 (y = e^x)
        GraphType::ExponentialFunction => {
            draw_axes(offset_y + height - 5, screen_cx); // X축 하단, Y축 중앙
            for x in 0..width {
                let dx = (x as f32 / width as f32) * 4.0 - 2.0; // -2.0 ~ 2.0 범위
                let dy = dx.exp();
                let y = height as f32 - 1.0 - (dy / std::f32::consts::E.powi(2) * height as f32);
                if y >= 0.0 && y < height as f32 {
                    canvas.set_pin(
                        Point::new(offset_x + x, offset_y + y as i16),
                        Intensity::new(GRAPH_SHARPNESS),
                    );
                }
            }
        }
        // 막대 차트 그리기
        GraphType::BarChart => {
            draw_axes(offset_y + height - 1, offset_x); // X축 하단, Y축 좌측
            let data = [10, 30, 20, 50, 40];
            let bar_width = width / data.len() as i16;
            let max_val = *data.iter().max().unwrap_or(&1) as f32;

            for (i, &val) in data.iter().enumerate() {
                let x_start = offset_x + i as i16 * bar_width;
                let bar_h = (val as f32 / max_val * (height as f32 - 1.0)) as i16;
                let y_start = offset_y + height - bar_h;

                Rectangle::new(
                    Point::new(x_start + 1, y_start - 1), // 막대 간격 및 Y축과 겹침 방지
                    Size::new((bar_width - 2).max(0), bar_h),
                )
                .style(Style::with_fill(Intensity::new(GRAPH_SHARPNESS)))
                .draw(canvas);
            }
        }
        // 파이 차트 그리기 (축은 생략)
        GraphType::PieChart => {
            let radius = cx.min(cy) - 2;
            let data = [30, 20, 50];
            let total: f32 = data.iter().sum::<i32>() as f32;
            let mut current_angle: f32 = 0.0;

            for y in 0..height {
                for x in 0..width {
                    let dx = x as i32 - cx as i32;
                    let dy = y as i32 - cy as i32;
                    if dx * dx + dy * dy <= (radius as i32) * (radius as i32) {
                        canvas.set_pin(
                            Point::new(offset_x + x, offset_y + y),
                            Intensity::new(GRAPH_SHARPNESS),
                        );
                    }
                }
            }

            // 구분선을 그려서 조각들을 명확하게 분할합니다. (강도 0)
            for &val in data.iter() {
                let end_x = screen_cx + (radius as f32 * current_angle.cos()) as i16;
                let end_y = screen_cy + (radius as f32 * current_angle.sin()) as i16;
                Line::new(Point::new(screen_cx, screen_cy), Point::new(end_x, end_y))
                    .style(Style::with_stroke(Intensity::MIN, 1))
                    .draw(canvas);

                let slice_angle = (val as f32 / total) * 2.0 * std::f32::consts::PI;
                current_angle += slice_angle;
            }
        }
        // 꺾은선 그래프 그리기
        GraphType::LineGraph => {
            draw_axes(offset_y + height - 1, offset_x);
            let data = [10, 30, 20, 50, 30, 60, 40];
            let step = width / (data.len() as i16 - 1).max(1);
            let max_val = *data.iter().max().unwrap_or(&1) as f32;

            let mut prev_pt: Option<Point> = None;
            for (i, &val) in data.iter().enumerate() {
                let x = offset_x + i as i16 * step;
                let y =
                    offset_y + height - 1 - (val as f32 / max_val * (height as f32 - 1.0)) as i16;
                let pt = Point::new(x, y);

                if let Some(prev) = prev_pt {
                    Line::new(prev, pt)
                        .style(Style::with_stroke(Intensity::new(GRAPH_SHARPNESS), 1))
                        .draw(canvas);
                }
                prev_pt = Some(pt);
            }
        }
    }
}

pub struct GraphApplet {
    /// 표시할 수학적 방정식 그래프 유형 목록
    pub graph_list: Vec<GraphType>,
    /// 그래프 종류 선택을 위한 1차원 ItemBar 메뉴 바 컴포넌트
    pub item_bar: ItemBar<'static>,
    /// 1차 방정식 그래프의 X 오프셋 값
    pub linear_x_offset: i16,
}

impl Default for GraphApplet {
    fn default() -> Self {
        let behavior = SelectorBehavior::default();
        Self {
            graph_list: vec![
                GraphType::LinearEquation,
                GraphType::QuadraticEquation,
                GraphType::TrigonometricFunction,
                GraphType::ExponentialFunction,
                GraphType::BarChart,
                GraphType::PieChart,
                GraphType::LineGraph,
            ],
            // Default 인스턴스를 반환하기 위해 임시 빈 ItemBar로 초기화해 둡니다.
            item_bar: ItemBar::new(Vec::new(), Box::new(Vertical), behavior),
            linear_x_offset: 0,
        }
    }
}

impl Applet for GraphApplet {
    fn on_start(&mut self, context: &mut Context) -> sdk::error::Result<()> {
        let size = context.window.get_size();
        let height = size.height;
        let item_step_y = ITEM_HEIGHT + ITEM_SPACING;
        let total_height = self.graph_list.len() as i16 * item_step_y + 4; // 4는 상하 여백
        let canvas_height = total_height.max(height);

        // 1차원 메뉴 컴포넌트의 동작 및 하이라이트 스타일 동작을 구성합니다.
        let behavior = SelectorBehavior {
            scroll_canvas_size: Some((MENU_PANEL_WIDTH, canvas_height)),
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

        // 각 아이템의 가상 렌더링 박스 개수를 결정하기 위해 더미 항목 문자열 목록을 준비합니다.
        let items = vec![""; self.graph_list.len()];

        // ItemBar 인스턴스를 새롭게 초기화합니다.
        self.item_bar = ItemBar::new(items, Box::new(Vertical), behavior);

        // 디스플레이 너비와 높이 정보를 기준으로 레이아웃 위치, 크기 및 여백 간격을 초기화합니다.
        self.item_bar.set_bounds(BoundsRect::new(Point::new(0, 0), Size::new(MENU_PANEL_WIDTH, height)));

        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> sdk::error::Result<sdk::event::UpdateResult> {
        let mut needs_redraw = false;

        let prev_index = self.item_bar.selected_index;
        let update_res = self.item_bar.on_update(context, true).unwrap_or(WidgetUpdateResult::Unchanged);
        if update_res == WidgetUpdateResult::NeedsRedraw {
            needs_redraw = true;
            if prev_index != self.item_bar.selected_index {
                self.linear_x_offset = 0; // 메뉴 변경 시 오프셋 리셋
                let graph_index = self.item_bar.selected_index;
                if let Some(graph_type) = self.graph_list.get(graph_index) {
                    context.audio.speak_text(graph_type.text(context.language));
                }
            }
        }

        // 키패드 이벤트를 처리하여 좌우 키 선택 시 linear_x_offset을 변경합니다.
        while let sdk::api::keypad::KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == sdk::api::keypad::KeyState::Pressed {
                let graph_index = self.item_bar.selected_index;
                if self.graph_list.get(graph_index) == Some(&GraphType::LinearEquation) {
                    match event.code {
                        sdk::api::keypad::KeyCode::Left => {
                            self.linear_x_offset -= 1;
                            needs_redraw = true;
                        }
                        sdk::api::keypad::KeyCode::Right => {
                            self.linear_x_offset += 1;
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        if needs_redraw {
            Ok(sdk::event::UpdateResult::NeedsRedraw)
        } else {
            Ok(sdk::event::UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> sdk::error::Result<()> {
        // 1. 현재 선택된 인덱스에 따라 수학적 방정식 그래프를 우측 공간에 렌더링합니다.
        let graph_index = self.item_bar.selected_index;
        if let Some(graph_type) = self.graph_list.get(graph_index) {
            draw_graph(canvas, *graph_type, self.linear_x_offset);
        } else {
            canvas.clear();
        }

        let display_size = canvas.get_size();
        let display_height = display_size.height;

        // 2. 좌측 메뉴 패널의 배경을 지우고 세로 방향 구분 외곽선을 그립니다.
        for j in 0..display_height {
            for i in 0..MENU_PANEL_WIDTH {
                let is_border =
                    j == 0 || j == display_height - 1 || i == 0 || i == MENU_PANEL_WIDTH - 1;
                let intensity = if is_border {
                    Intensity::MAX
                } else {
                    Intensity::MIN
                };
                canvas.set_pin(Point::new(i, j), intensity);
            }
        }

        // 3. ItemBar 컴포넌트를 이용해 뷰포트 내의 모든 메뉴 아이템 상자 및 선택된 상태 효과를 렌더링합니다.
        let _ = self.item_bar.on_draw(canvas);

        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let app = Box::new(GraphApplet::default());
    sdk::applet::run(app);
}

