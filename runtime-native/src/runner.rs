use crate::host::HostState;
use anyhow::Result;
use postcard::experimental::max_size::MaxSize;
use sdk::api::display::BoundsRect;
use sdk::api::display::Size;
use sdk::api::window::WindowEventV1;
use sdk::event::OnEventResult;
use sdk::event::UpdateResultV1;
use sdk::event::{Event, EventResult, EventResultV1, EventV1};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;
use wasmtime::{Store, TypedFunc};

/// WASM 애플릿의 메인 실행 루프를 담당합니다.
/// `run`, `on_update`, `on_draw` 함수는 WASM 모듈에서 내보낸 함수들입니다.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    mut store: Store<HostState>,
    memory: wasmtime::Memory,                   // WASM 메모리 인스턴스
    run: &TypedFunc<(), ()>,                    // WASM 애플릿의 초기화 함수
    on_event: &TypedFunc<(), i32>,              // WASM 애플릿에 이벤트 전달 함수
    get_event_buffer_ptr: &TypedFunc<i32, i32>, // 버퍼 포인터 획득 함수
    get_event_result_buffer_ptr: &TypedFunc<i32, i32>, // 버퍼 포인터 획득 함수
    target_fps: &TypedFunc<(), i32>,            // 목표 프레임 속도를 반환하는 함수
    applet_receiver: &mut UnboundedReceiver<String>, // 앱 전환 신호 수신 채널
    benchmark: bool,
) -> Result<(Store<HostState>, Option<String>)> {
    // WASM 애플릿의 초기화 함수를 호출합니다.
    run.call(&mut store, ())?;

    // 이벤트 버퍼 주소를 가져옵니다.
    let event_buffer_ptr =
        get_event_buffer_ptr.call(&mut store, Event::POSTCARD_MAX_SIZE as i32)? as usize;
    let event_result_buffer_ptr = get_event_result_buffer_ptr
        .call(&mut store, EventResult::POSTCARD_MAX_SIZE as i32)?
        as usize;
    let event_result_bytes = &mut [0; EventResult::POSTCARD_MAX_SIZE];

    store.data_mut().display.clear();

    let Size { width, height } = store.data().display.get_size();
    let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Window(
        WindowEventV1::Resize(BoundsRect::new((0, 0).into(), (width, height).into())),
    )))?;
    memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
    on_event.call(&mut store, ())?;

    let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Start))?;
    memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
    on_event.call(&mut store, ())?;

    // 초기 렌더링을 수행합니다.
    let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Draw))?;
    memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
    on_event.call(&mut store, ())?;
    store.data_mut().display.show()?; // 디스플레이에 변경 사항을 표시합니다.

    // 프레임당 목표 지속 시간을 설정합니다 (예: 16ms = 60 FPS).
    let target_fps = target_fps.call(&mut store, ())?;
    let frame_duration = Duration::from_millis(1000 / target_fps as u64);

    let mut fps_counter = 0;
    let mut last_fps_time = Instant::now();
    // 메인 루프를 시작합니다.
    loop {
        // 앱 전환 신호가 있는지 확인합니다.
        if let Ok(new_app) = applet_receiver.try_recv() {
            // 앱을 교체하기 전 안전하게 Stop 이벤트를 보냅니다.
            let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Stop))?;
            memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
            on_event.call(&mut store, ())?;
            return Ok((store, Some(new_app))); // 새 앱 이름과 함께 루프를 빠져나옵니다.
        }

        while let Some(event) = store.data().event_channel.pop() {
            let bytes_to_write = postcard::to_allocvec(&event)?;
            memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
            if let OnEventResult::HasResult = on_event.call(&mut store, ())?.into() {
                memory.read(
                    &mut store,
                    event_result_buffer_ptr,
                    &mut event_result_bytes[..],
                )?;
                let event_result: EventResult = postcard::from_bytes(event_result_bytes)?;
                if let EventResult::V1(EventResultV1::Update(UpdateResultV1::ExitApp)) =
                    event_result
                {
                    let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Stop))?;
                    memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
                    on_event.call(&mut store, ())?;
                    return Ok((store, None));
                }
            }
        }

        // 현재 프레임 시작 시간을 기록합니다.
        let frame_start = Instant::now();

        // WASM 애플릿의 `on_update` 함수를 호출하고 결과를 `UpdateResult` 열거형으로 변환합니다.
        let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Update))?;
        memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
        if let OnEventResult::HasResult = on_event.call(&mut store, ())?.into() {
            if benchmark {
                fps_counter += 1;
                let now = Instant::now();
                if now.duration_since(last_fps_time).as_secs() >= 1 {
                    tracing::info!("FPS: {}", fps_counter);
                    fps_counter = 0;
                    last_fps_time = now;
                }
            }

            memory.read(
                &mut store,
                event_result_buffer_ptr,
                &mut event_result_bytes[..],
            )?;

            let event_result: EventResult = postcard::from_bytes(event_result_bytes)?;

            let EventResult::V1(EventResultV1::Update(update_result)) = event_result;
            match update_result {
                UpdateResultV1::NeedsRedraw => {
                    // [렌더링 최적화]
                    // SDK 내부에서 NeedsRedraw를 반환하기 전에 이미 스스로 on_draw()를 호출하여 화면 버퍼 렌더링을 마칩니다.
                    // 따라서 호스트는 WASM으로 다시 Draw 이벤트를 보낼 필요 없이(이벤트 왕복 오버헤드 제거),
                    // 이미 업데이트된 디스플레이 버퍼를 실제 장치에 출력(show)하기만 하면 됩니다.
                    store.data_mut().display.show()?;
                }
                UpdateResultV1::Unchanged => {
                    // 화면 변경이 없어도 통신 유실 보정(Self-healing) 카운터가 동작할 수 있도록 show()를 호출합니다.
                    store.data_mut().display.show()?;
                }
                UpdateResultV1::ExitApp => {
                    let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Stop))?;
                    memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
                    on_event.call(&mut store, ())?;
                    return Ok((store, None)); // 애플리케이션을 완전히 종료합니다.
                }
            }
        }

        // 현재 프레임에 소요된 시간을 계산합니다.
        let elapsed = frame_start.elapsed();
        // 프레임 지속 시간보다 적게 소요되었다면, 남은 시간 동안 스레드를 슬립하여 프레임 속도를 조절합니다.
        if elapsed < frame_duration {
            tokio::time::sleep(frame_duration - elapsed).await;
        }
    }

    // let bytes_to_write = postcard::to_allocvec(&Event::V1(EventV1::Stop))?;
    // memory.write(&mut store, event_buffer_ptr, &bytes_to_write)?;
    // on_event.call(&mut store, ())?;

    // Ok((store, None))
}
