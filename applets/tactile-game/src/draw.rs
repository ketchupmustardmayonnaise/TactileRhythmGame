// applets/tactile-game/src/draw.rs

use crate::game::TactileGame;
use widget::item_selector::{Horizontal, ItemBar, LayoutStrategy, SelectorBehavior};
use widget::core::Widget;
use graphics::{
    Draw, Rectangle,
    style::{StrokeAlignment, Style},
};
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity, Point, Size};

// === 사용자 설정 가능 촉각 그래픽 상수 ===
pub const QUIZ_SIZE: i16 = 15; // 퀴즈 출제 박스의 가로세로 크기
pub const TIMER_BAR_HEIGHT: i16 = 1; // 타이머 바의 높이 (두께)
pub const ITEM_BORDER_WIDTH: i16 = 1; // 객관식 바 아이템 선택 시 외곽 테두리 두께
pub const ITEM_SPACING: i16 = ITEM_BORDER_WIDTH * 2; // 객관식 바 항목 간의 여백 간격

// === 사용자 지정 아이템바 레이아웃 오프셋 ===
pub const BAR_START_X: i16 = 1; // 아이템바 그리기 가로 시작 오프셋
pub const BAR_START_Y: i16 = 2; // 아이템바 그리기 세로 시작 오프셋

/// [1] 게임 시작 대기(Ready) 상태의 메인 화면 렌더러
/// 5지선다형 무한 훈련을 상징하는 5개의 빈 박스를 가로로 단정하게 그립니다.
pub fn draw_ready_screen(display: &mut dyn DisplayInterface, _game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    // 전체 화면 가로 너비를 5등분한 값을 아이템의 가로 및 세로 크기로 동일하게 설정합니다.
    let item_width = size.width / 5;
    let item_height = item_width;

    // 5개의 보기가 있음을 암시하는 5칸의 정렬 바 생성
    let items = vec!["", "", "", "", ""];
    let behavior = SelectorBehavior {
        scroll_canvas_size: None,
        layout_strategy: LayoutStrategy::FixedItemSize {
            width: item_width,
            height: item_height,
        },
        selection_style: None,
    };

    let mut bar = ItemBar::new(items, Box::new(Horizontal), behavior);
    bar.set_bounds(BoundsRect::new(Point::new(0, 0), Size::new(size.width, size.height)));

    // 5개의 예쁜 빈 상자(테두리) 기본 렌더링
    let _ = bar.on_draw(display);
}

