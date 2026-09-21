// applets/tactile-game/src/guide.rs

use crate::game::TactileGame;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::api::keypad::KeyCode;
use sdk::tts_segment;

use graphics::{
    Draw, Rectangle,
    style::{StrokeAlignment, Style},
};

// === 촉각 그래픽 전용 화면 구성 상수 ===
const QUIZ_SIZE: i16 = 15; // 퀴즈 박스 크기
const TIMER_BAR_HEIGHT: i16 = 1; // 타이머바 높이
const ITEM_SPACING: i16 = 2; // 패널 간격 오프셋
const BAR_START_X: i16 = 1; // 가로 시작 마진
const BAR_START_Y: i16 = 2; // 세로 시작 마진

/// 1~5 단계의 진동 세기를 신체 피드백 감각에 매핑하기 위한 헬퍼 함수입니다.
/// 설명하는 부분이 눈에 띄도록 주파수 비율과 진동 깊이를 세분화합니다.
fn get_level_intensity(level: u8) -> Intensity {
    match level {
        1 => Intensity::new_blink(36),
        2 => Intensity::new_blink(72),
        3 => Intensity::new_blink(109),
        4 => Intensity::new_blink(145),
        5 => Intensity::new_blink(182),
        _ => Intensity::MIN, // 잘못 설정된 패턴은 꺼짐(무진동) 처리
    }
}

/// 가이드 단계를 안전하게 갱신하고, 새로 진입한 단계에 알맞은 한글 음성 나레이션을 재생합니다.
pub fn set_guide_step(game: &mut TactileGame, step: u8, context: &mut Context) {
    game.guide_step = step;
    game.tick = 0; // [변경] 가이드 단계가 바뀔 때마다 틱을 0으로 동기화하여 깜빡임 시작 타이밍을 깔끔하게 일치시킵니다.

    // 단계별 가이드 안내 다국어(한국어 위주) 나레이션 문장 빌드
    let tts_texts = match step {
        1 => tts_segment!(
            "지금부터 점자 촉지게임 테스트 설명을 시작하겠습니다.",
            "설명하는 부분이 깜박이고, 중앙키를 누르면 다음으로 넘어갑니다.",
            "이전 설명을 다시 듣고 싶다면 기능키를 눌러주세요.",
            "테스트를 시작하고 싶으시면 메뉴키를 눌러주세요."
        ),
        2 => tts_segment!(
            "화면 위쪽에는 큰 사각형이 있어요.",
            "이 사각형 안에서 점들이 일정한 빠르기로 움직여요."
        ),
        3 => tts_segment!(
            "화면 아래쪽에는 작은 사각형이 3개 있어요.",
            "이 사각형들 안에서도 점들이 움직이는데, 각각 빠르기가 달라요."
        ),
        4 => tts_segment!("아래쪽 사각형은 왼쪽부터 차례로 1번, 2번, 3번이에요."),
        5 => tts_segment!("맨 아래의 타이머바는 1분동안 줄어들어요."),
        6 => tts_segment!(
            "먼저, 왼손이나 오른손 손가락으로 위쪽 큰 사각형을 만져서, 점이 움직이는 빠르기를 느껴 보세요."
        ),
        7 => tts_segment!(
            "그 다음, 아래쪽에 있는 3개의 사각형을 하나씩 만져 보세요.",
            "그 중에서 위쪽 사각형과 똑같은 빠르기로 움직이는 사각형을 찾은 다음 사각형의 번호를 말해보세요."
        ),
        8 => tts_segment!(
            "가이드화면이 끝났어요 문제는 모두 10개예요.",
            "준비되면 중앙키를 눌러주세요.",
            "다시 들으시려면 기능키를 눌러주세요."
        ),
        _ => Vec::new(),
    };

    let guide_tts = tts_texts
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();

    // 오디오 출력 채널을 통해 설명 음성을 순차 낭독합니다.
    context
        .audio
        .play_audio_sequence(&guide_tts, sdk::applet::SpeechOption::new());
}

/// 가이드 화면 진행 도중 수집된 키패드 입력을 흐름 설계에 맞게 완벽 제어합니다.
pub fn handle_guide_key_event(game: &mut TactileGame, code: KeyCode, context: &mut Context) {
    match code {
        KeyCode::Center => {
            if game.guide_step < 8 {
                // 1~7단계에서는 중앙키 입력 시 순차적으로 다음 가이드 단계로 전이합니다.
                set_guide_step(game, game.guide_step + 1, context);
            } else {
                // 마지막 8단계에서 중앙키를 누르면 실전 훈련(Playing)으로 상태가 바뀝니다.
                // [변경] 기존 로컬에 중복 정의되어 있던 함수를 지우고, game 객체의 start_real_game 메서드를 호출하여 흐름 제어를 통일합니다.
                game.start_real_game(context);
            }
        }
        KeyCode::Menu => {
            // 어느 가이드 단계에서든 메뉴키를 누르면 즉시 가이드를 탈출하고 실전 게임을 가동합니다.
            // [변경] 기존 로컬에 중복 정의되어 있던 함수를 지우고, game 객체의 start_real_game 메서드를 호출하여 흐름 제어를 통일합니다.
            game.start_real_game(context);
        }
        KeyCode::Function => {
            // 가이드 처음부터 다시 청취 가능
            if game.guide_step == 8 {
                set_guide_step(game, 1, context);
            } else {
                set_guide_step(game, game.guide_step - 1, context);
            }
        }
        _ => {}
    }
}

