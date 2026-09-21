use crate::host::{display::HostDisplayInterface, keypad::HostKeypadInterface};
use anyhow::Result;
use runtime_common::event::EventChannel;
use tokio::sync::mpsc::UnboundedSender;
use wasmtime::Linker;
pub mod applet_manager; // 런처 앱 관리 호스트 함수
pub mod audio; // 오디오 출력 관련 호스트 함수
pub mod display; // 디스플레이 관련 호스트 함수
pub mod http;
pub mod keypad; // 키패드 관련 호스트 함수
pub mod log; // 로깅 관련 호스트 함수
pub mod preferences;
pub mod time; // 시간 관련 호스트 함수

/// WASM 애플릿이 상호 작용할 수 있는 호스트 환경의 상태를 나타내는 구조체입니다.
/// 이 구조체는 디스플레이 및 키패드 인터페이스의 구현체를 소유합니다.
pub struct HostState {
    pub display: Box<dyn HostDisplayInterface + Send>, // 디스플레이 인터페이스 구현체
    pub keypad: Box<dyn HostKeypadInterface + Send>,
    pub event_channel: EventChannel,
    pub applet_sender: UnboundedSender<String>, // 앱 교체 신호를 보내기 위한 발송자
    pub current_app_name: String,
}

/// 모든 호스트 함수를 `wasmtime::Linker`에 추가하여 WASM 모듈에서 호출할 수 있도록 준비합니다.
/// 각 서브 모듈의 `add_to_linker` 함수를 호출하여 해당 모듈이 제공하는 호스트 함수를 등록합니다.
pub fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    applet_manager::add_to_linker(linker)?;
    audio::add_to_linker(linker)?;
    log::add_to_linker(linker)?;
    display::add_to_linker(linker)?;
    time::add_to_linker(linker)?;
    keypad::add_to_linker(linker)?;
    preferences::add_to_linker(linker)?;
    http::add_to_linker(linker)?;
    Ok(())
}
