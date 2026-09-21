use sdk::api::display::Point;

use crate::{Draw, Graphics, style::Style};

pub struct Circle {
    pub center: Point,
    pub diameter: i16,
    pub style: Style,
}

impl Circle {
    pub fn new(top_left: Point, diameter: i16) -> Self {
        let radius = diameter / 2;
        let center = Point::new(top_left.x + radius, top_left.y + radius);
        Self {
            center,
            diameter,
            style: Style::default(),
        }
    }

    pub fn with_center(center: Point, diameter: i16) -> Self {
        Self {
            center,
            diameter,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Draw for Circle {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized)) {
        graphics.draw_circle(self.center, self.diameter, self.style);
    }
}
