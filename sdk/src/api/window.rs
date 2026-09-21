use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::{
    api::display::{BoundsRect, DisplayInterface, Intensity, Point, Size},
    bridge::host_functions::display_set_pin,
    event::{EventHandler, EventKind},
};

/// WASM 애플릿이 디스플레이와 상호작용하기 위한 안전한 래퍼(wrapper) 구조체입니다.
/// 이 구조체는 `unsafe extern "C"` 함수 호출을 캡슐화합니다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Window {
    pub bounds_rect: BoundsRect,
}

impl Window {
    /// 새로운 `Window` 인스턴스를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn width(&self) -> i16 {
        self.bounds_rect.width()
    }

    pub fn height(&self) -> i16 {
        self.bounds_rect.height()
    }

    pub fn center(&self) -> Point {
        Point::new(self.width() / 2, self.height() / 2)
    }
}

impl DisplayInterface for Window {
    /// 지정된 위치의 픽셀(점자 핀) 상태를 호스트에 설정합니다.
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        // 클리핑: Window 내부의 로컬 좌표(0,0 부터 시작)를 벗어나면 그리지 않음
        if point.x < 0 || point.y < 0 || point.x >= self.width() || point.y >= self.height() {
            return;
        }

        // 호스트 디스플레이(절대 좌표계)에는 Window의 오프셋을 반영하여 전달
        let absolute_x = self.bounds_rect.x() + point.x;
        let absolute_y = self.bounds_rect.y() + point.y;
        unsafe { display_set_pin(absolute_x, absolute_y, intensity.value, intensity.blink) };
    }

    fn get_size(&self) -> Size {
        self.bounds_rect.size
    }
}

#[derive(Serialize, Deserialize, PartialEq, MaxSize)]
pub enum WindowEventV1 {
    Resize(BoundsRect),
}

impl EventKind for WindowEventV1 {}

impl EventHandler<WindowEventV1> for Window {
    fn handle_event(&mut self, event: WindowEventV1) {
        match event {
            WindowEventV1::Resize(bounds_rect) => {
                self.bounds_rect = bounds_rect;
            }
        }
    }
}

pub type WindowEvent = WindowEventV1;
