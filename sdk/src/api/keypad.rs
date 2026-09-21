use std::collections::VecDeque;

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::{
    bridge::host_functions::keypad_set_perkins_mode,
    event::{EventHandler, EventKind},
};

#[derive(Default)]
pub struct Keypad {
    keypad_events: VecDeque<KeypadEvent>,
    braille_chord_events: VecDeque<u8>,
    is_function_pressed: bool,
    is_menu_pressed: bool,
    help_triggered: bool,
}

impl Keypad {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pop_event(&mut self) -> KeypadPopResult {
        let keypad_event = self.keypad_events.pop_front();
        match keypad_event {
            Some(keypad_event) => KeypadPopResult::Event(keypad_event),
            None => KeypadPopResult::NoEvent,
        }
    }

    pub fn push_event_front(&mut self, event: KeypadEvent) {
        self.keypad_events.push_front(event);
    }

    pub fn pop_braille_chord(&mut self) -> KeypadBrailleChordPopResult {
        let braille_chord_event = self.braille_chord_events.pop_front();
        match braille_chord_event {
            Some(braille_chord_event) => {
                KeypadBrailleChordPopResult::BrailleChord(braille_chord_event)
            }
            None => KeypadBrailleChordPopResult::None,
        }
    }

    pub fn set_perkins_mode(&mut self, mode: bool) {
        unsafe {
            keypad_set_perkins_mode(mode);
        }
    }

    /// 처리되지 않고 큐에 남아있는 모든 키패드 및 점자 이벤트를 비웁니다.
    pub fn clear(&mut self) {
        self.keypad_events.clear();
        self.braille_chord_events.clear();
    }

    /// 기능 키(Function)와 메뉴 키(Menu)가 동시에 새로 눌려 도움말 트리거 조건이 충족되었는지 확인합니다.
    pub fn is_help_just_triggered(&mut self) -> bool {
        if self.is_function_pressed && self.is_menu_pressed {
            if !self.help_triggered {
                self.help_triggered = true;
                return true;
            }
        } else {
            self.help_triggered = false;
        }
        false
    }


}


pub enum KeypadPopResult {
    NoEvent,
    Event(KeypadEvent),
}

pub enum KeypadBrailleChordPopResult {
    None,
    BrailleChord(u8),
}

/// 키 이벤트를 나타내는 구조체입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct KeypadEventV1 {
    pub code: KeyCode,
    pub state: KeyState,
    pub side: KeypadSide,
}

impl EventKind for KeypadEventV1 {}

pub type KeypadEvent = KeypadEventV1;

/// 키패드에서 가능한 키들을 정의하는 열거형입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[repr(u8)]
pub enum KeyCode {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    Center = 4,
    Function = 5,
    Menu = 6,
    Unknown = 255,
}

impl TryFrom<i16> for KeyCode {
    type Error = ();

    /// `i16` 값을 `KeyCode` 열거형으로 변환을 시도합니다.
    /// WASM 호스트와 애플릿 간의 키 코드 통신에 사용됩니다.
    fn try_from(code: i16) -> Result<Self, Self::Error> {
        match code {
            0 => Ok(KeyCode::Up),
            1 => Ok(KeyCode::Down),
            2 => Ok(KeyCode::Left),
            3 => Ok(KeyCode::Right),
            4 => Ok(KeyCode::Center),
            5 => Ok(KeyCode::Function),
            6 => Ok(KeyCode::Menu),
            255 => Ok(KeyCode::Unknown),
            _ => Err(()), // 정의되지 않은 키 코드
        }
    }
}

impl From<KeyCode> for i16 {
    /// `KeyCode` 열거형을 `i16` 값으로 변환합니다.
    /// WASM 애플릿이 호스트에 키 정보를 전달할 때 사용됩니다.
    fn from(key: KeyCode) -> Self {
        match key {
            KeyCode::Up => 0,
            KeyCode::Down => 1,
            KeyCode::Left => 2,
            KeyCode::Right => 3,
            KeyCode::Center => 4,
            KeyCode::Function => 5,
            KeyCode::Menu => 6,
            KeyCode::Unknown => 255,
        }
    }
}

impl From<u8> for KeyCode {
    fn from(value: u8) -> Self {
        match value {
            0 => KeyCode::Up,
            1 => KeyCode::Down,
            2 => KeyCode::Left,
            3 => KeyCode::Right,
            4 => KeyCode::Center,
            5 => KeyCode::Function,
            6 => KeyCode::Menu,
            _ => KeyCode::Unknown,
        }
    }
}

/// 키의 현재 상태를 정의하는 열거형입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[repr(u8)]
pub enum KeyState {
    Pressed = 0,
    Released = 1,
    Unknown = 255,
}

impl TryFrom<i16> for KeyState {
    type Error = ();

    /// `i16` 값을 `KeyState` 열거형으로 변환을 시도합니다.
    /// WASM 호스트와 애플릿 간의 키 상태 통신에 사용됩니다.
    fn try_from(code: i16) -> Result<Self, Self::Error> {
        match code {
            0 => Ok(KeyState::Pressed),
            1 => Ok(KeyState::Released),
            255 => Ok(KeyState::Unknown),
            _ => Err(()), // 정의되지 않은 상태 코드
        }
    }
}

impl From<KeyState> for i16 {
    /// `KeyState` 열거형을 `i16` 값으로 변환합니다.
    /// WASM 애플릿이 호스트에 키 상태를 전달할 때 사용됩니다.
    fn from(state: KeyState) -> Self {
        match state {
            KeyState::Pressed => 0,
            KeyState::Released => 1,
            KeyState::Unknown => 255,
        }
    }
}

impl From<u8> for KeyState {
    fn from(value: u8) -> Self {
        match value {
            0 => KeyState::Pressed,
            1 => KeyState::Released,
            _ => KeyState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum KeypadSide {
    Left,
    Right,
}

impl From<i16> for KeypadSide {
    fn from(value: i16) -> Self {
        match value {
            0 => KeypadSide::Left,
            1 => KeypadSide::Right,
            _ => KeypadSide::Left,
        }
    }
}

impl From<KeypadSide> for i16 {
    fn from(value: KeypadSide) -> Self {
        match value {
            KeypadSide::Left => 0,
            KeypadSide::Right => 1,
        }
    }
}

impl EventHandler<KeypadEvent> for Keypad {
    fn handle_event(&mut self, event: KeypadEvent) {
        if event.state == KeyState::Pressed {
            match event.code {
                KeyCode::Function => self.is_function_pressed = true,
                KeyCode::Menu => self.is_menu_pressed = true,
                _ => {}
            }
        } else if event.state == KeyState::Released {
            match event.code {
                KeyCode::Function => self.is_function_pressed = false,
                KeyCode::Menu => self.is_menu_pressed = false,
                _ => {}
            }
        }

        if !self.is_function_pressed || !self.is_menu_pressed {
            self.help_triggered = false;
        }

        self.keypad_events.push_back(event);
    }
}


impl EventHandler<u8> for Keypad {
    fn handle_event(&mut self, event: u8) {
        self.braille_chord_events.push_back(event);
    }
}
