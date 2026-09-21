#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer, with_timeout};
use embedded_io_async::{Read, Write};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::AnyPin;
use esp_hal::timer::timg::TimerGroup;
use firmware::board_config::*;
use firmware::braille_display::{BrailleDisplay, BrailleDisplayConfig};
use firmware::keypad::{Keypad, KeypadConfig};
use firmware::power::{Power, PowerConfig};
use firmware::uart_comm::UartCommConfig;
use log::info;
use protocols::esp32::{RequestToEsp32, ResponseFromEsp32};

extern crate alloc;

// UART 송신을 위한 전역 채널 (버퍼 크기 16)
static TX_CHANNEL: Channel<CriticalSectionRawMutex, ResponseFromEsp32, 16> = Channel::new();

// 점자 디스플레이 더블 버퍼링을 위한 전역 배열 및 플래그 초기화
const INIT_ATOMIC: AtomicU8 = AtomicU8::new(0);
static FRAME_BUFFER_A: [AtomicU8; FRAME_BUFFER_SIZE] = [INIT_ATOMIC; FRAME_BUFFER_SIZE];
static FRAME_BUFFER_B: [AtomicU8; FRAME_BUFFER_SIZE] = [INIT_ATOMIC; FRAME_BUFFER_SIZE];
// false = BUFFER_A 렌더링, true = BUFFER_B 렌더링
static ACTIVE_FRAME_BUFFER: AtomicBool = AtomicBool::new(false);

// 부트 애니메이션 실행 상태 플래그
static BOOT_ANIMATION_RUNNING: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_ANIMATION_RUNNING: AtomicBool = AtomicBool::new(false);

enum InternalEvent {
    PowerKeyPressed,
    PowerKeyReleased,
    StopBootAnimation,
    StartShutdown,
}

static INTERNAL_EVENT_CHANNEL: Channel<CriticalSectionRawMutex, InternalEvent, 4> = Channel::new();

// 키패드 모드 변경을 rx_loop에서 keypad_loop로 전달하기 위한 채널
static KEYPAD_MODE_CHANNEL: Channel<CriticalSectionRawMutex, bool, 4> = Channel::new();

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/En/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

/// 임의의 개수의 비동기 태스크들을 유연하게 실행하기 위한 매크로입니다.
/// 내부적으로 `embassy_futures::join::join`을 재귀적으로 호출하여
/// 5개 이상의 태스크도 문제없이 병렬로 실행할 수 있게 해줍니다.
macro_rules! join_tasks {
    ($a:expr, $b:expr) => {
        embassy_futures::join::join($a, $b)
    };
    ($a:expr, $($rest:expr),+) => {
        embassy_futures::join::join($a, join_tasks!($($rest),+))
    };
}

#[derive(PartialEq)]
enum Status {
    StandBy,
    LinuxBooting,
    LinuxActive,
    // LinuxShuttingDown,
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.2.0

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    // Initialize UART
    let uart = match firmware::uart_comm::UartComm::new(UartCommConfig {
        uart: peripherals.UART2,
        tx: peripherals.GPIO18,
        rx: peripherals.GPIO17,
    }) {
        Ok(u) => u.into_async(),
        Err(e) => {
            log::error!("UART initialization failed: {:?}", e);
            core::future::pending::<()>().await;
            unreachable!()
        }
    };
    // let uart = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
    info!("UART initialized!");

    let power = Power::new(PowerConfig {
        cm_en: peripherals.GPIO15,
        pw12_en: peripherals.GPIO16,
    });
    info!("Power initialized!");

    // 키패드 초기화 (실제 사용하는 GPIO 핀으로 변경하세요)
    let keypad = Keypad::new(KeypadConfig {
        pins: [
            // Keypad_L
            AnyPin::from(peripherals.GPIO1),
            AnyPin::from(peripherals.GPIO2),
            AnyPin::from(peripherals.GPIO4),
            AnyPin::from(peripherals.GPIO5),
            AnyPin::from(peripherals.GPIO6),
            AnyPin::from(peripherals.GPIO7),
            AnyPin::from(peripherals.GPIO8),
            AnyPin::from(peripherals.GPIO9),
            // Keypad_R
            AnyPin::from(peripherals.GPIO13),
            AnyPin::from(peripherals.GPIO21),
            AnyPin::from(peripherals.GPIO33),
            AnyPin::from(peripherals.GPIO34),
            AnyPin::from(peripherals.GPIO35),
            AnyPin::from(peripherals.GPIO36),
            AnyPin::from(peripherals.GPIO37),
            AnyPin::from(peripherals.GPIO38),
        ],
    });
    info!("Keypad initialized!");

