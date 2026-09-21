use js_sys::WebAssembly;
use leptos::html::Canvas;
use leptos::prelude::*;
use leptos_meta::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;

use crate::Result;
use crate::components::braille_display::BrailleDisplay;
use crate::runner::{AppletExports, AppletRunner, ImportClosures};
use crate::storage::Storage;

const BRAILLE_DISPLAY_WIDTH: f64 = 48.0;
const BRAILLE_DISPLAY_HEIGHT: f64 = 32.0;

#[component]
pub fn Main() -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let (app_settings, set_app_settings) = signal(sdk::types::AppSettings::default());
    let (current_applet, set_current_applet) = signal(None::<String>);
    let (_lang, set_lang) = signal(0);
    let stop_handle = StoredValue::new_local(None::<Box<dyn Fn()>>);
    let is_loaded = StoredValue::new_local(false); // 스토리지 로드 완료 여부를 체크하는 플래그

    let run_applet_handle = StoredValue::new_local(None::<Callback<String>>);

    // 반응형 시스템의 비동기 업데이트 지연을 방지하기 위한 즉각적인 상태 반영 셀
    let immediate_applet = StoredValue::new(None::<String>);

    // 메모리 상에 애플릿들을 저장
    let applets = Arc::new(Mutex::new(
        std::collections::HashMap::<String, Vec<u8>>::new(),
    ));
    let applets_for_load = applets.clone();
    let applets_for_run = applets.clone();

    // Load initial list and configs from storage
    Effect::new(move |_| {
        // 1. 애플릿 자동 로딩 (비동기)
        // 스토리지 접근 성공 여부와 무관하게 항상 최신 애플릿을 가져오도록 합니다.
        leptos::task::spawn_local(load_default_applets(
            applets_for_load.clone(),
            run_applet_handle,
            current_applet,
        ));

        // 2. 사용자 앱 설정(AppSettings) 불러오기
        match Storage::new() {
            Ok(storage) => match storage.get_app_settings() {
                Ok(settings) => set_app_settings.set(settings),
                Err(e) => leptos::logging::error!("Failed to load app settings: {}", e),
            },
            Err(e) => leptos::logging::error!("Failed to initialize storage: {}", e),
        }

        is_loaded.set_value(true); // 로드 완료 마킹
    });

    // app_settings 상태가 변경될 때마다 자동으로 스토리지에 저장합니다.
    Effect::new(move |_| {
        let settings = app_settings.get(); // 상태 구독 (변경 시 Effect 재실행)

        // 초기 데이터 로드가 완료된 이후에만 스토리지 저장을 수행합니다.
        if is_loaded.get_value()
            && let Ok(storage) = Storage::new()
        {
            let _ = storage.save_app_settings(&settings);
        }
    });

    // 설정 애플릿(WASM) 등에서 변경한 스토리지 설정을 감지하여 Signal 동기화
    Effect::new(move |_| {
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Ok(storage) = Storage::new()
                && let Ok(settings) = storage.get_app_settings()
            {
                set_lang.set(settings.language as i32);
                set_app_settings.set(settings);
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("setting_changed", cb.as_ref().unchecked_ref());
        }

        let cb_js_value = cb.into_js_value();

        on_cleanup(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "setting_changed",
                    cb_js_value.unchecked_ref(),
                );
            }
        });
    });

    let run_applet = Callback::new(move |name: String| {
        immediate_applet.set_value(Some(name.clone()));
        set_current_applet.set(Some(name.clone()));
        // Stop previous applet if running
        stop_handle.update_value(|stop| {
            if let Some(stop) = stop.take() {
                stop();
            }
        });

        if let Some(canvas) = canvas_ref.get_untracked() {
            if let Ok(Some(ctx)) = canvas.get_context("2d") {
                let ctx = ctx.unchecked_into::<web_sys::CanvasRenderingContext2d>();

                // Load bytes from memory
                let bytes_opt = applets_for_run.lock().unwrap().get(&name).cloned();
                match bytes_opt {
                    Some(bytes) => {
                        let stop_setter = stop_handle;
                        let applets_clone = applets_for_run.clone();
                        leptos::task::spawn_local(async move {
                            let on_start_applet = move |new_name: String| {
                                leptos::logging::log!(
                                    "Handling start request for applet: {}",
                                    new_name
                                );
                                // 애플릿 전환 콜백 실행 (기존 앱 정지 및 새 앱 로드)
                                run_applet_handle.with_value(|cb| {
                                    if let Some(run) = *cb {
                                        run.run(new_name);
                                    }
                                });
                            };

                            let current_name = name.clone();
                            if let Some(stop_fn) = run_applet_instance(
                                bytes,
                                ctx,
                                on_start_applet,
                                applets_clone,
                                current_name,
                            )
                            .await
                            {
                                stop_setter.set_value(Some(stop_fn));
                            }
                        });
                    }
                    None => {
                        leptos::logging::error!("Applet data not found for: {}", name);
                    }
                }
            } else {
                leptos::logging::error!("Failed to get 2d context");
            }
        } else {
            leptos::logging::error!("Canvas ref is missing");
        }
    });

    run_applet_handle.set_value(Some(run_applet));

    // 키보드 ESC 키나 애플릿 종료 시 발생하는 런처 복귀 이벤트를 수신합니다.
    Effect::new(move |_| {
        let run_cb = run_applet;
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
            run_cb.run("launcher".to_string());
        }) as Box<dyn FnMut(web_sys::Event)>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("request_launcher", cb.as_ref().unchecked_ref());
        }

        let cb_js_value = cb.into_js_value();

        on_cleanup(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "request_launcher",
                    cb_js_value.unchecked_ref(),
                );
            }
        });
    });

    // WebSocket 캡처 스트림 연동 Effect 추가
    let (ws_address, set_ws_address) = signal(
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|ls| ls.get_item("capture_ws_address").ok().flatten())
            .unwrap_or_else(|| "ws://127.0.0.1:3007".to_string()),
    );
    let (ws_status, set_ws_status) = signal("Disconnected".to_string());
    let (ws_status_color, set_ws_status_color) = signal("bg-red-500".to_string());
    let (reconnect_trigger, set_reconnect_trigger) = signal(0);
    Effect::new(move |_| {
        reconnect_trigger.track();
        let current_ws_address = ws_address.get();

        if let Some(canvas) = canvas_ref.get()
            && let Ok(Some(ctx)) = canvas.get_context("2d")
            && let Ok(ctx_clone) = ctx.dyn_into::<web_sys::CanvasRenderingContext2d>()
        {
            let on_disconnect = move || {
                set_ws_status.set("Disconnected".to_string());
                set_ws_status_color.set("bg-red-500".to_string());
                leptos::logging::log!("WebSocket connection lost. Reconnecting in 3 seconds...");
                let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                    set_reconnect_trigger.update(|n| *n += 1);
                });
                if let Some(window) = web_sys::window() {
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.unchecked_ref(),
                        3000,
                    );
                }
            };

            let on_connect = move || {
                set_ws_status.set("Connected".to_string());
                set_ws_status_color.set("bg-green-500".to_string());
            };

            let run_applet_cloned = run_applet_handle;

            set_ws_status.set("Connecting...".to_string());
            set_ws_status_color.set("bg-yellow-500".to_string());

            match crate::ws_comm::start_websocket_comm(
                &current_ws_address,
                move |name| {
                    leptos::logging::log!("Applet launch requested from PC: {}", name);
                    run_applet_cloned.with_value(|cb| {
                        if let Some(run) = *cb {
                            run.run(name);
                        }
                    });
                },
                move |width, height, data, bpp| {
                    if immediate_applet.with_value(|v| v.as_deref() == Some("mirror")) {
                        crate::display::draw_packed_data(&ctx_clone, width, height, &data, bpp);
                        true
                    } else {
                        false
                    }
                },
                on_connect,
                on_disconnect,
            ) {
                Ok((ws, closures)) => {
                    let closures_handle = StoredValue::new_local(Some(closures));
                    on_cleanup(move || {
                        leptos::logging::log!("Cleaning up WebSocket connection...");
                        ws.set_onclose(None);
                        ws.set_onerror(None);
                        ws.set_onmessage(None);
                        ws.set_onopen(None);
                        let _ = ws.close();
                        closures_handle.update_value(|c| {
                            *c = None;
                        });
                    });
                }
                Err(_) => {
                    // 초기 접속 실패시 3초 후 on_disconnect() 동작과 동일하게 재접속 유도
                    set_ws_status.set("Disconnected".to_string());
                    set_ws_status_color.set("bg-red-500".to_string());
                    let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                        set_reconnect_trigger.update(|n| *n += 1);
                    });
                    if let Some(window) = web_sys::window() {
                        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                            cb.unchecked_ref(),
                            3000,
                        );
                    }
                }
            }
        }
    });

    view! {
        <Title text=move || {
            if let Some(name) = current_applet.get() {
                format!("{} - Braille Display", name)
            } else {
                "Braille Display".to_string()
            }
        } />
        <main class="flex flex-col h-screen w-full">
            <div class="flex items-center p-2 bg-gray-100 border-b border-gray-300 gap-2 shrink-0">
                <span class="text-sm font-semibold">"PC Capture WS:"</span>
                <input
                    type="text"
                    class="border rounded px-2 py-1 text-sm w-64 focus:outline-none"
                    prop:value=ws_address
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        if let Some(window) = web_sys::window()
                            && let Ok(Some(ls)) = window.local_storage()
                        {
                            let _ = ls.set_item("capture_ws_address", &val);
                        }
                        set_ws_address.set(val);
                        set_reconnect_trigger.update(|n| *n += 1);
                    }
                    on:keydown=move |ev| ev.stop_propagation()
                    on:keyup=move |ev| ev.stop_propagation()
                />
                <div class=move || format!("w-3 h-3 rounded-full shadow-inner {}", ws_status_color.get())></div>
                <span class="text-xs text-gray-600 font-medium">{move || ws_status.get()}</span>
            </div>
            <BrailleDisplay
                node_ref=canvas_ref
                width=BRAILLE_DISPLAY_WIDTH
                height=BRAILLE_DISPLAY_HEIGHT
                current_applet=current_applet
            />
        </main>
    }
}

