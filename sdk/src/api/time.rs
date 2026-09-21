use std::time::Duration;

use crate::bridge::host_functions::{
    time_get_monotonic_time_nanos, time_get_time_seconds, time_get_timezone_offset_seconds,
};

/// WASM 애플릿이 호스트의 시간 정보와 상호작용하기 위한 안전한 래퍼(wrapper) 구조체입니다.
/// 이 구조체는 `unsafe extern "C"` 함수 호출을 캡슐화합니다.
#[derive(Default)]
pub struct Time {}

impl Time {
    /// 새로운 `Time` 인스턴스를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// 호스트로부터 현재 시간을 초 단위로 가져옵니다.
    pub fn get_time_seconds(&self) -> u64 {
        unsafe { time_get_time_seconds() } // unsafe 블록 내에서 외부 함수를 호출합니다.
    }

    /// 호스트로부터 현재 시간대 오프셋을 초 단위로 가져옵니다.
    pub fn get_timezone_offset_seconds(&self) -> i32 {
        unsafe { time_get_timezone_offset_seconds() } // unsafe 블록 내에서 외부 함수를 호출합니다.
    }

    /// 호스트로부터 정밀한 경과 시간(Monotonic time)을 가져옵니다.
    pub fn get_monotonic_time(&self) -> Duration {
        let nanos = unsafe { time_get_monotonic_time_nanos() };
        Duration::from_nanos(nanos)
    }
}
