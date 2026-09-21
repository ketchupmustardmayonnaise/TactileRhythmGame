use crate::api::{keypad::KeypadEventV1, window::WindowEventV1};

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Event {
    V1(EventV1),
}

impl MaxSize for Event {
    const POSTCARD_MAX_SIZE: usize = 8192; // 8KB is plenty for events, including network responses.
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum EventV1 {
    Start,
    Stop,
    Update,
    Draw,
    Window(WindowEventV1),
    Keypad(KeypadEventV1),
    BrailleChord(u8),
    HttpResponse {
        request_id: u32,
        status_code: u16,
        body: Vec<u8>,
    },
}

impl MaxSize for EventV1 {
    const POSTCARD_MAX_SIZE: usize = 8000;
}

pub trait EventKind {}

impl EventKind for u8 {}

pub trait EventHandler<T: EventKind> {
    fn handle_event(&mut self, _event: T) {}
}

#[derive(Serialize, Deserialize, PartialEq, MaxSize)]
pub enum EventResult {
    V1(EventResultV1),
}

#[derive(Serialize, Deserialize, PartialEq, MaxSize)]
pub enum EventResultV1 {
    Update(UpdateResultV1),
}

/// 애플릿의 업데이트 결과와 런타임의 다음 동작을 정의하는 열거형입니다.
#[derive(Serialize, Deserialize, PartialEq, MaxSize)]
pub enum UpdateResultV1 {
    /// 다음 프레임을 그리도록 요청합니다.
    NeedsRedraw,
    /// 다음 업데이트까지 계속 실행하지만, 다시 그릴 필요는 없습니다.
    Unchanged,
    /// 애플리케이션을 종료합니다.
    ExitApp,
}

impl From<i32> for UpdateResultV1 {
    /// `i32` 값을 `UpdateResult` 열거형으로 변환합니다.
    /// 이 변환은 WASM 호스트와 애플릿 간의 통신에 사용됩니다.
    fn from(value: i32) -> Self {
        match value {
            1 => UpdateResultV1::NeedsRedraw,
            0 => UpdateResultV1::Unchanged,
            _ => UpdateResultV1::ExitApp,
        }
    }
}

impl From<UpdateResultV1> for i32 {
    /// `UpdateResult` 열거형을 `i32` 값으로 변환합니다.
    /// 이 변환은 WASM 애플릿이 호스트에 결과를 반환할 때 사용됩니다.
    fn from(result: UpdateResultV1) -> Self {
        match result {
            UpdateResultV1::NeedsRedraw => 1,
            UpdateResultV1::Unchanged => 0,
            UpdateResultV1::ExitApp => 2,
        }
    }
}

pub type UpdateResult = UpdateResultV1;

pub enum OnEventResult {
    NoResult = 0,
    HasResult = 1,
}

impl From<i32> for OnEventResult {
    fn from(value: i32) -> Self {
        match value {
            1 => OnEventResult::HasResult,
            _ => OnEventResult::NoResult,
        }
    }
}

impl From<OnEventResult> for i32 {
    fn from(result: OnEventResult) -> Self {
        match result {
            OnEventResult::NoResult => 0,
            OnEventResult::HasResult => 1,
        }
    }
}
