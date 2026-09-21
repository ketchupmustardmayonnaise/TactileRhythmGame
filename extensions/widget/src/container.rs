use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity, Point, Size};
use sdk::error::Result;
use crate::core::{Widget, WidgetUpdateResult};

/// 상하좌우 여백을 지정하기 위한 구조체입니다. (Margin, Padding 등에 사용)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spacing {
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
    pub left: i16,
}

impl Spacing {
    pub fn new(top: i16, right: i16, bottom: i16, left: i16) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn all(value: i16) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: i16, horizontal: i16) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// 다른 위젯을 감싸고 여백 및 테두리를 제공할 수 있는 컨테이너 위젯입니다.
#[derive(Default, Debug)]
pub struct Container {
    bounds: BoundsRect,
    pub margin: Spacing,
    pub padding: Spacing,
    pub border_width: i16,
    pub border_intensity: Option<Intensity>,
    pub background_intensity: Option<Intensity>,
}

impl Container {
    pub fn new(bounds: BoundsRect) -> Self {
        Self {
            bounds,
            ..Default::default()
        }
    }

    pub fn with_margin(mut self, margin: Spacing) -> Self {
        self.margin = margin;
        self
    }

    pub fn with_padding(mut self, padding: Spacing) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_border(mut self, width: i16, intensity: Intensity) -> Self {
        self.border_width = width;
        self.border_intensity = Some(intensity);
        self
    }

    pub fn with_background(mut self, intensity: Intensity) -> Self {
        self.background_intensity = Some(intensity);
        self
    }

    pub fn set_margin(&mut self, margin: Spacing) {
        self.margin = margin;
    }

    pub fn set_padding(&mut self, padding: Spacing) {
        self.padding = padding;
    }

    pub fn set_border(&mut self, width: i16, intensity: Intensity) {
        self.border_width = width;
        self.border_intensity = Some(intensity);
    }

    pub fn set_background(&mut self, intensity: Intensity) {
        self.background_intensity = Some(intensity);
    }

    /// 내부 자식 위젯이 배치될 수 있는 영역을 계산하여 반환합니다.
    pub fn content_bounds(&self) -> BoundsRect {
        let x = self.bounds.x() + self.margin.left + self.border_width + self.padding.left;
        let y = self.bounds.y() + self.margin.top + self.border_width + self.padding.top;

        let occupied_x = self.margin.left
            + self.margin.right
            + (self.border_width * 2)
            + self.padding.left
            + self.padding.right;
        let occupied_y = self.margin.top
            + self.margin.bottom
            + (self.border_width * 2)
            + self.padding.top
            + self.padding.bottom;

        let width = (self.bounds.width() - occupied_x).max(0);
        let height = (self.bounds.height() - occupied_y).max(0);

        BoundsRect::new(Point::new(x, y), Size::new(width, height))
    }

    fn border_bounds(&self) -> BoundsRect {
        let x = self.bounds.x() + self.margin.left;
        let y = self.bounds.y() + self.margin.top;
        let width = (self.bounds.width() - self.margin.left - self.margin.right).max(0);
        let height = (self.bounds.height() - self.margin.top - self.margin.bottom).max(0);
        BoundsRect::new(Point::new(x, y), Size::new(width, height))
    }
}

impl Widget for Container {
    fn bounds(&self) -> BoundsRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: BoundsRect) {
        self.bounds = bounds;
    }

    fn on_update(
        &mut self,
        _context: &mut Context,
        _is_focused: bool,
    ) -> Result<WidgetUpdateResult> {
        Ok(WidgetUpdateResult::Unchanged)
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        let b_bounds = self.border_bounds();
        if b_bounds.width() <= 0 || b_bounds.height() <= 0 {
            return Ok(());
        }

        // 1. 배경 그리기 (테두리를 포함한 내부 영역)
        if let Some(bg_intensity) = self.background_intensity {
            for y in b_bounds.y()..(b_bounds.y() + b_bounds.height()) {
                for x in b_bounds.x()..(b_bounds.x() + b_bounds.width()) {
                    canvas.set_pin(Point::new(x, y), bg_intensity);
                }
            }
        }

        // 2. 테두리 그리기
        if let Some(b_intensity) = self.border_intensity
            && self.border_width > 0
        {
            let min_x = b_bounds.x();
            let max_x = b_bounds.x() + b_bounds.width();
            let min_y = b_bounds.y();
            let max_y = b_bounds.y() + b_bounds.height();

            for y in min_y..max_y {
                for x in min_x..max_x {
                    let is_top_border = y < min_y + self.border_width;
                    let is_bottom_border = y >= max_y - self.border_width;
                    let is_left_border = x < min_x + self.border_width;
                    let is_right_border = x >= max_x - self.border_width;

                    if is_top_border || is_bottom_border || is_left_border || is_right_border {
                        canvas.set_pin(Point::new(x, y), b_intensity);
                    }
                }
            }
        }

        Ok(())
    }
}
