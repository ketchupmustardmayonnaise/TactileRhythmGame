use leptos::prelude::*;
use std::collections::HashSet;
use wasm_bindgen::JsCast;

fn dispatch_key_event(type_: &str, code: &str) {
    if let Some(window) = web_sys::window() {
        let init = web_sys::KeyboardEventInit::new();
        init.set_code(code);
        init.set_bubbles(true);
        init.set_cancelable(true);
        if let Ok(event) = web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(type_, &init) {
            let _ = window.dispatch_event(&event);
        }
    }
}

fn normalize_key_code(code: &str) -> &str {
    code
}

#[component]
fn KeyButton(
    label: &'static str,
    code: &'static str,
    class: &'static str,
    active_keys: RwSignal<HashSet<String>>,
) -> impl IntoView {
    view! {
        <button
            class=move || format!(
                "bg-gray-300 hover:bg-gray-400 flex items-center justify-center font-bold text-sm select-none transition-all duration-75 {} {}",
                class,
                if active_keys.with(|keys| keys.contains(code)) {
                    "!brightness-75 scale-[0.96] shadow-[inset_0_5px_10px_rgba(0,0,0,0.4)] translate-y-[2px]"
                } else {
                    "shadow-[0_4px_6px_rgba(0,0,0,0.2),inset_0_2px_0_rgba(255,255,255,0.6)]"
                }
            )
            on:mousedown=move |ev| {
                ev.prevent_default();
                dispatch_key_event("keydown", code);
            }
            on:mouseup=move |ev| {
                ev.prevent_default();
                dispatch_key_event("keyup", code);
            }
            on:mouseleave=move |ev| {
                if ev.buttons() > 0 {
                    dispatch_key_event("keyup", code);
                }
            }
            on:touchstart=move |ev| {
                ev.prevent_default();
                dispatch_key_event("keydown", code);
            }
            on:touchend=move |ev| {
                ev.prevent_default();
                dispatch_key_event("keyup", code);
            }
        >
            <span class="pointer-events-none block">{label}</span>
        </button>
    }
}

