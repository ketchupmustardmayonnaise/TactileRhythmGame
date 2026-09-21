use embedded_graphics::{
    pixelcolor::{BinaryColor, Gray8, raw::RawU8},
    primitives::{PrimitiveStyle, StrokeAlignment as EgStrokeAlignment},
};
use sdk::api::display::Intensity;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub fill_intensity: Option<Intensity>,
    pub stroke_intensity: Option<Intensity>,
    pub stroke_width: Option<i16>,
    pub stroke_alignment: StrokeAlignment,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fill_intensity: None,
            stroke_intensity: Some(Intensity::MAX),
            stroke_width: Some(1),
            stroke_alignment: StrokeAlignment::Center,
        }
    }
}

impl Style {
    pub fn with_stroke(color: Intensity, width: i16) -> Self {
        Self {
            fill_intensity: None,
            stroke_intensity: Some(color),
            stroke_width: Some(width),
            stroke_alignment: StrokeAlignment::Center,
        }
    }
    pub fn with_fill(color: Intensity) -> Self {
        Self {
            fill_intensity: Some(color),
            stroke_intensity: None,
            stroke_width: None,
            stroke_alignment: StrokeAlignment::Center,
        }
    }
    pub fn stroke_alignment(mut self, alignment: StrokeAlignment) -> Self {
        self.stroke_alignment = alignment;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StrokeAlignment {
    Center,
    Outside,
    Inside,
}

impl From<StrokeAlignment> for EgStrokeAlignment {
    fn from(value: StrokeAlignment) -> Self {
        match value {
            StrokeAlignment::Center => EgStrokeAlignment::Center,
            StrokeAlignment::Outside => EgStrokeAlignment::Outside,
            StrokeAlignment::Inside => EgStrokeAlignment::Inside,
        }
    }
}

impl From<Style> for PrimitiveStyle<Gray8> {
    fn from(style: Style) -> Self {
        let mut primitive_style = PrimitiveStyle::with_stroke(
            Gray8::from(RawU8::from(
                style.stroke_intensity.unwrap_or(Intensity::MAX).value,
            )),
            style.stroke_width.unwrap_or(1) as u32,
        );
        if style.stroke_intensity.is_none() {
            primitive_style.stroke_color = None;
            primitive_style.stroke_width = 0;
        }
        if let Some(fill) = style.fill_intensity {
            primitive_style.fill_color = Some(Gray8::from(RawU8::from(fill.value)));
        }
        primitive_style.stroke_alignment = style.stroke_alignment.into();
        primitive_style
    }
}

impl From<Style> for PrimitiveStyle<BinaryColor> {
    fn from(style: Style) -> Self {
        let mut primitive_style =
            PrimitiveStyle::with_stroke(BinaryColor::On, style.stroke_width.unwrap_or(1) as u32);
        if style.stroke_intensity.is_none() {
            primitive_style.stroke_color = None;
            primitive_style.stroke_width = 0;
        }
        if let Some(fill) = style.fill_intensity {
            primitive_style.fill_color = Some(if fill.value > 0 {
                BinaryColor::On
            } else {
                BinaryColor::Off
            });
        }
        primitive_style.stroke_alignment = style.stroke_alignment.into();
        primitive_style
    }
}
