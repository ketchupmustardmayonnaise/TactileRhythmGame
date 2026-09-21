use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::Sender;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode as CrosstermKeyCode, KeyEventKind},
    terminal,
};
use log::{info, warn};

use sdk::api::keypad::{KeyCode, KeyState, KeypadEventV1, KeypadSide};
use sdk::event::{Event, EventV1};

use crate::host::keypad::HostKeypadInterface;

/// 콘솔 입력을 사용하여 키패드 동작을 에뮬레이션하는 구조체입니다.
pub struct ConsoleKeypad {
    is_running: Arc<Mutex<bool>>,
    handle: Option<std::thread::JoinHandle<()>>,
    sender: Sender<Event>,
    should_exit: Arc<AtomicBool>,
}

impl ConsoleKeypad {
    /// 새로운 `ConsoleKeypad` 인스턴스를 생성합니다.
    /// 생성 시 터미널 raw 모드를 활성화합니다.
    pub fn new(sender: Sender<Event>, should_exit: Arc<AtomicBool>) -> Self {
        if terminal::enable_raw_mode().is_err() {
            warn!("Failed to enable terminal raw mode.");
        }
        Self {
            is_running: Default::default(),
            handle: Default::default(),
            sender,
            should_exit,
        }
    }
}

impl Drop for ConsoleKeypad {
    /// `ConsoleKeypad` 인스턴스가 소멸될 때 터미널 raw 모드를 비활성화합니다.
    fn drop(&mut self) {
        if let Ok(mut running) = self.is_running.lock() {
            *running = false;
        }
        if let Some(handle) = self.handle.take() {
            // 스레드가 종료될 때까지 대기합니다.
            let _ = handle.join();
        }
        if terminal::disable_raw_mode().is_err() {
            warn!("Failed to disable terminal raw mode.");
        }
    }
}

impl HostKeypadInterface for ConsoleKeypad {
    fn start(&mut self) {
        let is_running = self.is_running.clone();

        if let Ok(mut running) = is_running.lock() {
            if *running {
                return; // 이미 실행 중
            }
            *running = true;
        }

        let sender = self.sender.clone();
        let should_exit = self.should_exit.clone();

        let handle = std::thread::spawn(move || {
            loop {
                // 종료 조건 확인
                if let Ok(running) = is_running.lock() {
                    if !*running {
                        break;
                    }
                } else {
                    break;
                }

                // 이벤트 확인
                match event::poll(Duration::from_millis(10)) {
                    Ok(true) => {
                        if let Ok(CrosstermEvent::Key(key_event)) = event::read() {
                            let key_state = match key_event.kind {
                                KeyEventKind::Press => Some(KeyState::Pressed),
                                KeyEventKind::Release => Some(KeyState::Released),
                                KeyEventKind::Repeat => None,
                            };

                            let key_code: Option<KeyCode> = match key_event.code {
                                CrosstermKeyCode::Esc => {
                                    info!("Esc key pressed.");
                                    None
                                }
                                CrosstermKeyCode::Up => Some(KeyCode::Up),
                                CrosstermKeyCode::Down => Some(KeyCode::Down),
                                CrosstermKeyCode::Left => Some(KeyCode::Left),
                                CrosstermKeyCode::Right => Some(KeyCode::Right),
                                CrosstermKeyCode::Enter => Some(KeyCode::Center),
                                CrosstermKeyCode::Home | CrosstermKeyCode::Char('a') => {
                                    Some(KeyCode::Function)
                                }
                                CrosstermKeyCode::End | CrosstermKeyCode::Char('z') => {
                                    Some(KeyCode::Menu)
                                }
                                CrosstermKeyCode::Char('q') => {
                                    should_exit.store(true, Ordering::SeqCst);
                                    None
                                }
                                _ => {
                                    continue;
                                }
                            };

                            if let (Some(code), Some(state)) = (key_code, key_state) {
                                let ev = KeypadEventV1 {
                                    code,
                                    state,
                                    side: KeypadSide::Left,
                                };
                                if let Err(e) = sender.send(Event::V1(EventV1::Keypad(ev))) {
                                    warn!("Error sending event: {}", e);
                                }
                            } else {
                                if let Err(e) = sender.send(Event::V1(EventV1::Stop)) {
                                    warn!("Error sending stop event: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error polling for events: {}", e);
                    }
                    _ => {}
                }
            }
        });
        self.handle = Some(handle);
    }

    fn set_perkins_mode(&mut self, _mode: bool) {
        // Cannot be implemented with console keypad
    }
}
