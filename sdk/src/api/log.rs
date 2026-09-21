use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

use crate::bridge::host_functions::log_message;

/// `log` 크레이트의 로거 트레이트를 구현하는 정적 로거 인스턴스입니다.
static LOGGER: Logger = Logger;

/// `log` 크레이트의 `Log` 트레이트를 구현하여 로그 메시지를 처리하는 구조체입니다.
struct Logger;

impl Log for Logger {
    /// 특정 로그 메시지가 현재 로깅 설정에 따라 활성화되어야 하는지 여부를 결정합니다.
    fn enabled(&self, metadata: &Metadata) -> bool {
        // `log::set_max_level`로 설정된 전역 최대 레벨에 따라 필터링합니다.
        metadata.level() <= log::max_level()
    }

    /// 실제 로그 메시지를 처리하고 호스트로 전송합니다.
    fn log(&self, record: &Record) {
        // `log` 매크로는 이미 `enabled`를 확인하므로 다시 확인할 필요가 없습니다.
        // `log` 레벨을 호스트가 이해할 수 있는 `u32` 값으로 매핑합니다.
        let level = match record.level() {
            Level::Error => 1,
            Level::Warn => 2,
            Level::Info => 3,
            Level::Debug => 4,
            Level::Trace => 5,
        };

        // `to_string`은 `alloc` 크레이트와 전역 할당자를 필요로 합니다.
        // 이는 `no_std` + `alloc` WASM 프로젝트의 일반적인 설정입니다.
        let msg = record.args().to_string();

        // `unsafe` 블록 내에서 외부 함수 `log_message`를 호출하여 메시지를 호스트로 보냅니다.
        // 메시지의 포인터와 길이를 전달합니다.
        unsafe {
            log_message(level, msg.as_ptr(), msg.len());
        }
    }

    /// 로거의 버퍼를 비웁니다. 현재 구현에서는 버퍼링이 없으므로 아무 작업도 수행하지 않습니다.
    fn flush(&self) {}
}

/// 로거를 초기화합니다. 이 함수는 프로그램 시작 시 한 번 호출되어야 합니다.
///
/// 초기화 후에는 `log` 크레이트의 매크로(예: `log::info!`, `log::warn!`)를 사용하여 메시지를 로깅할 수 있습니다.
pub fn init() -> Result<(), SetLoggerError> {
    // 로거 인스턴스를 `log` 크레이트의 전역 로거로 설정합니다.
    log::set_logger(&LOGGER)?;
    // 최대 로그 레벨을 설정합니다. 이보다 낮은 레벨의 로그는 무시됩니다.
    // 이 설정은 구성 가능하게 만들 수 있습니다. 현재는 Trace 레벨까지 모든 것을 로깅합니다.
    log::set_max_level(LevelFilter::Trace);
    Ok(())
}
