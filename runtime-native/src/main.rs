use anyhow::Result;
use clap::Parser;
use host_utils::serial_comm::{CHANNEL_CAPACITY, ConnectionStatus};
use protocols::esp32::{RequestToEsp32, ResponseFromEsp32};
use runtime_common::event::EventChannel;
use sdk::api::display::Size;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::EnvFilter;
use wasmtime::*;

use crate::cli::Args;
use crate::config::RuntimeConfig;
use crate::hal::display::braille_display::BrailleDisplay;
use crate::hal::display::console_display::ConsoleDisplay;
use crate::hal::keypad::braille_keypad::BrailleKeypad;
use crate::hal::keypad::console_keypad::ConsoleKeypad;
use crate::host::HostState;
use crate::host::display::HostDisplayInterface;
use crate::host::keypad::HostKeypadInterface;
use crate::uart_comm::UartComm;

mod audio;
mod cli;
mod config;
mod hal;
mod host;
mod net_comm;
mod runner;
mod uart_comm;
mod usb_serial;

/// 메인 함수: WASM 애플릿을 로드하고 실행하는 전체 과정을 담당합니다.
#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 시스템 초기화 (RUST_LOG 환경 변수로 로그 레벨 제어 가능)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // CLI 인자 파싱
    let args = Args::parse();
    tracing::info!("파싱된 CLI 인자 확인: {:?}", args);

    // 애플릿 경로만 출력하고 종료
    if args.print_applet_dir {
        let wasm_dir = RuntimeConfig::load()
            .map(|s| s.get_applet_dir())
            .unwrap_or_else(|_| RuntimeConfig::init_default_applet_dir());
        println!("{}", wasm_dir.display());
        return Ok(());
    }

    // 실행할 초기 애플릿 이름(기본: launcher)을 결정합니다.
    let default_app_name = cli::determine_app_name(args.app_name)?;
    let mut current_app_name = default_app_name.clone();

    let mirror_running = Arc::new(AtomicBool::new(current_app_name == "mirror"));

    let event_channel = EventChannel::new();

    let (esp32_request_sender, esp32_response_receiver, esp32_status_receiver, shared_console_pins) =
        if args.device {
            let UartComm {
                esp32_request_sender,
                esp32_response_receiver,
                status_receiver,
            } = uart_comm::UartComm::new("/dev/ttyAMA0", 115_200)?;
            (
                esp32_request_sender,
                esp32_response_receiver,
                status_receiver,
                None,
            )
        } else {
            let (esp32_request_sender, _esp32_request_receiver) = channel::<RequestToEsp32>(32);
            let (_esp32_response_sender, esp32_response_receiver) =
                channel::<ResponseFromEsp32>(32);
            let (_, status_receiver) = channel::<ConnectionStatus>(CHANNEL_CAPACITY);

            let pins = Arc::new(Mutex::new(vec![
                0;
                (args.width as usize)
                    * (args.height as usize)
            ]));

            (
                esp32_request_sender,
                esp32_response_receiver,
                status_receiver,
                Some(pins),
            )
        };

    let _ = esp32_request_sender
        .send(RequestToEsp32::BootCompleted)
        .await;

    sleep(Duration::from_secs(1)).await;

    let _ = esp32_request_sender
        .send(RequestToEsp32::BootCompleted)
        .await;

    // ESP32 연결 상태(ConnectionStatus) 변경 이벤트를 감지하여 로깅하는 태스크 추가
    tokio::spawn(async move {
        let mut receiver = esp32_status_receiver;
        while let Some(status) = receiver.recv().await {
            tracing::info!("ESP32 Connection Status: {:?}", status);
        }
    });

    // 앱 교체 신호를 주고받을 MPSC 채널 생성
    let (applet_sender, mut applet_receiver) = tokio::sync::mpsc::unbounded_channel::<String>();

    // 우아한 종료를 위한 플래그
    let should_exit = Arc::new(AtomicBool::new(false));

    // USB CDC-ACM (가젯 시리얼) 포트 활성화 시도
    if args.device {
        if let Err(e) = usb_serial::start_usb_serial(
            "/dev/ttyGS0",
            460_800,
            esp32_request_sender.clone(),
            applet_sender.clone(),
            mirror_running.clone(),
            shared_console_pins.clone(),
        )
        .await
        {
            tracing::warn!("USB Serial (CDC-ACM) disabled or failed to start: {}", e);
        } else {
            tracing::info!("USB Serial (CDC-ACM) started on /dev/ttyGS0");
        }
    }

    // TCP 소켓 (NetComm) 서버 활성화 시도 (포트: 3006)
    let net_addr = "0.0.0.0:3006";
    if let Err(e) = net_comm::start_net_comm(
        net_addr,
        esp32_request_sender.clone(),
        applet_sender.clone(),
        mirror_running.clone(),
        shared_console_pins.clone(),
    )
    .await
    {
        tracing::warn!("NetComm (TCP) disabled or failed to start: {}", e);
    } else {
        tracing::info!("NetComm (TCP) started on {}", net_addr);
    }

    // 키패드 구현체 선택 및 이벤트 큐 등록
    let mut keypad: Box<dyn HostKeypadInterface + Send> = if args.device {
        Box::new(BrailleKeypad::new(
            event_channel.sender(),
            esp32_request_sender.clone(),
            esp32_response_receiver,
        ))
    } else {
        Box::new(ConsoleKeypad::new(
            event_channel.sender(),
            should_exit.clone(),
        ))
    };
    keypad.start();

    // 디스플레이 상태를 루프 밖에서 한 번만 초기화합니다.
    let mut display: Box<dyn HostDisplayInterface + Send> = if args.device {
        Box::new(BrailleDisplay::new(
            Size::new(args.width, args.height),
            esp32_request_sender.clone(),
            args.bits_per_pixel,
        ))
    } else {
        if let Some(pins) = shared_console_pins {
            Box::new(ConsoleDisplay::new_shared(
                Size::new(args.width, args.height),
                pins,
            ))
        } else {
            Box::new(ConsoleDisplay::new(Size::new(args.width, args.height)))
        }
    };

    // 메인 루프에서 재사용할 수 있도록 키패드가 등록된 이벤트를 변수에 담습니다.
    let mut current_event_channel = event_channel;

    // --- WASM 모듈 동적 교체 및 실행 루프 ---
    // Wasmtime 엔진 생성
    let engine = Engine::default();

    // Linker 생성
    let mut linker = Linker::new(&engine);
    host::add_to_linker(&mut linker)?;

    // 설정 파일에서 앱 디렉토리 경로를 가져옵니다.
    let wasm_dir = RuntimeConfig::load()
        .map(|s| s.get_applet_dir())
        .unwrap_or_else(|_| RuntimeConfig::init_default_applet_dir());

    loop {
        // 종료 플래그 확인
        if should_exit.load(Ordering::SeqCst) {
            tracing::info!("종료 플래그가 설정되어 메인 루프를 안전하게 종료합니다.");
            break;
        }

        // 설정된 디렉토리와 현재 앱 이름을 조합하여 WASM 파일 경로를 생성합니다.
        let wasm_path = wasm_dir.join(format!("{}.wasm", current_app_name));
        info!("모듈 컴파일 중: {}...", wasm_path.display());

        let module = Module::from_file(&engine, &wasm_path)?;

        info!("초기화 중...");

        let mut store = Store::new(
            &engine,
            HostState {
                display,
                keypad,
                event_channel: current_event_channel, // 채널 소유권을 Store 내부로 전달
                applet_sender: applet_sender.clone(),
                current_app_name: current_app_name.clone(),
            },
        );

        info!("모듈 인스턴스화 중...");
        let instance = linker.instantiate(&mut store, &module)?;

        info!("익스포트 추출 중...");
        let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;
        let on_event = instance.get_typed_func::<(), i32>(&mut store, "on_event")?;
        let get_event_buffer_ptr =
            instance.get_typed_func::<i32, i32>(&mut store, "get_event_buffer_ptr")?;
        let get_event_result_buffer_ptr =
            instance.get_typed_func::<i32, i32>(&mut store, "get_event_result_buffer_ptr")?;
        let target_fps = instance.get_typed_func::<(), i32>(&mut store, "target_fps")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("메모리를 찾을 수 없습니다"))?;

        info!("익스포트 호출 중...");
        let (returned_store, next_app) = runner::run(
            store,
            memory,
            &run,
            &on_event,
            &get_event_buffer_ptr,
            &get_event_result_buffer_ptr,
            &target_fps,
            &mut applet_receiver,
            args.benchmark,
        )
        .await?;

        // 다음 모듈을 위해 사용 완료한 Store에서 EventChannel 소유권을 다시 가져옵니다.
        let host_data = returned_store.into_data();
        current_event_channel = host_data.event_channel;
        keypad = host_data.keypad;
        display = host_data.display;

        if let Some(app_name) = next_app {
            current_app_name = app_name;
            info!("애플릿 전환 요청 확인됨: {}", current_app_name);
        } else {
            current_app_name = default_app_name.clone();
            // break; // 앱에서 완전 종료 요청이 왔을 경우 루프를 종료합니다.
        }
        mirror_running.store(current_app_name == "mirror", Ordering::SeqCst);
    }

    info!("완료.");
    Ok(())
}
