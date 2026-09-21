use sdk::api::display::{DisplayInterface, Intensity, Point};

pub const CELL_WIDTH: i16 = 2;
pub const CELL_HEIGHT: i16 = 4;
pub const CELL_GAP: i16 = 1;

pub const CELL_GUIDE_PIN_INTENSITY: u8 = 64;

const OFFSETS: [(i16, i16); 6] = [
    (0, 0), // dot 1
    (0, 1), // dot 2
    (0, 2), // dot 3
    (1, 0), // dot 4
    (1, 1), // dot 5
    (1, 2), // dot 6
];

pub fn render_cell(canvas: &mut dyn DisplayInterface, cell: u8, pos: Point, intensity: Intensity) {
    canvas.set_pin(pos, Intensity::new(CELL_GUIDE_PIN_INTENSITY));

    for (i, (dx, dy)) in OFFSETS.iter().enumerate() {
        if cell & (1 << i) != 0 {
            // 가이드 핀 표시 위해 y축에 1 더함
            canvas.set_pin(Point::new(pos.x + dx, 1 + pos.y + dy), intensity);
        }
    }
}

pub fn render_cursor(canvas: &mut dyn DisplayInterface, pos: Point, intensity: Intensity) {
    // 커서를 가이드 핀과 그 오른쪽 핀 (총 2개의 핀)으로 표시
    canvas.set_pin(pos, intensity);
    canvas.set_pin(Point::new(pos.x + 1, pos.y), intensity);
}
