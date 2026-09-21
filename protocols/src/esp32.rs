use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

extern crate alloc;

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestToEsp32 {
    Clear,
    UpdateDisplay {
        width: u16,
        height: u16,
        /// 4-bit 모드로 패킹된 프레임 데이터 (1바이트에 2픽셀)
        data: Vec<u8>,
    },
    UpdateDisplayByDiff {
        data: Vec<u16>,
    },
    SetKeypadMode(KeypadMode),
    BootCompleted,
    ShutdownStarted,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseFromEsp32 {
    Ok,
    Error,
    BrailleChord(u8),
    KeypadButtonEvents(Vec<KeypadButtonEvent>),
    RequestShutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeypadButtonType {
    Left = 0,
    Right,
    Up,
    Down,
    Center,
    Home,
    End,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeypadSide {
    Left = 0,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeypadButton {
    pub button_type: KeypadButtonType,
    pub side: KeypadSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeypadStatus {
    Released = 0,
    Pressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeypadButtonEvent {
    pub button: KeypadButton,
    pub status: KeypadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeypadMode {
    Normal = 0,
    Perkins,
}

impl From<KeypadMode> for bool {
    fn from(value: KeypadMode) -> Self {
        value == KeypadMode::Perkins
    }
}

impl From<bool> for KeypadMode {
    fn from(value: bool) -> Self {
        if value {
            KeypadMode::Perkins
        } else {
            KeypadMode::Normal
        }
    }
}