    // 점자 디스플레이 초기화
    let braille_display = match BrailleDisplay::new(BrailleDisplayConfig {
        spi: peripherals.SPI2,
        clock: peripherals.GPIO12,
        data: peripherals.GPIO11,
        strobe: peripherals.GPIO10,
        width: BRAILLE_DISPLAY_WIDTH,
        height: BRAILLE_DISPLAY_HEIGHT,
        frame_buffer_a: &FRAME_BUFFER_A,
        frame_buffer_b: &FRAME_BUFFER_B,
        active_frame_buffer: &ACTIVE_FRAME_BUFFER,
    }) {
        Ok(mut display) => {
            display.clear();
            display
        }
        Err(e) => {
            log::error!("BrailleDisplay initialization failed: {:?}", e);
            core::future::pending::<()>().await; // 오류 발생 시 더 이상 진행하지 않고 무한 대기
            unreachable!()
        }
    };
    info!("BrailleDisplay initialized!");

    let _ = spawner;

    // UART를 수신(Rx)과 송신(Tx)으로 분리합니다.
    let (uart_rx, uart_tx) = uart.inner.split();
    // let (mut uart_rx, mut uart_tx) = uart.split();

    // 분리된 비동기 루프들을 병렬로 동시에 실행 (메인 루프 대체)
    // 매크로를 사용하여 태스크가 늘어나더라도 쉽게 추가 가능합니다.
    join_tasks!(
        rx_loop(uart_rx),
        tx_loop(uart_tx),
        keypad_loop(keypad),
        display_loop(braille_display),
        animation_loop(),
        system_loop(power)
    )
    .await;

    // join3 내부의 퓨처들은 무한 루프이므로 이 줄은 절대 실행되지 않습니다.
    unreachable!("The main async tasks should never terminate");
}

