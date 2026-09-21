use crate::api::API_VERSION;
use crate::api::display::DisplayInterface;
use crate::applet::{APP, CONTEXT, DEFAULT_TARGET_FPS, SpeechOption};
// use crate::event::payload::{Event, RawEvent};
use crate::event::{
    Event, EventHandler, EventResult, EventResultV1, EventV1, OnEventResult, UpdateResultV1,
};
use log::error;
// use crate::event::payload::RawEvent;

#[unsafe(no_mangle)]
pub extern "C" fn api_version() -> i32 {
    API_VERSION
}

/// WASM 런타임에서 애플릿의 목표 초당 프레임 수(FPS)를 조회할 때 호출되는 콜백 함수입니다.
/// 해당함수는 터미널과 웹 환경을 맞추기위해 추가되었습니다.
/// 이 함수는 C ABI를 통해 외부에서 호출될 수 있습니다.
///
/// 현재 실행 중인 애플릿이 존재하면 해당 애플릿의 `target_fps` 값을 반환하고,
/// 그렇지 않으면 기본값(`DEFAULT_TARGET_FPS`)을 반환합니다.
#[unsafe(no_mangle)]
pub extern "C" fn target_fps() -> i32 {
    if let Some(app) = APP.lock().as_ref() {
        app.target_fps() as i32
    } else {
        DEFAULT_TARGET_FPS as i32
    }
}

/// 호스트와 게스트 간 이벤트를 주고받기 위한 공유 메모리 버퍼입니다.
static mut EVENT_BUFFER: Vec<u8> = Vec::new();

/// 호스트가 WASM 메모리에 직접 접근하여 이벤트를 기록할 수 있도록
/// 이벤트 버퍼의 포인터를 반환하는 콜백 함수입니다.
#[unsafe(no_mangle)]
pub extern "C" fn get_event_buffer_ptr(buffer_size: i32) -> *mut u8 {
    let size = buffer_size as usize;
    unsafe {
        let buffer = &mut *std::ptr::addr_of_mut!(EVENT_BUFFER);
        if buffer.len() < size {
            buffer.resize(size, 0);
        }
        buffer.as_mut_ptr()
    }
}

/// 호스트와 게스트 간 이벤트 결과를 주고받기 위한 공유 메모리 버퍼입니다.
static mut EVENT_RESULT_BUFFER: Vec<u8> = Vec::new();

/// 호스트가 WASM 메모리에 직접 접근하여 이벤트를 기록할 수 있도록
/// 이벤트 버퍼의 포인터를 반환하는 콜백 함수입니다.
#[unsafe(no_mangle)]
pub extern "C" fn get_event_result_buffer_ptr(buffer_size: i32) -> *mut u8 {
    let size = buffer_size as usize;
    unsafe {
        let buffer = &mut *std::ptr::addr_of_mut!(EVENT_RESULT_BUFFER);
        if buffer.len() < size {
            buffer.resize(size, 0);
        }
        buffer.as_mut_ptr()
    }
}