/// [2] 본격 게임 플레이(Playing) 화면 렌더러
/// 상단에는 퀴즈 박스, 하단에는 5지선다 선택지가 실시간 연출(정오답 피드백)과 함께 렌더링됩니다.
pub fn draw_play_screen(display: &mut dyn DisplayInterface, game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    // 전체 화면 가로 너비를 5등분한 개별 칸의 실가로폭을 계산하고 세로 높이에도 동일하게 적용합니다.
    let item_width = size.width / 5;
    let item_height = item_width;

    // 런타임에 동적으로 변경되는 메뉴 패널의 높이를 계산합니다. (아이템높이 + 여백 + 타이머높이)
    let menu_panel_height = item_height + ITEM_SPACING + TIMER_BAR_HEIGHT;

    // 2.1 상단 영역: 퀴즈 출제 박스 (Quiz Box) 렌더링
    let quiz_x = (size.width - QUIZ_SIZE) / 2;
    let quiz_y = (size.height - QUIZ_SIZE - menu_panel_height) / 2;

    // graphics::Rectangle 컴포넌트를 사용하여 퀴즈 박스 드로잉
    let mut style = Style::with_stroke(Intensity::MAX, 1).stroke_alignment(StrokeAlignment::Inside);

    if let Some(ref quiz) = game.quiz {
        // 퀴즈 자체의 고유 물리 진동 강도를 일관되게 렌더링합니다. (0~15 물리 레벨 -> 0~255 강도 변환)
        let quiz_intensity = quiz.answer * 17;
        style.fill_intensity = Some(Intensity::new(quiz_intensity));
    }

    Rectangle::new(Point::new(quiz_x, quiz_y), Size::new(QUIZ_SIZE, QUIZ_SIZE))
        .style(style)
        .draw(display);

    // 2.2 하단 영역: 5지선다 객관식 선택지 바 (Answer Bar)
    // 타이머 바의 높이와 사용자 지정 세로 오프셋(BAR_START_Y)을 반영하여 시작 좌표를 구합니다.
    let start_y = size.height - item_height - TIMER_BAR_HEIGHT - BAR_START_Y;
    let panel_bottom_y = start_y + item_height - 1;

    if let Some(ref bar) = game.answer_bar {
        // 사용자 지정 가로 오프셋 상수를 적용합니다.
        let offset_x = BAR_START_X;

        // 각 5개의 선택지 칸 내부에 실제 퀴즈의 진동 강도 후보군을 채워 넣습니다.
        // 플레이어는 손가락으로 각 칸을 쓸어가며 다른 진동 강도를 직접 비교하고 정답을 탐색할 수 있습니다.
        for idx in 0..5 {
            let x_start = offset_x + (idx as i16 * item_width);
            let intensity_level = if let Some(ref quiz) = game.quiz {
                // 퀴즈 구조체 내의 0~15 강도 후보군에 17을 곱하여 0~255 스케일의 물리 세기로 변환합니다.
                quiz.vibration_levels[idx] * 17
            } else {
                // 퀴즈 정보가 준비되지 않은 비상 대피용 균등 진동 분배
                (idx as u8) * 51
            };
            let level_intensity = Intensity::new(intensity_level);

            // 개별 아이템 박스의 외곽 1px 테두리 안쪽(알맹이 영역)에 고유 물리 강도를 가득 세팅합니다.
            for y in start_y + 1..panel_bottom_y {
                for x in x_start + 1..x_start + item_width - 1 {
                    if x >= 0 && x < size.width && y >= 0 && y < size.height {
                        display.set_pin(Point::new(x, y), level_intensity);
                    }
                }
            }
        }

        // 사용자가 현재 방향키로 포커스한(선택한) 칸에 대해 최대 진동 강도로 두꺼운 테두리를 오버레이하여 강조합니다.
        let selected_idx = bar.selected_index;
        let target_x = offset_x + (selected_idx as i16 * item_width);

        // 지정된 외곽선 두께(ITEM_BORDER_WIDTH)를 기준으로 테두리를 채웁니다.
        for y in start_y..=panel_bottom_y {
            for x in target_x..target_x + item_width {
                let offset_x = x - target_x;
                let offset_y = y - start_y;
                let is_thick_border = offset_x < ITEM_BORDER_WIDTH
                    || offset_x >= item_width - ITEM_BORDER_WIDTH
                    || offset_y < ITEM_BORDER_WIDTH
                    || offset_y >= item_height - ITEM_BORDER_WIDTH;

                if is_thick_border && x >= 0 && x < size.width && y >= 0 && y < size.height {
                    display.set_pin(Point::new(x, y), Intensity::MAX);
                }
            }
        }

        // 정답 제출 피드백 연출 프레임 시에는 포커스된 칸에 깜빡임/꺼짐 햅틱 효과를 덧씌웁니다.
        if let Some(correct) = game.feedback_correct {
            let intensity_val = if correct {
                // 정답일 경우 인지하기 쉽도록 격렬하고 경쾌하게 깜빡여 줍니다.
                if (game.tick / 3) % 2 == 0 { 255 } else { 0 }
            } else {
                // 오답일 경우 순간적으로 모든 진동을 죽여 침묵(0) 형태의 묵직한 경고를 줍니다.
                0
            };

            let feedback_intensity = Intensity::new(intensity_val);

            // 포커스된 칸 내부의 알맹이 영역(외곽선 두께 안쪽)에 피드백 물리 효과를 실시간 연출합니다.
            for y in start_y + ITEM_BORDER_WIDTH..=panel_bottom_y - ITEM_BORDER_WIDTH {
                for x in target_x + ITEM_BORDER_WIDTH..target_x + item_width - ITEM_BORDER_WIDTH {
                    if x >= 0 && x < size.width && y >= 0 && y < size.height {
                        display.set_pin(Point::new(x, y), feedback_intensity);
                    }
                }
            }
        }
    }

    // 시간 타이머 바 (Timer Bar) 렌더링 연계 적용
    draw_timer_bar(display, game);
}

