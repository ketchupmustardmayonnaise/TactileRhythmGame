use crate::host::HostState;
use anyhow::Result;
use wasmtime::{Caller, Extern, Linker};

/// `wasmtime::Linker`에 로깅 관련 호스트 함수들을 추가합니다.
/// 이 함수들은 WASM 모듈이 호스트 환경으로 로그 메시지를 보낼 수 있도록 합니다.
pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    // WASM 모듈에서 `log.message(level, ptr, len)`를 호출하면 호스트에서 로그 메시지를 처리합니다.
    linker.func_wrap(
        "log",
        "message",
        |mut caller: Caller<'_, HostState>, level: u32, ptr: u32, len: u32| {
            // WASM 모듈의 메모리 익스포트를 가져옵니다. 로그 메시지는 이 메모리에 저장되어 있습니다.
            let mem = match caller.get_export("memory") {
                Some(Extern::Memory(mem)) => mem, // 메모리 익스포트가 성공적으로 가져와진 경우
                _ => {
                    // 메모리 익스포트를 가져오지 못한 경우 경고를 로깅하고 함수를 종료합니다.
                    tracing::error!("WASM 모듈에서 메모리 익스포트를 가져오는데 실패했습니다");
                    return;
                }
            };
            // 로그 메시지를 저장할 버퍼를 생성합니다.
            let (ptr, len) = (ptr as usize, len as usize);

            // 로그 메시지 최대 길이 제한
            if len > runtime_common::log::MAX_LOG_MESSAGE_SIZE {
                tracing::error!("로그 메시지 길이가 제한을 초과했습니다: {} bytes", len);
                return;
            }

            let data = mem.data(&caller);
            if let Some(bytes) = data.get(ptr..ptr + len) {
                let msg = String::from_utf8_lossy(bytes);
                match level {
                    1 => tracing::error!("APP: {}", msg),
                    2 => tracing::warn!("APP: {}", msg),
                    3 => tracing::info!("APP: {}", msg),
                    4 => tracing::debug!("APP: {}", msg),
                    5 => tracing::trace!("APP: {}", msg),
                    _ => tracing::info!("APP: {}", msg),
                }
            }
        },
    )?;
    Ok(())
}