async fn load_default_applets(
    applets: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    run_applet_handle: StoredValue<Option<Callback<String>>, leptos::prelude::LocalStorage>,
    current_applet: ReadSignal<Option<String>>,
) {
    leptos::logging::log!("애플릿 다운로드를 시작합니다...");

    // reqwest는 절대 경로 URL을 요구하므로, 현재 브라우저의 origin을 가져옵니다.
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    // 브라우저 캐시를 무시하고 항상 최신 파일을 받기 위해 타임스탬프를 추가합니다.
    let timestamp = js_sys::Date::now() as u64;
    let list_url = format!("{}/applets/applets.json?t={}", origin, timestamp);

    // 서버에서 자동으로 생성된 applets.json 목록을 받아옵니다.
    match reqwest::get(&list_url).await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Vec<String>>().await {
                    Ok(applet_names) => {
                        leptos::logging::log!("applets.json 발견: {:?}", applet_names);
                        applets.lock().unwrap().clear(); // 이전 애플릿들을 완전히 삭제

                        for name in applet_names {
                            let url = format!("{}/applets/{}.wasm?t={}", origin, name, timestamp);
                            if let Ok(resp) = reqwest::get(&url).await {
                                if resp.status().is_success() {
                                    if let Ok(data) = resp.bytes().await {
                                        applets.lock().unwrap().insert(name.clone(), data.to_vec());
                                        leptos::logging::log!("애플릿 로드 완료: {}", name);
                                    }
                                } else {
                                    leptos::logging::error!(
                                        "{} 다운로드 실패: HTTP {}",
                                        name,
                                        resp.status()
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => leptos::logging::error!("applets.json 파싱 실패: {}", e),
                }
            } else {
                leptos::logging::error!("applets.json 응답 오류: HTTP {}", resp.status());
            }
        }
        Err(e) => {
            leptos::logging::error!("applets.json 요청 중 네트워크 에러 발생: {}", e);
        }
    }

    // 애플릿 로딩이 끝난 후, 아직 실행 중인 애플릿이 없고 launcher가 존재한다면 자동 실행합니다.
    if current_applet.get_untracked().is_none() {
        if applets.lock().unwrap().contains_key("launcher") {
            run_applet_handle.with_value(|cb| {
                if let Some(run) = *cb {
                    leptos::logging::log!("런처(launcher) 애플릿을 자동 실행합니다.");
                    run.run("launcher".to_string());
                }
            });
        } else {
            leptos::logging::warn!("다운로드된 애플릿 중 launcher가 없습니다.");
        }
    }
}

async fn run_applet_instance(
    bytes: Vec<u8>,
    ctx: web_sys::CanvasRenderingContext2d,
    on_start_applet: impl FnMut(String) + 'static,
    applets: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    current_app_name: String,
) -> Option<Box<dyn Fn()>> {
    // Clear canvas before running new applet
    let canvas = ctx.canvas().unwrap();
    ctx.set_fill_style_str("black");
    ctx.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

    let event_queue = runtime_common::event::EventChannel::new();
    let memory = Rc::new(RefCell::new(None::<WebAssembly::Memory>));
    let (imports, import_closures) = match create_imports(
        &ctx,
        memory.clone(),
        on_start_applet,
        applets,
        current_app_name,
        event_queue.sender(),
    ) {
        Ok(res) => res,
        Err(e) => {
            leptos::logging::error!("Failed to create imports: {}", e);
            return None;
        }
    };

    let promise = WebAssembly::instantiate_buffer(&bytes, &imports);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(result) => {
            let instance = js_sys::Reflect::get(&result, &"instance".into()).unwrap();
            let instance = instance.unchecked_into::<WebAssembly::Instance>();
            let exports_obj = instance.exports();
            let mem = js_sys::Reflect::get(&exports_obj, &"memory".into()).unwrap();
            *memory.borrow_mut() = Some(mem.unchecked_into());

            match AppletExports::new(&exports_obj) {
                Ok(exports) => {
                    match AppletRunner::new(exports, ctx, import_closures, event_queue).run() {
                        Ok(stop_fn) => Some(stop_fn),
                        Err(e) => {
                            leptos::logging::error!("{}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    leptos::logging::error!("{}", e);
                    None
                }
            }
        }
        Err(e) => {
            leptos::logging::error!("Failed to instantiate WASM module: {:?}", e);
            None
        }
    }
}

fn create_imports(
    ctx: &web_sys::CanvasRenderingContext2d,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
    on_start_applet: impl FnMut(String) + 'static,
    applets: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    current_app_name: String,
    event_sender: crossbeam_channel::Sender<sdk::event::Event>,
) -> Result<(js_sys::Object, ImportClosures)> {
    let imports = js_sys::Object::new();

    // Display module
    let display_closures = crate::display::setup_imports(&imports, ctx)?;

    // Log module
    let log_closures = crate::log::setup_imports(&imports, memory.clone())?;

    // Audio module
    let audio_closures = crate::audio::wasm_bindings::setup_imports(&imports, memory.clone())?;

    // Time module
    let time_closures = crate::time::setup_imports(&imports)?;

    // Keypad module
    let keypad_closures = crate::keypad::setup_imports(&imports)?;

    // Applet manager module
    let applet_manager_closures = crate::applet_manager::setup_imports(
        &imports,
        memory.clone(),
        on_start_applet,
        applets,
        current_app_name,
    )?;

    let preferences_closure = crate::preferences::setup_imports(&imports, memory.clone())?;

    // Network module
    let http_closures = crate::http::setup_imports(&imports, memory.clone(), event_sender)?;

    Ok((
        imports,
        ImportClosures {
            _set_pin: display_closures.set_pin,
            _message: log_closures.message,
            _get_time_seconds: time_closures.get_time_seconds,
            _get_timezone_offset_seconds: time_closures.get_timezone_offset_seconds,
            _get_monotonic_time_nanos: time_closures.get_monotonic_time_nanos,
            _play: audio_closures.play,
            _play_sequence: audio_closures.play_sequence,
            _list_applets: applet_manager_closures.list_applets,
            _start_applet: applet_manager_closures.start_applet,
            _get_current_applet: applet_manager_closures.get_current_applet,
            _preferences: preferences_closure,
            _keypad: keypad_closures,
            _http: http_closures,
        },
    ))
}