// [루프 1] 호스트의 요청을 처리하는 수신용 비동기 루프
async fn rx_loop(mut uart_rx: impl Read) {
    let mut uart_buf = [0_u8; 8192];
    let mut rx_len = 0;
    loop {
        // embedded_io_async::Read의 read() 메서드를 사용해 범용성을 확보합니다.
        match uart_rx.read(&mut uart_buf[rx_len..]).await {
            Ok(bytes_read) => {
                if bytes_read > 0 {
                    rx_len += bytes_read;

                    // while let으로 변경: 버퍼 안에 들어있는 모든 0x00(COBS 프레임)을 한 번에 소진하도록 합니다.
                    while let Some(end_idx) = uart_buf[..rx_len].iter().position(|&b| b == 0x00) {
                        if end_idx > 0 {
                            match postcard::from_bytes_cobs::<RequestToEsp32>(
                                &mut uart_buf[..=end_idx],
                            ) {
                                Ok(req) => {
                                    let resp = match req {
                                        RequestToEsp32::Clear => {
                                            if BOOT_ANIMATION_RUNNING.load(Ordering::Relaxed)
                                                || SHUTDOWN_ANIMATION_RUNNING
                                                    .load(Ordering::Relaxed)
                                            {
                                                ResponseFromEsp32::Ok
                                            } else {
                                                for i in 0..FRAME_BUFFER_SIZE {
                                                    FRAME_BUFFER_A[i].store(0, Ordering::Relaxed);
                                                    FRAME_BUFFER_B[i].store(0, Ordering::Relaxed);
                                                }
                                                ResponseFromEsp32::Ok
                                            }
                                        }
                                        RequestToEsp32::UpdateDisplayByDiff { data } => {
                                            if BOOT_ANIMATION_RUNNING.load(Ordering::Relaxed)
                                                || SHUTDOWN_ANIMATION_RUNNING
                                                    .load(Ordering::Relaxed)
                                            {
                                                ResponseFromEsp32::Ok
                                            } else {
                                                // 1. 현재 화면에 출력 중인 활성 버퍼를 확인
                                                let currently_using_b =
                                                    ACTIVE_FRAME_BUFFER.load(Ordering::Relaxed);
                                                let (active_buffer, back_buffer) =
                                                    if currently_using_b {
                                                        (&FRAME_BUFFER_B, &FRAME_BUFFER_A)
                                                    } else {
                                                        (&FRAME_BUFFER_A, &FRAME_BUFFER_B)
                                                    };

                                                // 2. 화면이 끊기지 않도록 이전 프레임(활성 버퍼) 상태를 백버퍼로 복사
                                                for i in 0..FRAME_BUFFER_SIZE {
                                                    let val =
                                                        active_buffer[i].load(Ordering::Relaxed);
                                                    back_buffer[i].store(val, Ordering::Relaxed);
                                                }

                                                // 3. 차이(Diff) 데이터만 백버퍼에 덮어쓰기
                                                let is_4bit = IS_4BIT_MODE.load(Ordering::Relaxed);
                                                for &packed in data.iter() {
                                                    let x = ((packed >> 10) & 0x3F) as usize;
                                                    let y = ((packed >> 4) & 0x3F) as usize;
                                                    let int4 = (packed & 0x0F) as u8;
                                                    let intensity = if is_4bit {
                                                        int4
                                                    } else {
                                                        (int4 << 4) | int4 // 4비트를 8비트(0~255) 범위로 선형 스케일링
                                                    };

                                                    let idx =
                                                        y * (BRAILLE_DISPLAY_WIDTH as usize) + x;
                                                    if idx < FRAME_BUFFER_SIZE {
                                                        back_buffer[idx]
                                                            .store(intensity, Ordering::Relaxed);
                                                    }
                                                }

                                                // 4. 화면 출력 대상을 교체 (Swap)
                                                ACTIVE_FRAME_BUFFER
                                                    .store(!currently_using_b, Ordering::Release);

                                                ResponseFromEsp32::Ok
                                            }
                                        }
                                        RequestToEsp32::UpdateDisplay {
                                            data,
                                            width: _width,
                                            height: _height,
                                        } => {
                                            if BOOT_ANIMATION_RUNNING.load(Ordering::Relaxed)
                                                || SHUTDOWN_ANIMATION_RUNNING.load(Ordering::Relaxed)
                                            {
                                                ResponseFromEsp32::Ok
                                            } else {
                                                // 1. 현재 그려지고 있지 않은 백버퍼(Back Buffer) 선택
                                                let currently_using_b =
                                                    ACTIVE_FRAME_BUFFER.load(Ordering::Relaxed);
                                                let back_buffer = if currently_using_b {
                                                    &FRAME_BUFFER_A
                                                } else {
                                                    &FRAME_BUFFER_B
                                                };

                                                // 4비트 모드로 고정 설정
                                                IS_4BIT_MODE.store(true, Ordering::Relaxed);

                                                // 2. 수신된 새로운 4비트 프레임 데이터를 백버퍼에 복사
                                                let mut pixel_idx = 0;
                                                'outer: for &byte in data.iter() {
                                                    for p in 0..2 {
                                                        if pixel_idx >= FRAME_BUFFER_SIZE {
                                                            break 'outer;
                                                        }
                                                        let shift = 4 - (p * 4);
                                                        let val = (byte >> shift) & 0x0F;
                                                        back_buffer[pixel_idx]
                                                            .store(val, Ordering::Relaxed);
                                                        pixel_idx += 1;
                                                    }
                                                }

                                                // 3. 복사가 완료되면 플래그를 스위칭하여 화면 출력 대상을 교체 (Swap)
                                                ACTIVE_FRAME_BUFFER
                                                    .store(!currently_using_b, Ordering::Release);

                                                ResponseFromEsp32::Ok
                                            }
                                        }
                                        RequestToEsp32::SetKeypadMode(keypad_mode) => {
                                            info!("Keypad mode changed: {:?}", keypad_mode);
                                            // bool 타입으로 변환(into)하여 채널을 통해 모드를 전송합니다.
                                            let _ =
                                                KEYPAD_MODE_CHANNEL.try_send(keypad_mode.into());
                                            ResponseFromEsp32::Ok
                                        }
                                        RequestToEsp32::BootCompleted => {
                                            info!("Boot completed.");
                                            let _ = INTERNAL_EVENT_CHANNEL
                                                .try_send(InternalEvent::StopBootAnimation);
                                            ResponseFromEsp32::Ok
                                        }
                                        RequestToEsp32::ShutdownStarted => {
                                            info!("Shutdown started.");
                                            SHUTDOWN_ANIMATION_RUNNING
                                                .store(true, Ordering::Relaxed);
                                            let _ = INTERNAL_EVENT_CHANNEL
                                                .try_send(InternalEvent::StartShutdown);
                                            ResponseFromEsp32::Ok
                                        }
                                    };

                                    // 처리된 응답을 채널을 통해 송신(Tx) 루프로 보냄
                                    // 송신 큐가 가득 찼더라도 수신 루프가 멈추지(blocking) 않도록 try_send 사용
                                    let _ = TX_CHANNEL.try_send(resp);
                                }
                                Err(e) => log::warn!(
                                    "Postcard Deserialize Error (len={}): {:?}",
                                    end_idx,
                                    e
                                ),
                            }
                        }

                        let remaining = rx_len - (end_idx + 1);
                        uart_buf.copy_within(end_idx + 1..rx_len, 0);
                        rx_len = remaining;
                    }

                    if rx_len == uart_buf.len() {
                        rx_len = 0;
                    }
                }
            }
            Err(e) => {
                log::error!("UART Read Error: {:?}", e);
                // 패킷 유실로 인해 꼬여버린 프레임이 디코딩 에러를 유발하지 않도록 수신 버퍼를 초기화합니다.
                rx_len = 0;
            }
        }
    }
}

