use crossbeam_channel::Sender;
use log::{info, warn};
use protocols::esp32::{KeypadButtonType, KeypadStatus, RequestToEsp32, ResponseFromEsp32};
use sdk::api::keypad::{KeyCode, KeyState, KeypadEventV1, KeypadSide};
use sdk::event::{Event, EventV1};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use crate::host::keypad::HostKeypadInterface;

const SHUTDOWN_DELAY: Duration = Duration::from_millis(500);

pub struct BrailleKeypad {
    event_sender: Sender<Event>,
    esp32_request_sender: tokio::sync::mpsc::Sender<RequestToEsp32>,
    esp32_response_receiver: Option<Receiver<ResponseFromEsp32>>,
    handle: Option<JoinHandle<()>>,
}

impl BrailleKeypad {
    pub fn new(
        sender: Sender<Event>,
        esp32_request_sender: tokio::sync::mpsc::Sender<RequestToEsp32>,
        esp32_response_receiver: Receiver<ResponseFromEsp32>,
    ) -> Self {
        Self {
            esp32_request_sender,
            event_sender: sender,
            esp32_response_receiver: Some(esp32_response_receiver),
            handle: None,
        }
    }
}

impl Drop for BrailleKeypad {
    fn drop(&mut self) {
        // BrailleKeypad가 소멸할 때 백그라운드 태스크도 함께 정리합니다.
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl HostKeypadInterface for BrailleKeypad {
    fn start(&mut self) {
        if self.handle.is_some() {
            return; // 이미 실행 중
        }

        let Some(mut receiver) = self.esp32_response_receiver.take() else {
            return; // 리시버가 이미 소비됨
        };

        let event_sender = self.event_sender.clone();

        let handle = tokio::spawn(async move {
            while let Some(resp) = receiver.recv().await {
                match resp {
                    ResponseFromEsp32::KeypadButtonEvents(events) => {
                        for ev in events {
                            let key_state = match ev.status {
                                KeypadStatus::Pressed => KeyState::Pressed,
                                KeypadStatus::Released => KeyState::Released,
                            };

                            let key_code = match ev.button.button_type {
                                KeypadButtonType::Up => Some(KeyCode::Up),
                                KeypadButtonType::Down => Some(KeyCode::Down),
                                KeypadButtonType::Left => Some(KeyCode::Left),
                                KeypadButtonType::Right => Some(KeyCode::Right),
                                KeypadButtonType::Center => Some(KeyCode::Center),
                                KeypadButtonType::Home => Some(KeyCode::Function),
                                KeypadButtonType::End => Some(KeyCode::Menu),
                                KeypadButtonType::Power => None,
                            };

                            if let Some(code) = key_code {
                                let keypad_event = KeypadEventV1 {
                                    code,
                                    state: key_state,
                                    side: match ev.button.side {
                                        protocols::esp32::KeypadSide::Left => KeypadSide::Left,
                                        protocols::esp32::KeypadSide::Right => KeypadSide::Right,
                                    },
                                };
                                if let Err(e) =
                                    event_sender.send(Event::V1(EventV1::Keypad(keypad_event)))
                                {
                                    warn!("Error sending braille keypad event: {}", e);
                                }
                            } else {
                                if let KeyState::Released = key_state {
                                    info!("Send stop event");
                                    if let Err(e) = event_sender.send(Event::V1(EventV1::Stop)) {
                                        warn!("Error sending stop event: {}", e);
                                    }
                                    // 길게 눌러졌을 때 처리
                                }
                            }
                        }
                    }
                    ResponseFromEsp32::BrailleChord(chord) => {
                        // 브라유 점자 조합 이벤트 처리 (필요시 SDK 이벤트로 확장 가능)
                        let braille_char =
                            core::char::from_u32(0x2800 + chord as u32).unwrap_or(' ');
                        info!("Braille chord: {:08b} ('{}')", chord, braille_char);
                        if let Err(e) = event_sender.send(Event::V1(EventV1::BrailleChord(chord))) {
                            warn!("Error sending braille chord event: {}", e);
                        }
                    }
                    ResponseFromEsp32::RequestShutdown => {
                        info!(
                            "ESP32로부터 시스템 종료 요청을 수신했습니다. 애플릿을 멈추고 종료를 시작합니다."
                        );
                        if let Err(e) = event_sender.send(Event::V1(EventV1::Stop)) {
                            warn!("Error sending stop event: {}", e);
                        }
                        tokio::spawn(async move {
                            // 애플릿이 Stop 이벤트를 처리하고 리소스를 정리할 수 있도록 잠시 대기합니다.
                            tokio::time::sleep(SHUTDOWN_DELAY).await;

                            if let Err(e) = std::process::Command::new("sudo")
                                .args(["shutdown", "now"])
                                .spawn()
                            {
                                warn!("Failed to execute shutdown command: {}", e);
                            }
                        });
                    }
                    _ => {} // Ok, Error 등은 키패드 입력이 아니므로 무시
                }
            }
        });

        self.handle = Some(handle);
    }

    fn set_perkins_mode(&mut self, mode: bool) {
        if let Err(e) = self
            .esp32_request_sender
            .try_send(RequestToEsp32::SetKeypadMode(mode.into()))
        {
            warn!("Failed to send set_perkins_mode request to ESP32: {}", e);
        }
    }
}
