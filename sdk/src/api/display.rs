use std::ops::{Add, Sub};

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// 점자 핀의 진동 강도 또는 높이 상태를 명시적으로 나타내는 타입입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Intensity {
    pub value: u8,
    pub blink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlinkFrequency {
    Freq1Hz = 1,  // Maps to index 1 (hardware code 9)
    Freq2Hz = 2,  // Maps to index 2 (hardware code 10)
    Freq4Hz = 3,  // Maps to index 3 (hardware code 11)
    Freq8Hz = 4,  // Maps to index 4 (hardware code 12)
    Freq16Hz = 5, // Maps to index 5 (hardware code 13)
    Freq32Hz = 6, // Maps to index 6 (hardware code 14)
}

impl From<BlinkFrequency> for Intensity {
    fn from(freq: BlinkFrequency) -> Self {
        let val_3bit = freq as u8; // 1..=6
        let value = (val_3bit as u16 * 255 / 7) as u8;
        Intensity { value, blink: true }
    }
}

impl Intensity {
    pub const MAX: Self = Self {
        value: 255,
        blink: false,
    };
    pub const MIN: Self = Self {
        value: 0,
        blink: false,
    };
    pub const OFF: Self = Self {
        value: 0,
        blink: false,
    };

    pub fn new(intensity: u8) -> Self {
        Self {
            value: intensity,
            blink: false,
        }
    }

    pub fn new_blink(intensity: u8) -> Self {
        Self {
            value: intensity,
            blink: true,
        }
    }
}

impl From<u8> for Intensity {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

/// 디스플레이 장치와의 상호작용을 위한 공통 인터페이스를 정의하는 트레이트입니다.
/// 이 트레이트는 호스트 런타임에서 구현됩니다.
pub trait DisplayInterface {
    /// 지정된 위치에 있는 점자 핀의 상태(높이 및 진동 강도)를 설정합니다.
    fn set_pin(&mut self, point: Point, intensity: Intensity);

    /// 디스플레이의 크기를 반환합니다.
    fn get_size(&self) -> Size;

    /// 사각형 영역을 특정 강도로 채웁니다.
    fn fill_rect(&mut self, size: Size, intensity: Intensity) {
        for x in 0..size.width {
            for y in 0..size.height {
                self.set_pin(Point::new(x, y), intensity);
            }
        }
    }

    /// 전체 디스플레이 영역을 특정 강도로 채웁니다.
    fn fill(&mut self, intensity: Intensity) {
        self.fill_rect(self.get_size(), intensity);
    }

    /// 전체 디스플레이 영역을 끕니다.
    fn clear(&mut self) {
        self.fill(Intensity::OFF);
    }
}

impl<T: DisplayInterface + ?Sized> DisplayInterface for &mut T {
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        (**self).set_pin(point, intensity);
    }

    fn get_size(&self) -> Size {
        (**self).get_size()
    }
}

impl<T: DisplayInterface + ?Sized> DisplayInterface for Box<T> {
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        (**self).set_pin(point, intensity);
    }

    fn get_size(&self) -> Size {
        (**self).get_size()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
pub struct Point {
    pub x: i16,
    pub y: i16,
}

impl Point {
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }
}

impl From<(i16, i16)> for Point {
    fn from(value: (i16, i16)) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}

impl Add<Point> for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Self::Output {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub<Point> for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Self::Output {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
pub struct Size {
    pub width: i16,
    pub height: i16,
}

impl Size {
    pub fn new(width: i16, height: i16) -> Self {
        // 값이 음수이면 0으로 설정합니다.
        let width = if width < 0 { 0 } else { width };
        let height = if height < 0 { 0 } else { height };
        Self { width, height }
    }
}

impl From<(i16, i16)> for Size {
    fn from(value: (i16, i16)) -> Self {
        Self {
            width: value.0,
            height: value.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Pin {
    pub point: Point,
    pub intensity: Intensity,
}

impl Pin {
    pub fn new(point: Point, intensity: Intensity) -> Self {
        Self { point, intensity }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
pub struct BoundsRect {
    pub top_left: Point,
    pub size: Size,
}

impl BoundsRect {
    pub fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }
}

impl BoundsRect {
    pub fn x(&self) -> i16 {
        self.top_left.x
    }

    pub fn y(&self) -> i16 {
        self.top_left.y
    }

    pub fn width(&self) -> i16 {
        self.size.width
    }

    pub fn height(&self) -> i16 {
        self.size.height
    }
}

impl From<(Point, Size)> for BoundsRect {
    fn from((top_left, size): (Point, Size)) -> Self {
        Self { top_left, size }
    }
}

/// 위젯들이 내부적으로 공유하거나 오프스크린 렌더링에 사용할 수 있는 가상 캔버스입니다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Canvas {
    pub size: Size,
    pub buffer: Vec<Intensity>,
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        let width = size.width.max(0);
        let height = size.height.max(0);
        Self {
            size: Size::new(width, height),
            buffer: vec![Intensity::OFF; (width * height) as usize],
        }
    }

    pub fn get_pin(&self, point: Point) -> Intensity {
        if point.x >= 0 && point.x < self.size.width && point.y >= 0 && point.y < self.size.height {
            self.buffer[(point.y * self.size.width + point.x) as usize]
        } else {
            Intensity::OFF
        }
    }

    pub fn resize(&mut self, new_size: Size) {
        let width = new_size.width.max(0);
        let height = new_size.height.max(0);
        if self.size.width == width && self.size.height == height {
            return;
        }
        let mut new_buffer = vec![Intensity::OFF; (width * height) as usize];
        for y in 0..self.size.height.min(height) {
            for x in 0..self.size.width.min(width) {
                new_buffer[(y * width + x) as usize] =
                    self.buffer[(y * self.size.width + x) as usize];
            }
        }
        self.size = Size::new(width, height);
        self.buffer = new_buffer;
    }

    pub fn clear(&mut self) {
        self.buffer.fill(Intensity::OFF);
    }
}

impl DisplayInterface for Canvas {
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        if point.x >= 0 && point.x < self.size.width && point.y >= 0 && point.y < self.size.height {
            self.buffer[(point.y * self.size.width + point.x) as usize] = intensity;
        }
    }

    fn get_size(&self) -> Size {
        self.size
    }
}

/// 다른 디스플레이 인터페이스(Canvas 등)의 특정 영역을 가리키는 뷰포트입니다.
/// 오프스크린 렌더링 시 자식 위젯들의 좌표를 변환하고 클리핑하는 역할을 합니다.
pub struct Viewport<'a> {
    pub target: &'a mut dyn DisplayInterface,
    pub bounds: BoundsRect,
    pub offset: Point,
}

impl<'a> Viewport<'a> {
    pub fn new(target: &'a mut dyn DisplayInterface, bounds: BoundsRect, offset: Point) -> Self {
        Self {
            target,
            bounds,
            offset,
        }
    }
}

impl<'a> DisplayInterface for Viewport<'a> {
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        if point.x < self.bounds.x()
            || point.y < self.bounds.y()
            || point.x >= self.bounds.x() + self.bounds.width()
            || point.y >= self.bounds.y() + self.bounds.height()
        {
            return;
        }
        let cx = point.x - self.offset.x;
        let cy = point.y - self.offset.y;
        self.target.set_pin(Point::new(cx, cy), intensity);
    }

    fn get_size(&self) -> Size {
        self.bounds.size
    }
}
