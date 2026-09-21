use sdk::api::display::{Intensity, Point};

use crate::{Draw, Graphics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
pub struct Text<'a> {
    pub text: &'a str,
    pub position: Point,
    pub font_size: u32,
    pub color: Intensity,
    pub alignment: TextAlign,
}

impl<'a> Text<'a> {
    pub fn new(text: &'a str, position: Point) -> Self {
        Self {
            text,
            position,
            font_size: 10, // 기본 폰트 사이즈
            color: Intensity::MAX,
            alignment: TextAlign::Left,
        }
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.font_size = size;
        self
    }

    pub fn color(mut self, color: Intensity) -> Self {
        self.color = color;
        self
    }

    pub fn alignment(mut self, alignment: TextAlign) -> Self {
        self.alignment = alignment;
        self
    }
}

impl<'a> Draw for Text<'a> {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized)) {
        graphics.draw_text(
            self.text,
            self.position,
            self.font_size,
            self.color,
            self.alignment,
        );
    }
}
