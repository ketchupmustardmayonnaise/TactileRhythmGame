// applets/tactile-game/src/draw.rs

use crate::game::TactileGame;

use graphics::{
    Draw, Rectangle,
    style::{StrokeAlignment, Style},
};
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};

// === 사용자 설정 가능 촉각 그래픽 상수 ===
pub const QUIZ_SIZE: i16 = 15; // 퀴즈 출제 박스의 가로세로 크기
pub const TIMER_BAR_HEIGHT: i16 = 1; // 타이머 바의 높이 (두께)
pub const ITEM_BORDER_WIDTH: i16 = 1; // 객관식 바 아이템 선택 시 외곽 테두리 두께
pub const ITEM_SPACING: i16 = ITEM_BORDER_WIDTH * 2; // 객관식 바 항목 간의 여백 간격

// === 사용자 지정 아이템바 레이아웃 오프셋 ===
pub const BAR_START_X: i16 = 1; // 아이템바 그리기 가로 시작 오프셋
pub const BAR_START_Y: i16 = 2; // 아이템바 그리기 세로 시작 오프셋

/// 1~5 단계의 진동 세기를 Intensity::new_blink로 매핑합니다.
/// 손가락 끝으로 확실하게 진동 빠르기와 깊이 차이를 체감할 수 있도록 물리 전압 범위(0~255)를 5단계로 크게 쪼개어 매핑합니다.
fn get_level_intensity(level: u8) -> Intensity {
    match level {
        1 => Intensity::new_blink(36),
        2 => Intensity::new_blink(72),
        3 => Intensity::new_blink(109),
        4 => Intensity::new_blink(145),
        5 => Intensity::new_blink(182),
        _ => Intensity::MIN, // 잘못된 단계는 최소 진동(꺼짐) 처리합니다.
    }
}

/// [1] 게임 시작 대기(Ready) 상태의 메인 화면 렌더러
/// 3지선다형 무한 훈련을 상징하는 3개의 빈 박스를 가로로 대칭 및 가로축 정중앙 정렬하여 그립니다.
pub fn draw_ready_screen(display: &mut dyn DisplayInterface, _game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    let item_size = size.width / 5;
    let item_height = item_size;

    // 개별 보기의 수평 안착 구역 너비를 구합니다.
    let sector_width = size.width / 5;

    // 런타임에 동적으로 변경되는 하단 패널 세로 시작 위치 계산
    let start_y = size.height - item_height - TIMER_BAR_HEIGHT - BAR_START_Y;
    let panel_bottom_y = start_y + item_height - 1;

    // 대기 화면에서 진동패턴 소개
    for idx in 0..5 {
        // 개별 구역 내에서 완벽한 가로 중앙 대칭 좌표 계산
        let x_start = BAR_START_X + (idx as i16 * sector_width) + (sector_width - item_size) / 2;

        // 각 작은 사각형의 가로 방향 경계 테두리를 그립니다.
        for x in x_start..x_start + item_size {
            if x >= 0 && x < size.width {
                if start_y >= 0 && start_y < size.height {
                    display.set_pin(Point::new(x, start_y), Intensity::MAX);
                }
                if panel_bottom_y >= 0 && panel_bottom_y < size.height {
                    display.set_pin(Point::new(x, panel_bottom_y), Intensity::MAX);
                }
            }
        }

        // 각 작은 사각형의 세로 방향 경계 테두리를 그립니다.
        for y in start_y..=panel_bottom_y {
            if y >= 0 && y < size.height {
                if x_start >= 0 && x_start < size.width {
                    display.set_pin(Point::new(x_start, y), Intensity::MAX);
                }
                let right_x = x_start + item_size - 1;
                if right_x >= 0 && right_x < size.width {
                    display.set_pin(Point::new(right_x, y), Intensity::MAX);
                }
            }
        }
        // 개별 작은 사각형의 외곽 테두리 안쪽(알맹이 영역)에 고유 물리 진동 강도를 채웁니다.
        for y in start_y + 1..panel_bottom_y {
            for x in x_start + 1..x_start + item_size - 1 {
                if x >= 0 && x < size.width && y >= 0 && y < size.height {
                    display.set_pin(Point::new(x, y), get_level_intensity(idx + 1));
                }
            }
        }
    }
}

