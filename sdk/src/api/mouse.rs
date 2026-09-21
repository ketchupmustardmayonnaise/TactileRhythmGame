/// 마우스 버튼 종류를 정의합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    Unknown = 255,
}

impl From<u8> for MouseButton {
    fn from(value: u8) -> Self {
        match value {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            _ => MouseButton::Unknown,
        }
    }
}

/// 단일 마우스/터치 이벤트를 나타냅니다.
#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub button: MouseButton,
    pub pressed: bool,
}