#[component]
pub fn VirtualKeypad(
    children: Children,
    current_applet: ReadSignal<Option<String>>,
) -> impl IntoView {
    let (is_perkins, set_is_perkins) = signal(false);

    let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        let current = crate::keypad::PERKINS_MODE.with(|p| p.get());
        if is_perkins.get_untracked() != current {
            set_is_perkins.set(current);
        }
    }) as Box<dyn FnMut()>);

    let window = web_sys::window().unwrap();
    let id = window
        .set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 200)
        .unwrap();

    // Closure 객체를 반응형 스코프의 스레드 로컬 저장소에 보관하여 컴포넌트가 제거될 때 같이 Drop되도록 합니다.
    let _stored_cb = StoredValue::new_local(cb);

    let active_keys = RwSignal::new(HashSet::<String>::new());

    let keydown_cb =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
            active_keys.update(|keys| {
                let code = ev.code();
                if code == "Escape" {
                    keys.insert("PowerLeft".to_string());
                    keys.insert("PowerRight".to_string());
                }
                keys.insert(normalize_key_code(&code).to_string());
            });
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

    let keyup_cb =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
            active_keys.update(|keys| {
                let code = ev.code();
                if code == "Escape" {
                    keys.remove("PowerLeft");
                    keys.remove("PowerRight");
                }
                keys.remove(normalize_key_code(&code));
            });
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

    let last_tts = RwSignal::new(String::new());
    let tts_cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: web_sys::CustomEvent| {
        if let Some(detail) = ev.detail().as_string() {
            last_tts.set(detail);
        }
    })
        as Box<dyn FnMut(web_sys::CustomEvent)>);

    let app_settings = RwSignal::new(
        crate::storage::Storage::new()
            .and_then(|s| s.get_app_settings())
            .unwrap_or_default(),
    );

    let save_and_dispatch = move |settings: &sdk::types::AppSettings| {
        if let Ok(storage) = crate::storage::Storage::new() {
            let _ = storage.save_app_settings(settings);
        }
        if let Some(window) = web_sys::window()
            && let Ok(event) = web_sys::CustomEvent::new("setting_changed")
        {
            let _ = window.dispatch_event(&event);
        }
    };

    let settings_cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
        if let Ok(storage) = crate::storage::Storage::new()
            && let Ok(settings) = storage.get_app_settings()
        {
            app_settings.set(settings);
        }
    })
        as Box<dyn FnMut(web_sys::Event)>);

    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("keydown", keydown_cb.as_ref().unchecked_ref());
        let _ = window.add_event_listener_with_callback("keyup", keyup_cb.as_ref().unchecked_ref());
        let _ =
            window.add_event_listener_with_callback("tts_played", tts_cb.as_ref().unchecked_ref());
        let _ = window.add_event_listener_with_callback(
            "setting_changed",
            settings_cb.as_ref().unchecked_ref(),
        );
    }

    let _stored_kd = StoredValue::new_local(keydown_cb);
    let _stored_ku = StoredValue::new_local(keyup_cb);
    let _stored_tts = StoredValue::new_local(tts_cb);
    let _stored_settings = StoredValue::new_local(settings_cb);

    on_cleanup(move || {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(id);
            _stored_kd.with_value(|cb| {
                let _ = window
                    .remove_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
            });
            _stored_ku.with_value(|cb| {
                let _ = window
                    .remove_event_listener_with_callback("keyup", cb.as_ref().unchecked_ref());
            });
            _stored_tts.with_value(|cb| {
                let _ = window
                    .remove_event_listener_with_callback("tts_played", cb.as_ref().unchecked_ref());
            });
            _stored_settings.with_value(|cb| {
                let _ = window.remove_event_listener_with_callback(
                    "setting_changed",
                    cb.as_ref().unchecked_ref(),
                );
            });
        }
    });

    view! {
        <div class="flex flex-col items-center gap-3 p-6 bg-white rounded-xl shadow-inner border border-gray-200 my-auto">

            <div class="flex flex-row items-center gap-4">
                // Left Keypad
                <div class="flex flex-col items-center gap-4">
                    // Power button
                    <div class="flex flex-col items-center gap-1">
                        <KeyButton label="ESC" code="PowerLeft" class="w-8 h-8 rounded-full text-[8px] bg-red-200 hover:bg-red-300" active_keys=active_keys />
                        <span class="text-[10px] text-gray-500 font-semibold">"Home"</span>
                    </div>

                    // D-pad + Center (Donut shape)
                    <div class="relative w-32 h-32 rounded-full bg-gray-100 shadow-lg">
                        <div class="absolute inset-0 rotate-45">
                            // Up (Top-Left in rotated container)
                            <div class="absolute top-0 left-0">
                                <KeyButton label="W" code="KeyW" class="w-16 h-16 rounded-tl-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Right (Top-Right in rotated container)
                            <div class="absolute top-0 right-0">
                                <KeyButton label="D" code="KeyD" class="w-16 h-16 rounded-tr-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Down (Bottom-Right in rotated container)
                            <div class="absolute bottom-0 right-0">
                                <KeyButton label="S" code="KeyS" class="w-16 h-16 rounded-br-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Left (Bottom-Left in rotated container)
                            <div class="absolute bottom-0 left-0">
                                <KeyButton label="A" code="KeyA" class="w-16 h-16 rounded-bl-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                        </div>
                        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
                            <KeyButton label="Space" code="Space" class="w-14 h-14 rounded-full text-[10px] bg-blue-200 hover:bg-blue-300 active:bg-blue-400 border-4 border-white" active_keys=active_keys />
                        </div>
                    </div>

                    // Bottom buttons (End on left, Home on right)
                    <div class="flex gap-8 mt-2">
                        <div class="flex flex-col items-center gap-1">
                            <KeyButton label="Q" code="KeyQ" class="w-12 h-12 rounded-full" active_keys=active_keys />
                            <span class="text-[10px] text-gray-500 font-semibold">"Menu"</span>
                        </div>
                        <div class="flex flex-col items-center gap-1">
                            <KeyButton label="E" code="KeyE" class="w-12 h-12 rounded-full" active_keys=active_keys />
                            <span class="text-[10px] text-gray-500 font-semibold">"Function"</span>
                        </div>
                    </div>
                </div>

                // Center Area (Canvas)
                <div class="flex flex-col items-center gap-6">
                    {children()}
                </div>

                // Right Keypad
                <div class="flex flex-col items-center gap-4">
                    // Power button
                    <div class="flex flex-col items-center gap-1">
                        <KeyButton label="ESC" code="PowerRight" class="w-8 h-8 rounded-full text-[8px] bg-red-200 hover:bg-red-300" active_keys=active_keys />
                        <span class="text-[10px] text-gray-500 font-semibold">"Home"</span>
                    </div>

                    // D-pad + Center (Donut shape)
                    <div class="relative w-32 h-32 rounded-full bg-gray-100 shadow-lg">
                        <div class="absolute inset-0 rotate-45">
                            // Up (Top-Left in rotated container)
                            <div class="absolute top-0 left-0">
                                <KeyButton label="↑" code="ArrowUp" class="w-16 h-16 rounded-tl-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Right (Top-Right in rotated container)
                            <div class="absolute top-0 right-0">
                                <KeyButton label="→" code="ArrowRight" class="w-16 h-16 rounded-tr-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Down (Bottom-Right in rotated container)
                            <div class="absolute bottom-0 right-0">
                                <KeyButton label="↓" code="ArrowDown" class="w-16 h-16 rounded-br-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                            // Left (Bottom-Left in rotated container)
                            <div class="absolute bottom-0 left-0">
                                <KeyButton label="←" code="ArrowLeft" class="w-16 h-16 rounded-bl-full border-2 border-white [&>span]:-rotate-45" active_keys=active_keys />
                            </div>
                        </div>
                        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
                            <KeyButton label="Enter" code="Enter" class="w-14 h-14 rounded-full text-[10px] bg-blue-200 hover:bg-blue-300 active:bg-blue-400 border-4 border-white" active_keys=active_keys />
                        </div>
                    </div>

                    // Bottom buttons (Home on left, End on right)
                    <div class="flex gap-8 mt-2">
                        <div class="flex flex-col items-center gap-1">
                            <KeyButton label="Ins" code="Insert" class="w-12 h-12 rounded-full text-xs" active_keys=active_keys />
                            <span class="text-[10px] text-gray-500 font-semibold">"Function"</span>
                        </div>
                        <div class="flex flex-col items-center gap-1">
                            <KeyButton label="Del" code="Delete" class="w-12 h-12 rounded-full text-xs" active_keys=active_keys />
                            <span class="text-[10px] text-gray-500 font-semibold">"Menu"</span>
                        </div>
                    </div>
                </div>
            </div>

            // Applet Name & TTS Output Display
            <div class="w-full flex flex-row items-stretch gap-4">
                <div class="w-32 shrink-0 h-16 p-2 bg-gray-50 rounded-lg border border-gray-200 flex flex-col justify-center text-center shadow-inner overflow-hidden">
                    <div class="text-gray-600 font-bold text-sm tracking-wider uppercase line-clamp-2 text-ellipsis overflow-hidden break-words w-full">
                        {move || current_applet.get().unwrap_or_else(|| "System".to_string())}
                    </div>
                </div>
                <div class="w-[39rem] shrink-0 h-16 p-2 bg-gray-50 rounded-lg border border-gray-200 flex flex-col text-center shadow-inner overflow-y-auto">
                    <div class=move || if last_tts.get().is_empty() { "text-gray-400 italic text-sm whitespace-normal break-all w-full m-auto" } else { "text-blue-600 font-semibold text-sm whitespace-normal break-all w-full m-auto" }>
                        {move || {
                            let text = last_tts.get();
                            if text.is_empty() {
                                "Waiting for TTS...".to_string()
                            } else {
                                text
                            }
                        }}
                    </div>
                </div>
            </div>

            // App Settings Display
            <div class="w-full flex flex-col items-center gap-2">
                <div class="w-full flex flex-row justify-around p-3 bg-gray-50 rounded-lg border border-gray-200 shadow-inner">
                    <div class="flex flex-col items-center">
                        <span class="text-[10px] font-bold text-gray-400 uppercase tracking-wider">"Language"</span>
                        <select
                            class="text-sm font-semibold text-gray-600 bg-transparent text-center cursor-pointer outline-none hover:text-blue-500"
                            on:change=move |ev| {
                                if let Some(target) = ev.target() {
                                    let val = target.unchecked_into::<web_sys::HtmlSelectElement>().value().parse::<i32>().unwrap_or(0);
                                    app_settings.update(|s| {
                                        s.language = sdk::types::Language::from(val);
                                        save_and_dispatch(s);
                                    });
                                }
                            }
                            prop:value=move || (app_settings.get().language as i32).to_string()
                        >
                            <option value="0">"Ko"</option>
                            <option value="1">"En"</option>
                        </select>
                    </div>

                    <div class="flex flex-col items-center">
                        <span class="text-[10px] font-bold text-gray-400 uppercase tracking-wider">"Volume"</span>
                        <select
                            class="text-sm font-semibold text-gray-600 bg-transparent text-center cursor-pointer outline-none hover:text-blue-500"
                            on:change=move |ev| {
                                if let Some(target) = ev.target() {
                                    let val = target.unchecked_into::<web_sys::HtmlSelectElement>().value().parse::<f32>().unwrap_or(50.0);
                                    app_settings.update(|s| {
                                        s.volume = val;
                                        save_and_dispatch(s);
                                    });
                                }
                            }
                            prop:value=move || app_settings.get().volume.to_string()
                        >
                            <option value="10">"10%"</option>
                            <option value="20">"20%"</option>
                            <option value="30">"30%"</option>
                            <option value="40">"40%"</option>
                            <option value="50">"50%"</option>
                            <option value="60">"60%"</option>
                            <option value="70">"70%"</option>
                            <option value="80">"80%"</option>
                            <option value="90">"90%"</option>
                            <option value="100">"100%"</option>
                        </select>

                    </div>
                    <div class="flex flex-col items-center">
                        <span class="text-[10px] font-bold text-gray-400 uppercase tracking-wider">"Mode"</span>
                        <span
                            class=move || format!(
                                "px-2 mt-0.5 rounded text-xs font-bold text-white transition-colors {}",
                                if is_perkins.get() { "bg-blue-500" } else { "bg-green-500" }
                            )
                        >
                            {move || if is_perkins.get() { "PERKINS" } else { "STANDARD" }}
                        </span>
                    </div>
                </div>
            </div>
        </div>
    }
}