/// WASM 런타임에서 메모리 버퍼에 이벤트 쓰기를 완료한 후 호출되는 콜백 함수입니다.
///
/// 이 함수는 C ABI를 통해 외부에서 호출될 수 있습니다.
/// 전달받은 `count` 만큼 버퍼를 순회하며 이벤트를 애플릿의 `on_event` 메서드로 전달합니다.
#[unsafe(no_mangle)]
pub extern "C" fn on_event() -> i32 {
    if let Some(app) = APP.lock().as_mut()
        && let Some(context) = CONTEXT.lock().as_mut()
    {
        let event: Event = unsafe {
            let buffer = &*std::ptr::addr_of!(EVENT_BUFFER);
            postcard::from_bytes(&buffer[..]).unwrap()
        };
        let event_result_buffer = unsafe { &mut *std::ptr::addr_of_mut!(EVENT_RESULT_BUFFER) };

        match event {
            Event::V1(event_v1) => match event_v1 {
                EventV1::Window(window_event) => {
                    context.window.handle_event(window_event);
                }
                EventV1::Keypad(keypad_event) => {
                    context.keypad.handle_event(keypad_event);
                    if context.keypad.is_help_just_triggered() {
                        let help = app.on_help(context);
                        let help_text = help.get_string_by_lang(&context.language);
                        if !help_text.is_empty() {
                            context.audio.speak_text_with_option(
                                help_text,
                                crate::applet::SpeechOption::important(),
                            );
                        }
                    }
                }



                EventV1::BrailleChord(chord) => {
                    context.keypad.handle_event(chord);
                }
                EventV1::HttpResponse {
                    request_id,
                    status_code,
                    body,
                } => {
                    context.http.handle_response(request_id, status_code, body);
                }
                EventV1::Start => {
                    if let Err(e) = app.on_start(context) {
                        error!("on_init 오류: {}", e);
                        context.audio.speak_text_with_option(
                            &format!("에러가 발생하여 애플릿을 종료합니다. {}", e),
                            SpeechOption::important(),
                        );
                        if let Err(err) = postcard::to_slice(
                            &EventResult::V1(EventResultV1::Update(UpdateResultV1::ExitApp)),
                            event_result_buffer,
                        ) {
                            error!("postcard error: {}", err);
                        } else {
                            return OnEventResult::HasResult.into();
                        }
                    }
                }
                EventV1::Stop => {
                    if let Err(e) = app.on_stop(context) {
                        error!("on_stop 오류: {}", e);
                        context.audio.speak_text_with_option(
                            &format!("에러가 발생하여 애플릿을 종료합니다. {}", e),
                            SpeechOption::important(),
                        );
                    }
                    // 앱을 종료
                    if let Err(e) = postcard::to_slice(
                        &EventResult::V1(EventResultV1::Update(UpdateResultV1::ExitApp)),
                        event_result_buffer,
                    ) {
                        error!("postcard error: {}", e);
                    } else {
                        return OnEventResult::HasResult.into();
                    };
                }
                EventV1::Update => {
                    let result = app.on_update(context);

                    // 애플릿이 프레임 내에 소비하지 않은 잔여 이벤트들을 모두 비워줍니다 (메모리 누수 및 입력 지연 방지)
                    context.keypad.clear();

                    // TTS 상태 변화를 감지하고 자동으로 오디오를 출력합니다.
                    // (APP과 CONTEXT의 Lock을 점유 중이므로 락을 인자로 받는 내부 함수를 안전하게 호출합니다)
                    crate::applet::process_tts_tracking(app.as_ref(), context);

                    match result {
                        Ok(update_result) => {
                            // 프레임 업데이트 결과가 렌더링을 요구하는 경우,
                            // 호스트의 Draw 이벤트를 기다리지 않고 즉시 SDK 단에서 그리기를 수행합니다.
                            if update_result == UpdateResultV1::NeedsRedraw {
                                let mut window = std::mem::take(&mut context.window);

                                // SDK 레벨에서 화면을 초기화합니다.
                                window.clear();

                                let draw_result = app.on_draw(&mut window);
                                context.window = window;

                                if let Err(e) = draw_result {
                                    error!("on_draw 오류: {}", e);
                                    context.audio.speak_text_with_option(
                                        &format!("에러가 발생하여 애플릿을 종료합니다. {}", e),
                                        SpeechOption::important(),
                                    );
                                    if let Err(err) = postcard::to_slice(
                                        &EventResult::V1(EventResultV1::Update(
                                            UpdateResultV1::ExitApp,
                                        )),
                                        event_result_buffer,
                                    ) {
                                        error!("postcard error: {}", err);
                                    }
                                    return OnEventResult::HasResult.into();
                                }
                            }

                            if let Err(e) = postcard::to_slice(
                                &EventResult::V1(EventResultV1::Update(update_result)),
                                event_result_buffer,
                            ) {
                                error!("postcard error: {}", e);
                            } else {
                                return OnEventResult::HasResult.into();
                            };
                        }
                        Err(e) => {
                            error!("on_update 오류: {}", e);
                            context.audio.speak_text_with_option(
                                &format!("에러가 발생하여 애플릿을 종료합니다. {}", e),
                                SpeechOption::important(),
                            );
                            if let Err(err) = postcard::to_slice(
                                &EventResult::V1(EventResultV1::Update(UpdateResultV1::ExitApp)),
                                event_result_buffer,
                            ) {
                                error!("postcard error: {}", err);
                            } else {
                                return OnEventResult::HasResult.into();
                            };
                        }
                    }
                }
                EventV1::Draw => {
                    let mut window = std::mem::take(&mut context.window);

                    // SDK 레벨에서 화면을 초기화합니다.
                    window.clear();

                    let result = app.on_draw(&mut window);
                    context.window = window;

                    if let Err(e) = result {
                        error!("on_draw 오류: {}", e);
                        context.audio.speak_text_with_option(
                            &format!("에러가 발생하여 애플릿을 종료합니다. {}", e),
                            SpeechOption::important(),
                        );
                        if let Err(err) = postcard::to_slice(
                            &EventResult::V1(EventResultV1::Update(UpdateResultV1::ExitApp)),
                            event_result_buffer,
                        ) {
                            error!("postcard error: {}", err);
                        } else {
                            return OnEventResult::HasResult.into();
                        }
                    }
                }
            },
        };
    }
    OnEventResult::NoResult.into()
}
