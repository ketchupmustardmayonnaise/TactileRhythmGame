//! 보드 전체의 하드웨어 설정 및 튜닝 파라미터를 관리하는 모듈입니다.

use embassy_time::Duration;
use esp_hal::time::Rate;

use protocols::esp32::{KeypadButton, KeypadButtonType, KeypadSide};

// ------------------------------------------------------------------------
// [ 1. 통신 설정 ]
// ------------------------------------------------------------------------
pub const UART_BAUDRATE: u32 = 115_200;
pub const SPI_FREQUENCY: Rate = Rate::from_mhz(6);

// ------------------------------------------------------------------------
// [ 2. 점자 디스플레이 및 PWM 설정 ]
// ------------------------------------------------------------------------
pub const BRAILLE_DISPLAY_WIDTH: u16 = 48;
pub const BRAILLE_DISPLAY_HEIGHT: u16 = 32;
pub const FRAME_BUFFER_SIZE: usize = (BRAILLE_DISPLAY_WIDTH * BRAILLE_DISPLAY_HEIGHT) as usize;
pub const PWM_STEP: u8 = 16;
pub const PWM_INTERVAL: Duration = Duration::from_micros(1000);
pub const STROBE_DELAY_MICROS: u32 = 1;

pub static IS_4BIT_MODE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// ------------------------------------------------------------------------
// [ 3. 키패드 튜닝 파라미터 ]
// ------------------------------------------------------------------------
pub const DEBOUNCE_RELEASE_DELAY: Duration = Duration::from_millis(3);
pub const CHORDING_WINDOW_DELAY: Duration = Duration::from_millis(5);
pub const POLL_INTERVAL_DELAY: Duration = Duration::from_millis(2);
pub const DEBOUNCE_CONFIRM_DELAY: Duration = Duration::from_millis(3);

pub const KEYPAD_BUTTONS: [KeypadButton; 16] = [
    KeypadButton {
        button_type: KeypadButtonType::Left,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Right,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Up,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Down,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Center,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::End,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Home,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Power,
        side: KeypadSide::Left,
    },
    KeypadButton {
        button_type: KeypadButtonType::Left,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Right,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Up,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Down,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Center,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Home,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::End,
        side: KeypadSide::Right,
    },
    KeypadButton {
        button_type: KeypadButtonType::Power,
        side: KeypadSide::Right,
    },
];

// ------------------------------------------------------------------------
// [ 4. 전원 설정 ]
// ------------------------------------------------------------------------
