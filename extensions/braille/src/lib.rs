mod cell;
mod encoder;
mod label;
mod text_area;

use sdk::api::display::{DisplayInterface, Intensity, Point};

use cell::render_cell;
pub use encoder::{SPACE, text_to_braille};


pub use cell::{CELL_GAP, CELL_HEIGHT, CELL_WIDTH};
pub use label::{Alignment, Label};
pub use text_area::TextArea;

pub const WORD_GAP: i16 = 2;

pub fn render_text(canvas: &mut dyn DisplayInterface, text: &str, origin: Point) {
    render_text_with_intensity(canvas, text, origin, Intensity::MAX);
}

pub fn render_text_with_intensity(
    canvas: &mut dyn DisplayInterface,
    text: &str,
    origin: Point,
    intensity: Intensity,
) {
    let cells = text_to_braille(text);
    let mut x = origin.x;
    let y = origin.y;

    for cell in &cells {
        if *cell == SPACE {
            x += WORD_GAP;
            continue;
        }
        render_cell(canvas, *cell, Point::new(x, y), intensity);
        x += CELL_WIDTH + CELL_GAP;
    }
}

pub fn render_chords(canvas: &mut dyn DisplayInterface, braille_chords: &[u8], origin: Point) {
    let mut x = origin.x;
    let y = origin.y;

    for chord in braille_chords {
        render_cell(canvas, *chord, Point::new(x, y), Intensity::MAX);
        x += CELL_WIDTH + CELL_GAP;
    }
}

pub fn measure_text(text: &str) -> i16 {
    let cells = text_to_braille(text);
    let mut width: i16 = 0;

    for cell in &cells {
        if *cell == SPACE {
            width += WORD_GAP;
        } else {
            width += CELL_WIDTH + CELL_GAP;
        }
    }

    if width > 0 {
        width -= CELL_GAP;
    }
    width
}
