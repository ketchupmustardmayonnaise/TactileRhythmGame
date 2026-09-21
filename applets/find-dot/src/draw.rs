use graphics::{Circle, Draw};
use sdk::api::display::{DisplayInterface, Intensity, Point};

use crate::game::PROGRESS_BAR_HEIGHT;

pub fn draw_blinking_dot(canvas: &mut dyn DisplayInterface, position: Point, visible: bool) {
    let intensity = if visible {
        Intensity::MAX
    } else {
        Intensity::MIN
    };
    // 5x5 크기의 십자가 형태로 점 표시를 그립니다.
    // 가로선 그리기 (중심 기준 좌우로 2핀씩)
    for dx in -2_i16..=2_i16 {
        canvas.set_pin(Point::new(position.x + dx, position.y), intensity);
    }
    // 세로선 그리기 (중심 기준 위아래로 2핀씩, 중심점은 가로선에서 그렸으므로 제외)
    for dy in -2_i16..=2_i16 {
        if dy != 0 {
            canvas.set_pin(Point::new(position.x, position.y + dy), intensity);
        }
    }
}

pub fn draw_cursor(canvas: &mut dyn DisplayInterface, position: Point) {
    Circle::with_center(position, 4).draw(canvas);
}

pub fn draw_progress_bar(canvas: &mut dyn DisplayInterface, ratio: f32) {
    let size = canvas.get_size();
    let progress_y = size.height - PROGRESS_BAR_HEIGHT;
    let progress_width = (size.width as f32 * ratio) as i16;
    for y in progress_y..size.height {
        for x in 0..progress_width {
            canvas.set_pin(Point::new(x, y), Intensity::MAX);
        }
    }
}
