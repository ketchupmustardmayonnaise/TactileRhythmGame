use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Point};
use sdk::error::Result;
use widget::{Widget, WidgetUpdateResult};

use crate::cell::{CELL_GAP, CELL_HEIGHT, CELL_WIDTH};

pub const CELL_STEP: i16 = CELL_WIDTH + CELL_GAP;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Alignment {
    Start,
    #[default]
    Center,
    End,
}

pub struct Label {
    text: String,
    content_width: i16,
    alignment: Alignment,
    scroll_offset: i16,
    bounds: BoundsRect,
}

impl Label {
    pub fn new(text: &str) -> Self {
        Self {
            content_width: crate::measure_text(text),
            text: text.to_string(),
            alignment: Alignment::default(),
            scroll_offset: 0,
            bounds: BoundsRect::default(),
        }
    }

    pub fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// 이전 호환성을 위한 빌더 메서드 (자동 스크롤 제거됨)
    pub fn scroll_speed(self, _frames: u32) -> Self {
        self
    }

    pub fn with_bounds(mut self, bounds: BoundsRect) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn set_text(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_string();
            self.content_width = crate::measure_text(text);
            self.scroll_offset = 0;
        }
    }

    pub fn content_width(&self) -> i16 {
        self.content_width
    }

    pub fn scroll_offset(&self) -> i16 {
        self.scroll_offset
    }

    /// 해당 `display_width`에서 스크롤할 수 있는 최대 오프셋을 반환합니다.
    pub fn max_scroll_offset(&self, display_width: i16) -> i16 {
        (self.content_width - display_width).max(0)
    }

    /// 수동으로 스크롤 오프셋을 설정합니다 (0 ~ max_scroll_offset 사이로 클램핑).
    pub fn set_scroll_offset(&mut self, offset: i16, display_width: i16) {
        let max_scroll = self.max_scroll_offset(display_width);
        self.scroll_offset = offset.clamp(0, max_scroll);
    }

    /// 지정한 델타값만큼 수동으로 스크롤합니다 (양수: 오른쪽 스크롤/텍스트 좌이동, 음수: 왼쪽 스크롤/텍스트 우이동).
    pub fn scroll_by(&mut self, delta: i16, display_width: i16) {
        self.set_scroll_offset(self.scroll_offset + delta, display_width);
    }

    /// 수동으로 왼쪽으로 스크롤합니다 (이전/왼쪽 텍스트 표시).
    /// `display_width`가 0 이하이면 설정된 `bounds.width()`를 사용합니다.
    pub fn scroll_left(&mut self, display_width: i16) {
        let width = if display_width > 0 {
            display_width
        } else {
            self.bounds.width()
        };
        self.scroll_by(-CELL_STEP, width);
    }

    /// 수동으로 오른쪽으로 스크롤합니다 (다음/오른쪽 텍스트 표시).
    /// `display_width`가 0 이하이면 설정된 `bounds.width()`를 사용합니다.
    pub fn scroll_right(&mut self, display_width: i16) {
        let width = if display_width > 0 {
            display_width
        } else {
            self.bounds.width()
        };
        self.scroll_by(CELL_STEP, width);
    }

    pub fn is_scrolling(&self, display_width: i16) -> bool {
        self.content_width > display_width
    }

    pub fn render(&self, canvas: &mut dyn DisplayInterface, y: i16, display_width: i16) {
        if self.content_width > display_width || self.scroll_offset > 0 {
            let origin = Point::new(-self.scroll_offset, y);
            crate::render_text(canvas, &self.text, origin);
        } else {
            let x = match self.alignment {
                Alignment::Start => 0,
                Alignment::Center => (display_width - self.content_width) / 2,
                Alignment::End => display_width - self.content_width,
            };
            crate::render_text(canvas, &self.text, Point::new(x, y));
        }
    }

    pub fn render_centered(&self, canvas: &mut dyn DisplayInterface) {
        let size = canvas.get_size();
        let y = (size.height - CELL_HEIGHT) / 2;
        self.render(canvas, y, size.width);
    }
}

impl Widget for Label {
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
        let display_width = self.bounds.width();
        let y = self.bounds.y() + (self.bounds.height() - CELL_HEIGHT) / 2;

        if self.content_width > display_width || self.scroll_offset > 0 {
            let origin = Point::new(self.bounds.x() - self.scroll_offset, y);
            crate::render_text(canvas, &self.text, origin);
        } else {
            let x = match self.alignment {
                Alignment::Start => 0,
                Alignment::Center => (display_width - self.content_width) / 2,
                Alignment::End => display_width - self.content_width,
            };
            crate::render_text(canvas, &self.text, Point::new(self.bounds.x() + x, y));
        }
        Ok(())
    }
}

impl Default for Label {
    fn default() -> Self {
        Self::new("")
    }
}
