use sdk::api::display::Point;

use crate::{Draw, Graphics, style::Style};

pub struct Line {
    pub start: Point,
    pub end: Point,
    pub style: Style,
}

impl Line {
    pub fn new(start: Point, end: Point) -> Self {
        Self {
            start,
            end,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// 브레젠험(Bresenham) 알고리즘을 사용하여 두 점을 잇는 선분의 좌표를 계산하고 콜백을 호출합니다.
    // 반지름 길이를 받아다 해당 크기만큼만 선을 그리는 방식으로 수정 필요
    pub fn bresenham_line<F>(mut x0: i16, mut y0: i16, x1: i16, y1: i16, mut plot: F)
    where
        F: FnMut(i16, i16),
    {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            plot(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}

impl Draw for Line {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized)) {
        graphics.draw_line(self.start, self.end, self.style);
    }
}