/// [2] 본격 게임 플레이(Playing) 화면 렌더러
/// 상단에는 퀴즈 박스, 하단에는 3지선다 선택지가 정교한 상하 밸런스 배치 연출과 함께 렌더링됩니다.
pub fn draw_play_screen(display: &mut dyn DisplayInterface, game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    let item_size = size.width / 5;
    let item_height = item_size;

    // 화면 가로의 구역 너비를 구합니다.
    let sector_width = size.width / 3;

    // 런타임에 동적으로 변경되는 메뉴 패널의 높이를 계산합니다. (아이템높이 + 여백 + 타이머높이)
    let menu_panel_height = item_height + ITEM_SPACING + TIMER_BAR_HEIGHT;

    // 2.1 상단 영역: 퀴즈 출제 박스 (Quiz Box) 렌더링
    let quiz_x = (size.width - QUIZ_SIZE) / 2;
    let quiz_y = (size.height - QUIZ_SIZE - menu_panel_height) / 2;

    // graphics::Rectangle 컴포넌트를 사용하여 퀴즈 박스 드로잉
    let style = Style::with_stroke(Intensity::MAX, 1).stroke_alignment(StrokeAlignment::Inside);

    Rectangle::new(Point::new(quiz_x, quiz_y), Size::new(QUIZ_SIZE, QUIZ_SIZE))
        .style(style)
        .draw(display);

    // 정답 자체의 고유 물리 진동 강도를 일관되게 렌더링합니다.
    let answer_intensity = if let Some(ref quiz) = game.quiz {
        get_level_intensity(quiz.answer)
    } else {
        Intensity::MIN
    };
    // 퀴즈 박스 안쪽 체움
    for y in quiz_y + 1..quiz_y + QUIZ_SIZE - 1 {
        for x in quiz_x + 1..quiz_x + QUIZ_SIZE - 1 {
            if x >= 0 && x < size.width && y >= 0 && y < size.height {
                display.set_pin(Point::new(x, y), answer_intensity);
            }
        }
    }
    // 하단 영역 객관식 선택지 바 (Answer Bar)
    // 타이머 바의 높이와 사용자 지정 세로 오프셋(BAR_START_Y)을 반영하여 시작 좌표를 구합니다.
    let start_y = size.height - item_height - TIMER_BAR_HEIGHT - BAR_START_Y;
    let panel_bottom_y = start_y + item_height - 1;

    if let Some(ref _bar) = game.answer_bar {
        // 선택지 칸 내부에 실제 퀴즈의 진동 강도 후보군을 채워 넣습니다.
        for idx in 0..3 {
            // 개별 구역 내에서 완벽한 가로 중앙 대칭 좌표 계산
            let x_start =
                BAR_START_X + (idx as i16 * sector_width) + (sector_width - item_size) / 2;
            let level_intensity = if let Some(ref quiz) = game.quiz {
                // 퀴즈 구조체 내의 섞여있는 고유 진동세기 강도(1..=5)에 맞춰 대응되는 햅틱 세기를 매핑합니다.
                get_level_intensity(quiz.vibration_levels[idx])
            } else {
                // 백업용 햅틱 강도 분배
                Intensity::new((idx as u8) * 85)
            };

            // 각 작은 사각형의 가로 방향 경계 테두리를 그립니다.
            for x in x_start..x_start + item_size {
                if x >= 0 && x < size.width {
                    if start_y >= 0 && start_y < size.height {
                        display.set_pin(Point::new(x, start_y), Intensity::MAX);
                    }
                    if panel_bottom_y >= 0 && panel_bottom_y < size.height {
                        display.set_pin(Point::new(x, panel_bottom_y), Intensity::MAX);
                    }
                }
            }

            // 각 작은 사각형의 세로 방향 경계 테두리를 그립니다.
            for y in start_y..=panel_bottom_y {
                if y >= 0 && y < size.height {
                    if x_start >= 0 && x_start < size.width {
                        display.set_pin(Point::new(x_start, y), Intensity::MAX);
                    }
                    let right_x = x_start + item_size - 1;
                    if right_x >= 0 && right_x < size.width {
                        display.set_pin(Point::new(right_x, y), Intensity::MAX);
                    }
                }
            }

            // 개별 작은 사각형의 외곽 테두리 안쪽(알맹이 영역)에 고유 물리 진동 강도를 채웁니다.
            for y in start_y + 1..panel_bottom_y {
                for x in x_start + 1..x_start + item_size - 1 {
                    if x >= 0 && x < size.width && y >= 0 && y < size.height {
                        display.set_pin(Point::new(x, y), level_intensity);
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

            let intensity = if is_filled {
                Intensity::MAX // 남은 시간 비율 부분은 100% 강력한 진동 전달
            } else {
                Intensity::MIN // 시간 소진된 영역은 무진동(꺼짐) 상태 유지
            };

            display.set_pin(Point::new(x, y), intensity);
        }
    }
}
