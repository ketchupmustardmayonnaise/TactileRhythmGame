use graphics::style::Style;
use graphics::{Circle, Draw, Graphics, Line, Rectangle};
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use sdk::error::Result;

use crate::game::DrawingGame;
use crate::types::EditorMode;

/// 캔버스의 특정 좌표 점을 현재 모드에 따라 수정합니다.
pub fn plot_pixel(game: &mut DrawingGame, cx: i16, cy: i16) {
    let intensity = match game.mode {
        EditorMode::Draw => Intensity::MAX,
        EditorMode::Erase => Intensity::MIN,
        EditorMode::BrushSizeChange => return, // 브러시 크기 변경 모드에서는 픽셀을 수정하지 않음
    };

    // 그래픽 라이브러리가 0(Intensity::MIN)을 투명으로 간주하여 무시하는 현상을
    // 방지하기 위해, 캔버스의 픽셀에 직접 접근하여 값을 덮어씁니다.
    for &(dx, dy) in &game.brush_mask {
        let px = cx as i32 + dx as i32;
        let py = cy as i32 + dy as i32;
        if px >= 0 && py >= 0 && px < game.canvas.width as i32 && py < game.canvas.height as i32 {
            game.canvas.set_pixel(px as usize, py as usize, intensity);
        }
    }
}

pub fn draw_connected_line(game: &mut DrawingGame, x0: i16, y0: i16, x1: i16, y1: i16) {
    let intensity = match game.mode {
        EditorMode::Draw => Intensity::MAX,
        EditorMode::Erase => Intensity::MIN,
        EditorMode::BrushSizeChange => return,
    };

    let canvas_width = game.canvas.width as i32;
    let canvas_height = game.canvas.height as i32;

    Line::bresenham_line(x0, y0, x1, y1, |x, y| {
        for &(dx, dy) in &game.brush_mask {
            let px = x as i32 + dx as i32;
            let py = y as i32 + dy as i32;
            if px >= 0 && py >= 0 && px < canvas_width && py < canvas_height {
                game.canvas.set_pixel(px as usize, py as usize, intensity);
            }
        }
    });
}

/// 게임 화면을 그립니다.
/// 현재 커서 위치에 십자 모양을 그립니다.
pub fn draw(game: &DrawingGame, canvas: &mut dyn DisplayInterface) -> Result<()> {
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

    // 모드에 따라 다른 스타일로 커서를 그림
    // let style = PrimitiveStyle::with_stroke(Gray8::WHITE, game.config.view.cursor_stroke_width);
    let style = Style::with_stroke(Intensity::MAX, game.config.view.cursor_stroke_width);

    // game.physics_state.pos는 World 좌표이므로 정수로 변환 후 world_to_screen으로 변환합니다.
    let cursor_pos = Point::new(
        game.physics_state.pos.0 as i16,
        game.physics_state.pos.1 as i16,
    );
    let screen_cursor = game.viewport.world_to_screen(cursor_pos);
    let cx = screen_cursor.x;
    let cy = screen_cursor.y;

    let r = game.brush_size;
    let size = r * 2 + 1;

    let display_size = canvas.get_size();
    let display_w = display_size.width as i32;
    let display_h = display_size.height as i32;

    // 커서가 보이지 않는 상태(깜박임)일 경우 커서를 그리지 않고 조기 반환합니다.
    if !game.cursor_visible && game.mode != EditorMode::BrushSizeChange {
        return Ok(());
    }

    // 모드에 따라 다른 모양의 커서를 그립니다.
    match game.mode {
        EditorMode::Draw => {
            // ● 브러시 모드
            for &(dx, dy) in &game.brush_mask {
                let px = cx as i32 + dx as i32;
                let py = cy as i32 + dy as i32;
                if px >= 0 && py >= 0 && px < display_w && py < display_h {
                    canvas.set_pin(Point::new(px as i16, py as i16), Intensity::MAX);
                }
            }
            // 테두리 선 그리기
            Circle::with_center(Point::new(cx, cy), size + 2)
                .style(Style::with_stroke(
                    Intensity::MIN,
                    game.config.view.cursor_stroke_width,
                ))
                .draw(canvas);

            // 중심점 표시 - 테두리 그리니까 안보임
            if cx >= 0 && cy >= 0 && (cx as i32) < display_w && (cy as i32) < display_h {
                canvas.set_pin(Point::new(cx, cy), Intensity::MAX);
            }
        }
        EditorMode::Erase => {
            // ○ 지우개 모드
            for &(dx, dy) in &game.brush_mask {
                let px = cx as i32 + dx as i32;
                let py = cy as i32 + dy as i32;
                if px >= 0 && py >= 0 && px < display_w && py < display_h {
                    canvas.set_pin(Point::new(px as i16, py as i16), Intensity::MIN);
                }
            }
            // 테두리 선
            Circle::with_center(Point::new(cx, cy), size + 2)
                .style(Style::with_stroke(
                    Intensity::MAX,
                    game.config.view.cursor_stroke_width,
                ))
                .draw(canvas);

            // 중심점 표시
            if cx >= 0 && cy >= 0 && (cx as i32) < display_w && (cy as i32) < display_h {
                canvas.set_pin(Point::new(cx, cy), Intensity::MAX);
            }
        }
        EditorMode::BrushSizeChange => {
            Circle::with_center(Point::new(cx, cy), size)
                .style(style)
                .draw(canvas);
        }
    }
    Ok(())
}
