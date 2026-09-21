use crate::host::HostState;
use anyhow::Result;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use wasmtime::{Caller, Linker};

pub trait HostDisplayInterface: DisplayInterface {
    /// 디스플레이를 업데이트 합니다.
    fn show(&mut self) -> Result<()>;
}

/// `wasmtime::Linker`에 디스플레이 관련 호스트 함수들을 추가합니다.
/// 이 함수들은 WASM 모듈이 호스트 환경의 디스플레이와 상호작용할 수 있도록 합니다.
pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    // WASM 모듈에서 `display.set_pin(x, y, state)`를 호출하면 특정 픽셀의 상태를 설정합니다.
    linker.func_wrap(
        "display",
        "set_pin",
        |mut caller: Caller<'_, HostState>, x: i32, y: i32, intensity: i32, blink: i32| {
            // 호스트 상태에 접근하여 디스플레이 객체의 `set_pin` 메서드를 호출합니다.
            caller.data_mut().display.set_pin(
                Point::new(x as i16, y as i16),
                Intensity {
                    value: intensity as u8,
                    blink: blink != 0,
                },
            );
        },
    )?;

    Ok(())
}
