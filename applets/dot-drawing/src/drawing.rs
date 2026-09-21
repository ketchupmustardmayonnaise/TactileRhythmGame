use graphics::style::Style;
use graphics::{Draw, Graphics, Line, Rectangle};
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::error::Result;

use crate::game::Game;
use crate::types::EditorMode;

/// 캔버스의 특정 좌표 점을 현재 모드에 따라 수정합니다.
pub fn plot_pixel(game: &mut Game, cx: i16, cy: i16) {
    let intensity = match game.mode {
        EditorMode::Draw => Intensity::MAX,
        EditorMode::Erase => Intensity::MIN,
    };

    // 그래픽 라이브러리가 0(Intensity::MIN)을 투명으로 간주하여 무시하는 현상을
    // 방지하기 위해, 캔버스의 픽셀에 직접 접근하여 값을 덮어씁니다.
    if cx >= 0 && cy >= 0 && cx < game.canvas.width as i16 && cy < game.canvas.height as i16 {
        game.canvas.set_pixel(cx as usize, cy as usize, intensity);
    }
}

/// 굵은 선(Thick Line) 알고리즘을 사용하여 두 점 사이를 렌더링 최적화하여 연결합니다.
pub fn draw_connected_line(game: &mut Game, x0: i16, y0: i16, x1: i16, y1: i16) {
    let intensity = match game.mode {
        EditorMode::Draw => Intensity::MAX,
        EditorMode::Erase => Intensity::MIN,
    };

    let canvas_width = game.canvas.width as i32;
    let canvas_height = game.canvas.height as i32;

    Line::bresenham_line(x0, y0, x1, y1, |x, y| {
        if x as i32 >= 0 && y as i32 >= 0 && (x as i32) < canvas_width && (y as i32) < canvas_height
        {
            game.canvas.set_pixel(x as usize, y as usize, intensity);
        }
    });
}

/// 게임 화면을 그립니다.
/// 현재 커서 위치에 십자 모양을 그립니다.
pub fn draw(game: &Game, canvas: &mut dyn DisplayInterface) -> Result<()> {
    canvas.clear(); // 화면을 검은색으로 지웁니다.

    // [뷰포트 컬링]
    // 전체 캔버스를 순회하는 대신, 현재 화면(Screen)에 해당하는 좌표만 순회합니다.
    // 화면 좌표(0..width, 0..height) -> 절대 좌표(World)로 역산하여 픽셀 유무 확인.

    // game 전체를 캡처하면 display(가변)와 충돌하므로 필요한 필드만 분리해서 캡처합니다.
    let viewport = &game.viewport;
    let pixel_buffer = &game.canvas;

    let active_pixels = viewport.iter_visible_pixels(pixel_buffer);

    canvas.draw_iter(active_pixels);

    // 캔버스 테두리 그리기 (3픽셀 두께, 바깥쪽으로 정렬하여 그림 영역 침범 방지)
    let canvas_w = pixel_buffer.width as i16;
    let canvas_h = pixel_buffer.height as i16;

    let border_style = Style::with_stroke(Intensity::MAX, game.config.view.canvas_border_width)
        .stroke_alignment(graphics::style::StrokeAlignment::Outside);

    let screen_origin = viewport.world_to_screen(Point::new(0, 0));
    Rectangle::new(screen_origin, Size::new(canvas_w, canvas_h))
        .style(border_style)
        .draw(canvas);

    // 커서는 화면 중앙(혹은 뷰포트가 비추는 위치)에 고정되어 보입니다.
    // game.physics_state.pos는 World 좌표이므로 정수로 변환 후 world_to_screen으로 변환합니다.
    let cursor_pos = Point::new(
        game.physics_state.pos.0 as i16,
        game.physics_state.pos.1 as i16,
    );
    let screen_cursor = game.viewport.world_to_screen(cursor_pos);
    let cx = screen_cursor.x;
    let cy = screen_cursor.y;

    // 모드와 무관하게 십자 형태의 커서를 그립니다.
    if game.cursor_visible {
        // 중앙 및 상하좌우 1px씩 찍어 십자(➕) 모양 생성
        canvas.set_pin(Point::new(cx, cy), Intensity::MAX);
        canvas.set_pin(Point::new(cx - 1, cy), Intensity::MAX);
        canvas.set_pin(Point::new(cx + 1, cy), Intensity::MAX);
        canvas.set_pin(Point::new(cx, cy - 1), Intensity::MAX);
        canvas.set_pin(Point::new(cx, cy + 1), Intensity::MAX);
    }
    Ok(())
}
