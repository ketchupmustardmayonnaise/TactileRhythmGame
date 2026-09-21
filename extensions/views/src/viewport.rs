use sdk::api::display::{Pin, Point};

use crate::canvas::PixelBuffer;

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub center: Point,
    pub width: i16,
    pub height: i16,
}

impl Viewport {
    pub fn new(center: Point, width: i16, height: i16) -> Self {
        Self {
            center,
            width,
            height,
        }
    }

    /// World 좌표를 Screen 좌표로 변환합니다.
    pub fn world_to_screen(&self, world_pos: Point) -> Point {
        let center_offset = Point::new(self.width / 2, self.height / 2);
        (world_pos - self.center) + center_offset
    }

    /// Screen 좌표를 World 좌표로 변환합니다.
    #[allow(dead_code)]
    pub fn screen_to_world(&self, screen_pos: Point) -> Point {
        let center_offset = Point::new(self.width / 2, self.height / 2);
        (screen_pos - center_offset) + self.center
    }

    /// 현재 화면(Viewport)에 보이는 World 좌표 영역을 반환합니다. (min_x, min_y, max_x, max_y)
    pub fn get_visible_rect(&self) -> (i16, i16, i16, i16) {
        let center_offset = Point::new(self.width / 2, self.height / 2);
        let min = (Point::new(0, 0) - center_offset) + self.center;
        let max = (Point::new(self.width, self.height) - center_offset) + self.center;
        (min.x, min.y, max.x, max.y)
    }

    /// 캔버스에서 현재 뷰포트에 보이는 픽셀들만 추출하여 스크린 좌표로 변환된 Pixel 이터레이터를 반환합니다.
    pub fn iter_visible_pixels<'a>(
        &'a self,
        canvas: &'a PixelBuffer,
    ) -> impl Iterator<Item = Pin> + 'a {
        let (min_wx, min_wy, max_wx, max_wy) = self.get_visible_rect();

        let min_x = min_wx.max(0) as usize;
        let min_y = min_wy.max(0) as usize;
        // 화면 크기보다 여유 있게(+1) 잡아서 경계선 짤림 방지
        let max_x = (max_wx + 1).max(0) as usize;
        let max_y = (max_wy + 1).max(0) as usize;

        canvas
            .iter_region(min_x, min_y, max_x, max_y)
            .map(move |(wx, wy, intensity)| {
                let world_pos = Point::new(wx as i16, wy as i16);
                let screen_pos = self.world_to_screen(world_pos);
                Pin::new(screen_pos, intensity)
            })
    }
}
