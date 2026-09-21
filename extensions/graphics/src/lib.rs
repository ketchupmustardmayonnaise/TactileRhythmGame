mod circle;
mod line;
mod rectangle;
pub mod style;
mod text;
use std::sync::LazyLock;

use embedded_graphics::{
    Drawable, Pixel,
    pixelcolor::Gray8,
    prelude::{
        DrawTarget, GrayColor, OriginDimensions, Point as EgPoint, Primitive, Size as EgSize,
    },
    primitives::{Circle as EgCircle, Line as EgLine, Rectangle as EgRectangle},
};
use sdk::api::display::{DisplayInterface, Intensity, Pin, Point, Size};

use crate::style::Style;
pub use circle::Circle;
pub use line::Line;
pub use rectangle::Rectangle;
pub use text::Text;
pub use text::TextAlign;

struct DisplayWrapper<'a, T: DisplayInterface + ?Sized>(pub &'a mut T);

/// `embedded_graphics` 크레이트의 `OriginDimensions` 트레이트 구현입니다.
/// 디스플레이의 원점과 크기를 제공합니다.
impl<'a, T: DisplayInterface + ?Sized> OriginDimensions for DisplayWrapper<'a, T> {
    fn size(&self) -> EgSize {
        let size = self.0.get_size();
        EgSize::new(size.width as u32, size.height as u32)
    }
}

/// `embedded_graphics` 크레이트의 `DrawTarget` 트레이트 구현입니다.
/// 픽셀 스트림을 받아 디스플레이에 그리는 기능을 제공합니다.
impl<'a, T: DisplayInterface + ?Sized> DrawTarget for DisplayWrapper<'a, T> {
    type Color = Gray8; // 픽셀 색상은 8비트 회색조를 사용합니다.
    type Error = (); // 발생 가능한 에러 타입

    /// 픽셀들의 이터레이터를 받아 디스플레이에 그립니다.
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            // 좌표를 i16로 변환 가능한지 확인합니다.
            if let (Ok(x), Ok(y)) = (
                TryInto::<i16>::try_into(coord.x),
                TryInto::<i16>::try_into(coord.y),
            ) {
                // 디스플레이에 픽셀을 설정합니다.
                // 8단계로 양자화하여 사람이 촉각으로 구분하기 쉬운 강도로 변환합니다.
                self.0.set_pin((x, y).into(), Intensity::from(color.luma()));
            }
        }
        Ok(())
    }
}

pub trait Graphics {
    fn draw_iter(&mut self, pins: impl IntoIterator<Item = Pin>);
    fn draw_line(&mut self, start: Point, end: Point, style: Style);
    fn draw_circle(&mut self, center: Point, diameter: i16, style: Style);
    fn draw_rectangle(&mut self, top_left: Point, size: Size, style: Style);
    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        font_size: u32,
        color: Intensity,
        alignment: TextAlign,
    );
}

impl<T: DisplayInterface + ?Sized> Graphics for T {
    fn draw_iter(&mut self, pins: impl IntoIterator<Item = Pin>) {
        for pin in pins {
            self.set_pin(pin.point, pin.intensity);
        }
    }

    fn draw_line(&mut self, start: Point, end: Point, style: Style) {
        let _ = EgLine::new(
            EgPoint::new(start.x as i32, start.y as i32),
            EgPoint::new(end.x as i32, end.y as i32),
        )
        .into_styled(style.into())
        .draw(&mut DisplayWrapper(self));
    }

    fn draw_circle(&mut self, center: Point, diameter: i16, style: Style) {
        // with_center의 우측 하단 0.5픽셀 편향 문제를 방지하기 위해 top_left를 직접 계산합니다.
        let top_left = EgPoint::new(
            (center.x - diameter / 2) as i32,
            (center.y - diameter / 2) as i32,
        );

        let _ = EgCircle::new(top_left, diameter as u32)
            .into_styled(style.into())
            .draw(&mut DisplayWrapper(self));
    }

    fn draw_rectangle(&mut self, top_left: Point, size: Size, style: Style) {
        let _ = EgRectangle::new(
            EgPoint::new(top_left.x as i32, top_left.y as i32),
            EgSize::new(size.width as u32, size.height as u32),
        )
        .into_styled(style.into())
        .draw(&mut DisplayWrapper(self));
    }

    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        font_size: u32,
        color: Intensity,
        alignment: TextAlign,
    ) {
        // rusttype을 사용하여 TTF 폰트를 렌더링합니다.
        // 주의: assets 폴더의 실제 위치에 따라 상대 경로를 조정해야 할 수 있습니다.
        static FONT: LazyLock<rusttype::Font<'static>> = LazyLock::new(|| {
            let font_data = include_bytes!("NanumGothic.ttf");
            rusttype::Font::try_from_bytes(font_data).expect("Error constructing Font")
        });
        let font = &*FONT;
        let scale = rusttype::Scale::uniform(font_size as f32);

        // layout을 한 번만 호출하여 글리프 정보를 가져옵니다.
        let glyphs: Vec<_> = font
            .layout(text, scale, rusttype::point(0.0, 0.0))
            .collect();

        // 텍스트 너비를 계산하여 정렬에 사용합니다.
        let text_width = glyphs
            .last()
            .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
            .unwrap_or(0.0);

        // 정렬에 따라 시작 x 좌표를 계산합니다.
        let start_x = match alignment {
            TextAlign::Left => position.x as f32,
            TextAlign::Center => position.x as f32 - text_width / 2.0,
            TextAlign::Right => position.x as f32 - text_width,
        };
        let start_y = position.y as f32; // y 좌표는 기준선(baseline)으로 사용합니다.

        // 계산된 시작 위치를 적용하여 각 글리프를 그립니다.
        for glyph in &glyphs {
            let positioned_glyph = glyph.unpositioned().clone().positioned(rusttype::point(
                start_x + glyph.position().x,
                start_y + glyph.position().y,
            ));
            if let Some(bounding_box) = positioned_glyph.pixel_bounding_box() {
                positioned_glyph.draw(|x, y, v| {
                    let px = bounding_box.min.x + x as i32;
                    let py = bounding_box.min.y + y as i32;

                    // v는 해당 픽셀의 폰트 불투명도(0.0 ~ 1.0)입니다.
                    let intensity_value = (color.value as f32 * v) as u8;

                    if intensity_value > 0 {
                        let intensity = Intensity::from(intensity_value);
                        self.set_pin((px as i16, py as i16).into(), intensity);
                    }
                });
            }
        }
    }
}

pub trait Draw {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized));
}

impl<T: Draw> Draw for &[T] {
    fn draw(&self, graphics: &mut (impl Graphics + ?Sized)) {
        for item in *self {
            item.draw(graphics);
        }
    }
}
