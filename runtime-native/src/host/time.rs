use crate::host::HostState;
use anyhow::Result;
use std::{
    sync::LazyLock,
    time::{Instant, SystemTime},
};
use time::UtcOffset; // 시간대 오프셋을 가져오기 위한 크레이트
use wasmtime::{Caller, Linker};

// 모노토닉 시간을 위한 시작 시점
static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);

/// 현재 시간을 Unix epoch (1970-01-01 00:00:00 UTC) 이후 경과된 초 단위로 반환하는 호스트 함수입니다.
/// WASM 모듈이 호스트 시스템의 시간을 알 수 있도록 합니다.
fn get_time_seconds(mut _caller: Caller<'_, HostState>) -> u64 {
    // `SystemTime::now()`를 통해 현재 시스템 시간을 가져오고,
    // `duration_since(SystemTime::UNIX_EPOCH)`를 통해 Unix epoch 이후의 기간을 계산한 후,
    // `as_secs()`를 통해 초 단위로 변환합니다.
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 현재 시스템의 로컬 시간대 오프셋을 초 단위로 반환하는 호스트 함수입니다.
/// WASM 모듈이 호스트의 시간대 정보를 알 수 있도록 합니다.
fn get_timezone_offset_seconds(mut _caller: Caller<'_, HostState>) -> i32 {
    // `UtcOffset::current_local_offset()`을 통해 현재 로컬 시간대 오프셋을 가져오고,
    // `whole_seconds()`를 통해 이를 초 단위 정수로 변환합니다.
    if let Ok(offset) = UtcOffset::current_local_offset() {
        offset.whole_seconds()
    } else {
        0
    }
}

/// 애플리케이션 시작 시점부터 경과된 시간을 나노초 단위로 반환합니다.
/// 게임 루프 등 정밀한 시간 측정이 필요할 때 사용합니다.
fn get_monotonic_time_nanos(mut _caller: Caller<'_, HostState>) -> u64 {
    Instant::now().duration_since(*START_TIME).as_nanos() as u64
}

/// `wasmtime::Linker`에 시간 관련 호스트 함수들을 추가합니다.
/// 이 함수들은 WASM 모듈이 호스트의 시간 정보에 접근할 수 있도록 합니다.
pub fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    // WASM 모듈에서 `time.get_time_seconds()`를 호출하면 `get_time_seconds` 호스트 함수가 실행됩니다.
    linker.func_wrap("time", "get_time_seconds", get_time_seconds)?;
    // WASM 모듈에서 `time.get_timezone_offset_seconds()`를 호출하면 `get_timezone_offset_seconds` 호스트 함수가 실행됩니다.
    linker.func_wrap(
        "time",
        "get_timezone_offset_seconds",
        get_timezone_offset_seconds,
    )?;
    linker.func_wrap("time", "get_monotonic_time_nanos", get_monotonic_time_nanos)?;
    Ok(())
}
