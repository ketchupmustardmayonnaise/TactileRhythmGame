use alloc::vec::Vec;
use embassy_time::Timer;
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

extern crate alloc;

use crate::board_config::{DEBOUNCE_CONFIRM_DELAY, KEYPAD_BUTTONS, POLL_INTERVAL_DELAY};
use protocols::esp32::{
    KeypadButton, KeypadButtonEvent, KeypadButtonType, KeypadSide, KeypadStatus,
};

pub const KEYPAD_PERKINS_BUTTONS: [(KeypadButton, u8); 6] = [
    (
        KeypadButton {
            button_type: KeypadButtonType::Right,
            side: KeypadSide::Left,
        },
        1,
    ),
    (
        KeypadButton {
            button_type: KeypadButtonType::Up,
            side: KeypadSide::Left,
        },
        2,
    ),
    (
        KeypadButton {
            button_type: KeypadButtonType::Left,
            side: KeypadSide::Left,
        },
        3,
    ),
    (
        KeypadButton {
            button_type: KeypadButtonType::Left,
            side: KeypadSide::Right,
        },
        4,
    ),
    (
        KeypadButton {
            button_type: KeypadButtonType::Up,
            side: KeypadSide::Right,
        },
        5,
    ),
    (
        KeypadButton {
            button_type: KeypadButtonType::Right,
            side: KeypadSide::Right,
        },
        6,
    ),
];

pub struct KeypadConfig<'d> {
    // 16개의 제네릭 타입 대신 AnyPin을 활용해 배열로 관리하면 코드가 훨씬 간결해집니다.
    pub pins: [AnyPin<'d>; 16],
}

pub struct Keypad<'d> {
    buttons: [Input<'d>; 16],
    pub perkins_mode: bool,
    last_state: u16,
    accumulated_chord: u8,
    waiting_all_released: bool,
}

impl<'d> Keypad<'d> {
    /// 16개의 핀을 모두 Pull-Up 입력 모드로 설정하여 키패드를 초기화합니다.
    pub fn new(config: KeypadConfig<'d>) -> Self {
        let input_config = InputConfig::default().with_pull(Pull::Up);
        let buttons = config.pins.map(|pin| Input::new(pin, input_config));
        Self {
            buttons,
            perkins_mode: false,
            last_state: 0,
            accumulated_chord: 0,
            waiting_all_released: false,
        }
    }

    pub fn set_perkins_mode(&mut self, mode: bool) {
        self.perkins_mode = mode;
    }

    pub async fn read(&mut self) -> (Option<u8>, Vec<KeypadButtonEvent>) {
        loop {
            Timer::after(POLL_INTERVAL_DELAY).await;

            let mut current_pressed = 0u16;
            for i in 0..16 {
                if self.buttons[i].is_low() {
                    current_pressed |= 1 << i;
                }
            }

            if current_pressed != self.last_state {
                // 바운싱 노이즈 재검증 대기
                Timer::after(DEBOUNCE_CONFIRM_DELAY).await;

                let mut confirm_pressed = 0u16;
                for i in 0..16 {
                    if self.buttons[i].is_low() {
                        confirm_pressed |= 1 << i;
                    }
                }

                if confirm_pressed == current_pressed {
                    let mut events = Vec::new();
                    let mut chord_completed = None;

                    let mut perkins_pressed_count = 0;
                    let mut newly_pressed_perkins = 0u8;
                    let mut newly_released_perkins = false;

                    for i in 0..16 {
                        let is_pressed = (current_pressed & (1 << i)) != 0;
                        let was_pressed = (self.last_state & (1 << i)) != 0;
                        let button = KEYPAD_BUTTONS[i];

                        let mut is_perkins = false;
                        let mut perkins_value = 0;

                        if self.perkins_mode {
                            if let Some(&(_, val)) =
                                KEYPAD_PERKINS_BUTTONS.iter().find(|(pb, _)| pb == &button)
                            {
                                is_perkins = true;
                                perkins_value = val;
                                if is_pressed {
                                    perkins_pressed_count += 1;
                                }
                            }
                        }

                        if is_pressed != was_pressed {
                            if is_perkins {
                                if is_pressed {
                                    newly_pressed_perkins |= 1 << (perkins_value - 1);
                                } else {
                                    newly_released_perkins = true;
                                }
                            } else {
                                events.push(KeypadButtonEvent {
                                    button,
                                    status: if is_pressed {
                                        KeypadStatus::Pressed
                                    } else {
                                        KeypadStatus::Released
                                    },
                                });
                            }
                        }
                    }

                    if self.perkins_mode {
                        if !self.waiting_all_released {
                            if newly_pressed_perkins != 0 {
                                self.accumulated_chord |= newly_pressed_perkins;
                            }
                            if newly_released_perkins && self.accumulated_chord != 0 {
                                chord_completed = Some(self.accumulated_chord);
                                self.waiting_all_released = true;
                            }
                        }

                        if perkins_pressed_count == 0 {
                            self.waiting_all_released = false;
                            self.accumulated_chord = 0;
                        }
                    }

                    self.last_state = current_pressed;

                    if chord_completed.is_some() || !events.is_empty() {
                        return (chord_completed, events);
                    }
                }
            }
        }
    }
}