/// [3] 실시간 타이머 바 (Timer Bar) 렌더러
/// 하단 객관식 선택 바 바로 위의 수직 공간에 가로 방향 게이지 바를 정교하게 그립니다.
/// 시각장애인 플레이어가 전체 남은 시간을 촉각적으로 가늠할 수 있도록, 양 끝단에 가이드 경계 도트를 은은하게 켜줍니다.
pub fn draw_timer_bar(display: &mut dyn DisplayInterface, game: &TactileGame) {
    let size = display.get_size();

    // 타이머 바가 그려질 수직 y 좌표 (화면 최하단 영역)
    let start_y = size.height - TIMER_BAR_HEIGHT;
    if start_y < 0 {
        return; // 세로 공간이 극히 부족한 특수 해상도의 경우 그리기 생략
    }

    // 좌우 여백 2픽셀씩 안전 마진을 고려한 타이머 바의 가로 유효 영역 계산
    let start_x = 2;
    let end_x = size.width - 3;
    let bar_width = end_x - start_x + 1;

    if bar_width <= 0 {
        return;
    }

    // 현재 남은 제한시간 비율 연산 (0.0 ~ 1.0 범위로 바인딩)
    let ratio = (game.timer_current / game.timer_max).clamp(0.0, 1.0);
    let filled_width = (ratio * bar_width as f32).round() as i16;

    // 타이머 가로 게이지 바를 한 픽셀씩 세밀하게 채색 렌더링 (TIMER_BAR_HEIGHT 높이만큼 그리기 적용)
    for y in start_y..size.height {
        for x in start_x..=end_x {
            let is_filled = x < start_x + filled_width;
            let is_edge = x == start_x || x == end_x;

            let intensity = if is_filled {
                Intensity::MAX // 남은 시간 비율 부분은 100% 강력한 진동 전달
            } else if is_edge {
                Intensity::new(6) // 전체 타이머 범위를 만져서 파악할 수 있도록 양 끝단 도트에 은은한 햅틱 가이드라인 부여
            } else {
                Intensity::MIN // 시간 소진된 영역은 무진동(꺼짐) 상태 유지
            };

            display.set_pin(Point::new(x, y), intensity);
        }
    }
}

/// [4] 일시정지(Stop) 상태 화면 렌더러
/// 일시정지를 상징하는 커다란 일시정지 기호(|| 모양)를 화면 중앙에 촉각적으로 출력합니다.
pub fn draw_stop_screen(display: &mut dyn DisplayInterface, _game: &TactileGame) {
    display.clear();

    let size = display.get_size();
    let bar_w = 4;
    let bar_h = 12;
    let gap = 4;

    let center_x = size.width / 2;
    let center_y = size.height / 2;

    // 왼쪽 일시정지 바
    let left_x = center_x - bar_w - (gap / 2);
    let bar_y = center_y - (bar_h / 2);

    for j in bar_y..bar_y + bar_h {
        for i in left_x..left_x + bar_w {
            display.set_pin(Point::new(i, j), Intensity::MAX);
        }
    }

    // 오른쪽 일시정지 바
    let right_x = center_x + (gap / 2);
    for j in bar_y..bar_y + bar_h {
        for i in right_x..right_x + bar_w {
            display.set_pin(Point::new(i, j), Intensity::MAX);
        }
    }
}

/// 5. 모든 리셋 초기화(Reset) 확인용 보안 화면 렌더러
pub fn draw_reset_screen(display: &mut dyn DisplayInterface, game: &TactileGame) {
    display.clear();
    // 경고를 알리는 테두리가 좁혀지는 역동적 루프 연출
    let size = display.get_size();
    let step = (game.tick / 6).rem_euclid(3);

    for y in 0..size.height {
        for x in 0..size.width {
            let is_warning_pixel =
                x == step || x == size.width - 1 - step || y == step || y == size.height - 1 - step;
            if is_warning_pixel {
                display.set_pin(Point::new(x, y), Intensity::MAX);
            }
        }
    }

    // 물음표(?) 촉각 심볼을 정가운데에 단단하게 드로잉
    let center_x = size.width / 2;
    let center_y = size.height / 2;

    let q_dots = [
        Point::new(center_x - 1, center_y - 3),
        Point::new(center_x, center_y - 3),
        Point::new(center_x + 1, center_y - 3),
        Point::new(center_x + 1, center_y - 2),
        Point::new(center_x, center_y - 1),
        Point::new(center_x, center_y),
        Point::new(center_x, center_y + 2), // 물음표 하단의 고독한 도트 점
    ];

    for p in q_dots.iter() {
        display.set_pin(*p, Intensity::MAX);
    }
}