// [루프 2] 채널에 쌓인 응답/이벤트를 호스트로 전송하는 비동기 루프
async fn tx_loop(mut uart_tx: impl Write) {
    loop {
        let resp = TX_CHANNEL.receive().await;
        let mut tx_buf = [0_u8; 256];
        if let Ok(tx_slice) = postcard::to_slice_cobs(&resp, &mut tx_buf) {
            let _ = uart_tx.write_all(tx_slice).await;
        }
    }
}

// [루프 3] 키패드를 모니터링하다가 입력이 감지되면 채널로 전송하는 비동기 루프
async fn keypad_loop(mut keypad: Keypad<'static>) {
    loop {
        match select(keypad.read(), KEYPAD_MODE_CHANNEL.receive()).await {
            Either::First((braille_chord, events)) => {
                if let Some(braille_chord) = braille_chord {
                    let braille_char =
                        core::char::from_u32(0x2800 + braille_chord as u32).unwrap_or(' ');
                    info!("Braille chord: {:08b} ('{}')", braille_chord, braille_char);
                    // 이벤트 발생 즉시 송신(Tx) 큐로 전송
                    TX_CHANNEL
                        .send(ResponseFromEsp32::BrailleChord(braille_chord))
                        .await;
                }
                if !events.is_empty() {
                    info!("Keypad events: {:?}", events);
                    for event in &events {
                        if event.button.button_type == protocols::esp32::KeypadButtonType::Power {
                            if event.status == protocols::esp32::KeypadStatus::Pressed {
                                let _ =
                                    INTERNAL_EVENT_CHANNEL.try_send(InternalEvent::PowerKeyPressed);
                            } else {
                                let _ = INTERNAL_EVENT_CHANNEL
                                    .try_send(InternalEvent::PowerKeyReleased);
                            }
                        }
                    }
                    // 이벤트 발생 즉시 송신(Tx) 큐로 전송
                    TX_CHANNEL
                        .send(ResponseFromEsp32::KeypadButtonEvents(events))
                        .await;
                }
            }
            Either::Second(mode) => {
                keypad.set_perkins_mode(mode);
                info!("Keypad mode updated (Perkins: {})", mode);
            }
        }
    }
}

