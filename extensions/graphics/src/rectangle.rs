use sdk::api::display::{Point, Size};

use crate::{Draw, Graphics, style::Style};

#[derive(Debug, Clone, Copy, Default)]
pub struct Rectangle {
    pub top_left: Point,
    pub size: Size,
    pub style: Style,
}

impl Rectangle {
    pub fn new(point: Point, size: Size) -> Self {
        Self {
            top_left: point,
            size,
            style: Style::default(),
        }
    }
}

impl Rectangle {
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn x(&self) -> i16 {
        self.top_left.x
    }

    pub fn y(&self) -> i16 {
        self.top_left.y
    }

    pub fn width(&self) -> i16 {
        self.size.width
    }

    pub fn height(&self) -> i16 {
        self.size.height
    }
}

impl Draw for Rectangle {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized)) {
        graphics.draw_rectangle(self.top_left, self.size, self.style);
    }
}