/// [6] 게임 종료 (GameOver) 화면 렌더러
/// 제한시간 초과로 인한 패배를 즉각적이고 직관적으로 전달하기 위해,
/// 화면에 커다란 엑스('X') 형태의 촉각 심볼을 그리고, 지진처럼 맥박이 떨리는 햅틱 경고 진동 주기를 가미합니다.
pub fn draw_over_screen(display: &mut dyn DisplayInterface, game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    // 6.1 지진 효과 연출 (틱 주기에 입각하여 온 몸으로 느끼는 역동적인 부르르 떨림 구현)
    // 틱이 홀수 주기에 들어설 때 화면의 가장자리 테두리에 옅은 진동을 부가하여 실패의 타격감을 부여합니다.
    let is_shake = (game.tick / 4) % 2 == 0;
    let bg_intensity = if is_shake {
        Intensity::new(3) // 지진 효과를 극대화하는 은은한 베이스 라인 햅틱 감도
    } else {
        Intensity::MIN
    };

    // 화면 상하좌우 최외곽 테두리에 지진 맥박 경보 인가
    for y in 0..size.height {
        for x in 0..size.width {
            let is_border = x == 0 || x == size.width - 1 || y == 0 || y == size.height - 1;
            if is_border {
                display.set_pin(Point::new(x, y), bg_intensity);
            }
        }
    }

    // 6.2 화면 한가운데에 커다란 엑스('X')형 실패 촉각 심볼 드로잉
    let center_x = size.width / 2;
    let center_y = size.height / 2;
    let x_size = 6; // 대각선 그리기 반경 크기

    for i in -x_size..=x_size {
        // 주 대각선 (\ 방향) 및 부 대각선 (/ 방향) 교차 점 지정
        let p1 = Point::new(center_x + i, center_y + i);
        let p2 = Point::new(center_x + i, center_y - i);

        // 유효 윈도우 해상도 범위 내에서만 안전하게 핀 가동
        if p1.x >= 0 && p1.x < size.width && p1.y >= 0 && p1.y < size.height {
            display.set_pin(p1, Intensity::MAX);
        }
        if p2.x >= 0 && p2.x < size.width && p2.y >= 0 && p2.y < size.height {
            display.set_pin(p2, Intensity::MAX);
        }
    }
}

/// [7] 정답/오답(Correct/Wrong) 제출 시 대형 피드백 화면 렌더러
/// 플레이어가 제출한 답안의 채점 결과에 따라, 화면 가득 커다란 동그라미('O') 혹은 엑스('X') 심볼을 그려서 직관적인 촉각적 정오답 피드백을 제공합니다.
pub fn draw_feedback_screen(
    display: &mut dyn DisplayInterface,
    _game: &TactileGame,
    correct: bool,
) {
    display.clear();
    let size = display.get_size();

    let center_x = size.width / 2;
    let center_y = size.height / 2;

    if correct {
        // 7.1 정답인 경우: 화면 중앙에 커다란 동그라미('O') 촉각 심볼 렌더링
        // 반지름 5 정도의 원형 궤적을 픽셀 거리 수식으로 정확히 계산하여 도톰하게 켭니다.
        for y in 0..size.height {
            for x in 0..size.width {
                let dx = x - center_x;
                let dy = y - center_y;
                let dist_sq = dx * dx + dy * dy;
                // dist_sq가 약 (5-1.5)^2 ~ (5+0.5)^2 범위 내에 드는 픽셀들을 켜서 자연스러운 두께를 만듭니다.
                if (12..=30).contains(&dist_sq) {
                    display.set_pin(Point::new(x, y), Intensity::MAX);
                }
            }
        }
    } else {
        // 7.2 오답인 경우: 화면 중앙에 커다란 엑스('X')형 실패 촉각 심볼 렌더링
        let x_size = 5; // 대각선 그리기 반경 크기

        for i in -x_size..=x_size {
            // 주 대각선 (\ 방향) 및 부 대각선 (/ 방향) 교차 점 지정
            let p1 = Point::new(center_x + i, center_y + i);
            let p2 = Point::new(center_x + i, center_y - i);

            // 유효 윈도우 해상도 범위 내에서만 안전하게 핀 가동
            if p1.x >= 0 && p1.x < size.width && p1.y >= 0 && p1.y < size.height {
                display.set_pin(p1, Intensity::MAX);
            }
            if p2.x >= 0 && p2.x < size.width && p2.y >= 0 && p2.y < size.height {
                display.set_pin(p2, Intensity::MAX);
            }
        }
    }
}