// [루프 4] 백그라운드에서 점자 디스플레이의 PWM(밝기) 제어를 수행하는 비동기 루프
async fn display_loop(mut braille_display: BrailleDisplay<'static>) {
    braille_display.draw().await;
}

// [루프 5] 초기화 및 종료 중 심미적 효과를 제공하는 애니메이션 비동기 루프
async fn animation_loop() {
    loop {
        if BOOT_ANIMATION_RUNNING.load(Ordering::Relaxed) {
            firmware::boot_animation::run(
                &FRAME_BUFFER_A,
                &FRAME_BUFFER_B,
                &ACTIVE_FRAME_BUFFER,
                &BOOT_ANIMATION_RUNNING,
            )
            .await;
        } else if SHUTDOWN_ANIMATION_RUNNING.load(Ordering::Relaxed) {
            firmware::shutdown_animation::run(
                &FRAME_BUFFER_A,
                &FRAME_BUFFER_B,
                &ACTIVE_FRAME_BUFFER,
                &SHUTDOWN_ANIMATION_RUNNING,
            )
            .await;
        } else {
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}

// [루프 6] 시스템 전원 및 리눅스 상태를 관리하는 비동기 루프
async fn system_loop(mut power: Power<'static>) {
    let mut status = Status::StandBy;
    let mut power_key_pressed = false;

    power.disable_cm();
    power.disable_pw12();
    BOOT_ANIMATION_RUNNING.store(false, Ordering::Relaxed);
    IS_4BIT_MODE.store(false, Ordering::Relaxed);

    loop {
        let event_opt = if power_key_pressed && (status == Status::LinuxActive || status == Status::LinuxBooting) {
            match with_timeout(Duration::from_secs(3), INTERNAL_EVENT_CHANNEL.receive()).await {
                Ok(ev) => Some(ev),
                Err(_) => {
                    info!("Power key held for 3 seconds. Starting shutdown...");
                    SHUTDOWN_ANIMATION_RUNNING.store(true, Ordering::Relaxed);
                    let _ = TX_CHANNEL.try_send(ResponseFromEsp32::RequestShutdown);
                    let _ = INTERNAL_EVENT_CHANNEL.try_send(InternalEvent::StartShutdown);
                    power_key_pressed = false;
                    None
                }
            }
        } else {
            Some(INTERNAL_EVENT_CHANNEL.receive().await)
        };

        if let Some(event) = event_opt {
            match event {
                InternalEvent::PowerKeyPressed => {
                    if status == Status::StandBy {
                        info!("Power key pressed. Booting Linux...");
                        status = Status::LinuxBooting;
                        IS_4BIT_MODE.store(false, Ordering::Relaxed);
                        power.enable_cm();
                        power.enable_pw12();
                        BOOT_ANIMATION_RUNNING.store(true, Ordering::Relaxed);
                    } else if status == Status::LinuxActive || status == Status::LinuxBooting {
                        power_key_pressed = true;
                    }
                }
                InternalEvent::PowerKeyReleased => {
                    power_key_pressed = false;
                }
                InternalEvent::StopBootAnimation => {
                    if status == Status::LinuxBooting {
                        info!("Linux boot finished. Stopping animation.");
                        status = Status::LinuxActive;
                        BOOT_ANIMATION_RUNNING.store(false, Ordering::Relaxed);
                    }
                }
                InternalEvent::StartShutdown => {
                    if status == Status::LinuxActive || status == Status::LinuxBooting {
                        info!("Shutting down Linux...");
                        // status = Status::LinuxShuttingDown;
                        IS_4BIT_MODE.store(false, Ordering::Relaxed);
                        BOOT_ANIMATION_RUNNING.store(false, Ordering::Relaxed);
                        SHUTDOWN_ANIMATION_RUNNING.store(true, Ordering::Relaxed);
                        Timer::after(Duration::from_secs(7)).await;
                        SHUTDOWN_ANIMATION_RUNNING.store(false, Ordering::Relaxed);
                        power.disable_cm();
                        power.disable_pw12();
                        status = Status::StandBy;
                        info!("System is now in StandBy mode.");
                    }
                }
            }
        }
    }
}
