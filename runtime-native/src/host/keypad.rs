use crate::host::HostState;
use anyhow::Result;
use wasmtime::{Caller, Linker};

pub trait HostKeypadInterface {
    fn start(&mut self);
    fn set_perkins_mode(&mut self, mode: bool);
}

/// `wasmtime::Linker`에 키패드 관련 호스트 함수들을 추가합니다.
/// 이 함수들은 WASM 모듈이 호스트 환경의 키패드와 상호작용할 수 있도록 합니다.
pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "keypad",
        "set_perkins_mode",
        |mut caller: Caller<'_, HostState>, mode: i32| {
            caller.data_mut().keypad.set_perkins_mode(mode == 1);
        },
    )?;
    Ok(())
}
