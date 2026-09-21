use embedded_graphics::{
    Pixel,
    pixelcolor::{Gray8, GrayColor},
    prelude::{DrawTarget, OriginDimensions, Size},
};
use sdk::api::display::{Intensity, Point};


/// 8비트 강도(Intensity) 픽셀 버퍼
#[derive(Debug, Clone)]
pub struct PixelBuffer {
    pub width: usize,
    /// 캔버스 너비
    pub height: usize,
    /// 캔버스 높이
    pub pixels: Vec<Intensity>, // 8비트 강도 값을 온전히 저장
}

impl PixelBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Intensity::OFF; width * height],
        }
    }
    /// 특정 좌표의 픽셀 상태를 반환합니다.
    pub fn get_pixel(&self, x: usize, y: usize) -> Intensity {
        if x >= self.width || y >= self.height {
            return Intensity::OFF;
        }
        self.pixels[y * self.width + x]
    }
    /// 특정 좌표의 픽셀 상태를 설정합니다.
    pub fn set_pixel(&mut self, x: usize, y: usize, intensity: Intensity) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.pixels[y * self.width + x] = intensity;
    }
    /// 캔버스 크기를 변경합니다.
    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        let mut new_pixels = vec![Intensity::OFF; new_width * new_height];

        // 활성화된 픽셀만 순회하며 새 버퍼에 복사
        for y in 0..self.height.min(new_height) {
            for x in 0..self.width.min(new_width) {
                new_pixels[y * new_width + x] = self.get_pixel(x, y);
            }
        }
        // 기존 데이터 복사 후 크기 및 픽셀 버퍼 교체 - 새로운 크기로 변경
        self.width = new_width;
        self.height = new_height;
        self.pixels = new_pixels;
    }

    /// 켜진 픽셀들의 좌표와 강도(x, y, Intensity)를 반환하는 이터레이터
    pub fn iter_active_pixels(&self) -> impl Iterator<Item = (usize, usize, Intensity)> + '_ {
        let width = self.width;
        self.pixels
            .iter()
            .enumerate()
            .filter_map(move |(idx, &intensity)| {
                if intensity.value > 0 {
                    Some((idx % width, idx / width, intensity))
                } else {
                    None
                }
            })
    }

    /// 특정 영역(Rect) 내의 켜진 픽셀들만 반환하는 이터레이터
    /// 화면에 보이는 영역(Viewport)만 렌더링할 때 사용합니다.
    pub fn iter_region(
        &self,
        min_x: usize,
        min_y: usize,
        max_x: usize,
        max_y: usize,
    ) -> impl Iterator<Item = (usize, usize, Intensity)> + '_ {
        let width = self.width;
        // 범위 클램핑 (캔버스 밖을 참조하지 않도록)
        let start_y = min_y.min(self.height);
        let end_y = max_y.min(self.height);
        let start_x = min_x.min(self.width);
        let end_x = max_x.min(self.width);

        (start_y..end_y).flat_map(move |y| {
            let row_offset = y * width; // 한 행의 시작 비트 인덱스
            (start_x..end_x).filter_map(move |x| {
                let intensity = self.pixels[row_offset + x];
                if intensity.value > 0 {
                    Some((x, y, intensity))
                } else {
                    None
                }
            })
        })
    }
}

impl sdk::api::display::DisplayInterface for PixelBuffer {
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        if point.x >= 0 && point.y >= 0 {
            self.set_pixel(point.x as usize, point.y as usize, intensity);
        }
    }

    fn get_size(&self) -> sdk::api::display::Size {
        sdk::api::display::Size::new(self.width as i16, self.height as i16)
    }
}


impl OriginDimensions for PixelBuffer {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for PixelBuffer {
    type Color = Gray8;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels.into_iter() {
            if point.x >= 0 && point.y >= 0 {
                self.set_pixel(
                    point.x as usize,
                    point.y as usize,
                    Intensity::new(color.luma()),
                );
            }
        }
        Ok(())
    }
}