/// 가이드 단계(1~8단계) 및 틱 타이머에 기반해, 각 부품들의 점자 디바이스 촉각 드로잉을 총괄합니다.
pub fn draw_guide_screen(display: &mut dyn DisplayInterface, game: &TactileGame) {
    display.clear();
    let size = display.get_size();

    // 1. 크기 계산 및 레이아웃 배치 앵커 산정
    let item_size = size.width / 5;
    let item_height = item_size;
    let sector_width = size.width / 3;
    let menu_panel_height = item_height + ITEM_SPACING + TIMER_BAR_HEIGHT;

    // 상단 구역: 퀴즈 출제 박스 (Quiz Box) 시작 좌표
    let quiz_x = (size.width - QUIZ_SIZE) / 2;
    let quiz_y = (size.height - QUIZ_SIZE - menu_panel_height) / 2;

    // 하단 구역: 3지선다 선택지 패널 사각형들의 수직 시작 오프셋
    let start_y = size.height - item_height - TIMER_BAR_HEIGHT - BAR_START_Y;
    let panel_bottom_y = start_y + item_height - 1;

    // 2. 깜박임 유효 틱 연산 (약 0.5초 주기의 전형적인 깜빡임 구현)
    let is_blink_on = (game.tick / 15) % 2 == 0;

    // === [퀴즈 박스 렌더링 논리] ===
    let draw_quiz_border = match game.guide_step {
        2 => is_blink_on, // 2단계: 퀴즈 박스 소개 (깜박임 작동)
        _ => true,        // 이외 단계: 항상 점등 상태로 켜둠
    };

    if draw_quiz_border {
        let style = Style::with_stroke(Intensity::MAX, 1).stroke_alignment(StrokeAlignment::Inside);
        Rectangle::new(Point::new(quiz_x, quiz_y), Size::new(QUIZ_SIZE, QUIZ_SIZE))
            .style(style)
            .draw(display);
    }

    // 6단계와 7단계에서는 퀴즈박스 내부에 get_level_intensity(2) 촉각 진동 패턴 표시
    if game.guide_step == 6 || game.guide_step == 7 {
        let pattern_intensity = get_level_intensity(2);
        for y in quiz_y + 1..quiz_y + QUIZ_SIZE - 1 {
            for x in quiz_x + 1..quiz_x + QUIZ_SIZE - 1 {
                if x >= 0 && x < size.width && y >= 0 && y < size.height {
                    display.set_pin(Point::new(x, y), pattern_intensity);
                }
            }
        }
    }

    // === [3개 선택패널 렌더링 논리] ===
    for idx in 0..3 {
        let x_start = BAR_START_X + (idx as i16 * sector_width) + (sector_width - item_size) / 2;

        let draw_panel_border = match game.guide_step {
            3 => is_blink_on, // 3단계: 선택패널 3개 소개 (모두 동시에 깜박임)
            4 => {
                // 4단계: 왼쪽부터 1번, 2번, 3번이 차례대로 일정한 간격으로 깜박임을 수행하고, 다른 사각형들은 사라지지 않고 상시 켜진 상태를 유지합니다.
                let sub_tick = game.tick % 100;
                if sub_tick < 25 {
                    // 0 ~ 24틱: 1번째 사각형(idx == 0)만 깜박이고, 2/3번째 사각형은 항상 표시합니다.
                    if idx == 0 {
                        (sub_tick / 5) % 2 == 0
                    } else {
                        true
                    }
                } else if (25..50).contains(&sub_tick) {
                    // 25 ~ 49틱: 2번째 사각형(idx == 1)만 깜박이고, 1/3번째 사각형은 항상 표시합니다.
                    if idx == 1 {
                        (sub_tick / 5) % 2 == 0
                    } else {
                        true
                    }
                } else if (50..75).contains(&sub_tick) {
                    // 50 ~ 74틱: 3번째 사각형(idx == 2)만 깜박이고, 1/2번째 사각형은 항상 표시합니다.
                    if idx == 2 {
                        (sub_tick / 5) % 2 == 0
                    } else {
                        true
                    }
                } else {
                    // 75 ~ 99틱 (텀): 모든 사각형이 깜박임 없이 화면에 온전히 표시됩니다.
                    true
                }
            }
            _ => true, // 이외 단계: 선택패널 테두리는 상시 켜둠
        };

        // 선택패널 외곽 테두리 렌더링
        if draw_panel_border {
            // 사각형의 가로축 라인 복제 드로잉
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

            // 사각형의 세로축 라인 복제 드로잉
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
        }

        // 7단계에서는 아래 3개 사각형에 각각 5단계, 2단계, 4단계 진동 채우기 수행
        if game.guide_step == 7 {
            let fill_level = match idx {
                0 => 5,
                1 => 2,
                2 => 4,
                _ => 0,
            };
            let fill_intensity = get_level_intensity(fill_level);

            for y in start_y + 1..panel_bottom_y {
                for x in x_start + 1..x_start + item_size - 1 {
                    if x >= 0 && x < size.width && y >= 0 && y < size.height {
                        display.set_pin(Point::new(x, y), fill_intensity);
                    }
                }
            }
        }
    }

    // === [하단 타이머바 렌더링 논리] ===
    // 5단계 소개 시 타이머바가 선명하게 깜박이며, 이외 단계에서는 그리지 않거나 대기 유지
    if game.guide_step == 5 && is_blink_on {
        let start_y_timer = size.height - TIMER_BAR_HEIGHT;
        if start_y_timer >= 0 {
            let start_x = 2;
            let end_x = size.width - 3;
            for y in start_y_timer..size.height {
                for x in start_x..=end_x {
                    if x >= 0 && x < size.width && y >= 0 && y < size.height {
                        display.set_pin(Point::new(x, y), Intensity::MAX);
                    }
                }
            }
        }
    }
}
