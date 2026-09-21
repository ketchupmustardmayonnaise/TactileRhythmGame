use sdk::api::display::{DisplayInterface, Intensity, Point};

use crate::types::{ALL_DIRECTIONS, Direction, GameMode};
use braille::CELL_HEIGHT;

const SQUARE_SIZE: i16 = 1;
const SQUARE_GAP: i16 = 0;
const SQUARE_FILL_INSET: i16 = 0;

const END_LINE_SPACING: i16 = 2;

pub const PROGRESS_BAR_HEIGHT: i16 = 2;
pub const PROGRESS_BAR_GAP: i16 = 1;

fn centered_text_x(display_width: i16, text: &str) -> i16 {
    (display_width - braille::measure_text(text)) / 2
}

fn square_position(direction: Direction, game_center: Point) -> Point {
    let step = SQUARE_SIZE + SQUARE_GAP;
    let cx = game_center.x - SQUARE_SIZE / 2;
    let cy = game_center.y - SQUARE_SIZE / 2;

    match direction {
        Direction::Center => Point::new(cx, cy),
        Direction::Up => Point::new(cx, cy - step),
        Direction::Down => Point::new(cx, cy + step),
        Direction::Left => Point::new(cx - step, cy),
        Direction::Right => Point::new(cx + step, cy),
    }
}

fn draw_square_fill(canvas: &mut dyn DisplayInterface, top_left: Point) {
    for dy in SQUARE_FILL_INSET..(SQUARE_SIZE - SQUARE_FILL_INSET) {
        for dx in SQUARE_FILL_INSET..(SQUARE_SIZE - SQUARE_FILL_INSET) {
            canvas.set_pin(Point::new(top_left.x + dx, top_left.y + dy), Intensity::MAX);
        }
    }
}

pub fn draw_cross_pattern(
    canvas: &mut dyn DisplayInterface,
    game_center: Point,
    active: Option<Direction>,
) {
    for dir in ALL_DIRECTIONS {
        let pos = square_position(dir, game_center);
        if active != Some(dir) {
            draw_square_fill(canvas, pos);
        }
    }
}

pub fn draw_mode_menu(canvas: &mut dyn DisplayInterface, selected_mode: GameMode) {
    let size = canvas.get_size();
    let section_h = size.height / 3;
    let margin_x = 1;

    let (fill_start_y, fill_end_y) = match selected_mode {
        GameMode::Tutorial => (0, section_h),
        GameMode::Length => (section_h, section_h * 2),
        GameMode::Speed => (section_h * 2, size.height),
    };

    for y in fill_start_y..fill_end_y {
        for x in margin_x..(size.width - margin_x) {
            canvas.set_pin(Point::new(x, y), Intensity::MAX);
        }
    }
}

pub fn draw_end_screen(canvas: &mut dyn DisplayInterface, message: &str, score: u32) {
    let size = canvas.get_size();
    let score_text = format!("{}점", score);
    let total_height = CELL_HEIGHT * 2 + END_LINE_SPACING;
    let start_y = (size.height - total_height) / 2;

    let msg_x = centered_text_x(size.width, message);
    braille::render_text(canvas, message, Point::new(msg_x, start_y));

    let score_y = start_y + CELL_HEIGHT + END_LINE_SPACING;
    let score_x = centered_text_x(size.width, &score_text);
    braille::render_text(canvas, &score_text, Point::new(score_x, score_y));
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
