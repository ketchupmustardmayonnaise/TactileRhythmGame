use crate::{Error, Result};
use runtime_common::event::EventChannel;
use sdk::api::keypad::{KeyState, KeypadEventV1, KeypadSide};
use sdk::event::{Event, EventResult, EventResultV1, EventV1, UpdateResult};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

pub struct ImportClosures {
    pub _set_pin: Closure<dyn FnMut(i32, i32, i32, bool)>,
    pub _message: Closure<dyn FnMut(i32, i32, i32)>,
    pub _get_time_seconds: Closure<dyn FnMut() -> u64>,
    pub _get_timezone_offset_seconds: Closure<dyn FnMut() -> i32>,
    pub _get_monotonic_time_nanos: Closure<dyn FnMut() -> u64>,
    pub _play: Closure<dyn FnMut(i32, i32)>,
    pub _play_sequence: Closure<dyn FnMut(i32, i32)>,
    pub _list_applets: Closure<dyn FnMut(i32, i32) -> i32>,
    pub _start_applet: Closure<dyn FnMut(i32, i32) -> i32>,
    pub _get_current_applet: Closure<dyn FnMut(i32, i32) -> i32>,
    pub _preferences: crate::preferences::PreferencesClosure,
    pub _keypad: crate::keypad::KeypadClosures,
    pub _http: crate::http::HttpClosures,
}

#[derive(Clone)]
pub struct AppletExports {
    run: js_sys::Function,
    on_event: js_sys::Function,
    get_event_buffer_ptr: js_sys::Function,
    get_event_result_buffer_ptr: js_sys::Function,
    memory: js_sys::WebAssembly::Memory,
}

impl AppletExports {
    pub fn new(exports: &js_sys::Object) -> Result<Self> {
        let get = |name: &str| {
            js_sys::Reflect::get(exports, &name.into())
                .map_err(|e| Error::ExportError(name.to_string(), format!("{:?}", e)))?
                .dyn_into::<js_sys::Function>()
                .map_err(|_| Error::NotExportFunctionError(name.to_string()))
        };

        let memory = js_sys::Reflect::get(exports, &"memory".into())
            .map_err(|e| Error::ExportError("memory".to_string(), format!("{:?}", e)))?
            .dyn_into::<js_sys::WebAssembly::Memory>()
            .map_err(|_| Error::NotExportFunctionError("memory".to_string()))?;

        Ok(AppletExports {
            run: get("run")?,
            on_event: get("on_event")?,
            get_event_buffer_ptr: get("get_event_buffer_ptr")?,
            get_event_result_buffer_ptr: get("get_event_result_buffer_ptr")?,
            memory,
        })
    }
}

pub struct AppletRunner {
    exports: AppletExports,
    ctx: web_sys::CanvasRenderingContext2d,
    _import_closures: ImportClosures,
    event_queue: EventChannel,
}

impl AppletRunner {
    pub fn new(
        exports: AppletExports,
        ctx: web_sys::CanvasRenderingContext2d,
        import_closures: ImportClosures,
        event_queue: EventChannel,
    ) -> Self {
        Self {
            exports,
            ctx,
            _import_closures: import_closures,
            event_queue,
        }
    }

    pub fn run(self) -> Result<Box<dyn Fn()>> {
        self.initialize()?;
        let guards = self.setup_event_listeners();
        self.start_loop(guards)
    }

    fn initialize(&self) -> Result<()> {
        crate::display::clear_pin_cache();
        self.exports
            .run
            .call0(&JsValue::NULL)
            .map_err(|e| Error::ExportFunctionRunError("run".to_string(), format!("{:?}", e)))?;

        let canvas = self.ctx.canvas().unwrap();
        let width = canvas.width() as i16;
        let height = canvas.height() as i16;

        let bounds = sdk::api::display::BoundsRect {
            top_left: sdk::api::display::Point { x: 0, y: 0 },
            size: sdk::api::display::Size {
                width: width / 10,
                height: height / 10,
            },
        };

        let _ = self.send_event(Event::V1(EventV1::Window(
            sdk::api::window::WindowEventV1::Resize(bounds),
        )));
        let _ = self.send_event(Event::V1(EventV1::Start));
        let _ = self.send_event(Event::V1(EventV1::Draw));

        leptos::logging::log!("Successfully instantiated WASM module");
        Ok(())
    }

    fn setup_event_listeners(&self) -> Vec<EventListenerGuard> {
        let window = web_sys::window().unwrap();
        let target: web_sys::EventTarget = window.unchecked_into();

        let accumulated_chord = std::rc::Rc::new(std::cell::Cell::new(0u8));
        let waiting_all_released = std::rc::Rc::new(std::cell::Cell::new(false));
        let perkins_pressed_count = std::rc::Rc::new(std::cell::Cell::new(0i32));

        let keydown = {
            let sender = self.event_queue.sender();
            let accumulated_chord = accumulated_chord.clone();
            let waiting_all_released = waiting_all_released.clone();
            let perkins_pressed_count = perkins_pressed_count.clone();

            EventListenerGuard::new(
                target.clone(),
                "keydown",
                Box::new(move |e: web_sys::Event| {
                    if let Ok(ke) = e.dyn_into::<web_sys::KeyboardEvent>() {
                        // ESC 키 또는 가상 키패드의 전원 버튼을 누르면 런처로 복귀
                        if ke.code() == "Escape"
                            || ke.code() == "PowerLeft"
                            || ke.code() == "PowerRight"
                        {
                            ke.prevent_default();
                            if let Some(window) = web_sys::window()
                                && let Ok(event) = web_sys::CustomEvent::new("request_launcher")
                            {
                                let _ = window.dispatch_event(&event);
                            }
                            return;
                        }

                        if !ke.repeat() {
                            let is_perkins = crate::keypad::PERKINS_MODE.with(|p| p.get());
                            if is_perkins && let Some(dot) = Self::map_perkins_dot(&ke.code()) {
                                ke.prevent_default();
                                perkins_pressed_count.set(perkins_pressed_count.get() + 1);
                                if !waiting_all_released.get() {
                                    accumulated_chord
                                        .set(accumulated_chord.get() | (1u8 << (dot - 1)));
                                }
                                return;
                            }

                            if let Some((key_code, side)) = Self::map_key_code(&ke.code()) {
                                ke.prevent_default();
                                let event = Event::V1(EventV1::Keypad(KeypadEventV1 {
                                    code: key_code,
                                    state: KeyState::Pressed,
                                    side,
                                }));
                                let _ = sender.send(event);
                            }
                        }
                    }
                }),
            )
        };

        let keyup = {
            let sender = self.event_queue.sender();
            let accumulated_chord = accumulated_chord.clone();
            let waiting_all_released = waiting_all_released.clone();
            let perkins_pressed_count = perkins_pressed_count.clone();

            EventListenerGuard::new(
                target.clone(),
                "keyup",
                Box::new(move |e: web_sys::Event| {
                    if let Ok(ke) = e.dyn_into::<web_sys::KeyboardEvent>() {
                        let is_perkins = crate::keypad::PERKINS_MODE.with(|p| p.get());
                        if is_perkins && let Some(_dot) = Self::map_perkins_dot(&ke.code()) {
                            ke.prevent_default();
                            let count = perkins_pressed_count.get().saturating_sub(1);
                            perkins_pressed_count.set(count);

                            if !waiting_all_released.get() && accumulated_chord.get() != 0 {
                                let _ = sender.send(Event::V1(EventV1::BrailleChord(
                                    accumulated_chord.get(),
                                )));
                                waiting_all_released.set(true);
                            }

                            if count == 0 {
                                waiting_all_released.set(false);
                                accumulated_chord.set(0);
                            }
                            return;
                        }

                        if let Some((key_code, side)) = Self::map_key_code(&ke.code()) {
                            ke.prevent_default();
                            let _ = sender.send(Event::V1(EventV1::Keypad(KeypadEventV1 {
                                code: key_code,
                                state: KeyState::Released,
                                side,
                            })));
                        }
                    }
                }),
            )
        };

        let setting_changed = EventListenerGuard::new(
            target.clone(),
            "setting_changed",
            Box::new(move |_e: web_sys::Event| {
                crate::preferences::apply_current_volume();
            }),
        );

        vec![keydown, keyup, setting_changed]
    }

    fn start_loop(self, guards: Vec<EventListenerGuard>) -> Result<Box<dyn Fn()>> {
        let f = Rc::new(RefCell::new(None));
        let g = f.clone();

        let is_running = Rc::new(std::cell::Cell::new(true));
        let is_running_clone = is_running.clone();

        let runner = self;
        let _guards = guards;

        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            let _ = &_guards; // Keep guards alive
            if !is_running_clone.get() {
                let _ = runner.send_event(Event::V1(EventV1::Stop));
                let _ = f.borrow_mut().take();
                return;
            }

            if !runner.process_frame() {
                let _ = runner.send_event(Event::V1(EventV1::Stop));
                let _ = f.borrow_mut().take();
                // 애플릿이 스스로 종료를 요청(ExitApp)한 경우에도 런처로 복귀하도록 이벤트를 발생시킵니다.
                if let Some(window) = web_sys::window()
                    && let Ok(event) = web_sys::CustomEvent::new("request_launcher")
                {
                    let _ = window.dispatch_event(&event);
                }
                return;
            }

            if crate::display::has_blinking_pins() {
                crate::display::redraw_pins(&runner.ctx, false);
            }

            if let Err(e) = Self::request_animation_frame(f.borrow().as_ref().unwrap()) {
                leptos::logging::error!("request_animation_frame failed: {}", e);
            }
        }) as Box<dyn FnMut()>));

        if let Err(e) = Self::request_animation_frame(g.borrow().as_ref().unwrap()) {
            leptos::logging::error!("request_animation_frame failed: {}", e);
        }

        Ok(Box::new(move || is_running.set(false)))
    }

    fn send_event(&self, event: Event) -> Result<Option<EventResult>> {
        let bytes = postcard::to_allocvec(&event).unwrap();

        let buffer_ptr_js = self
            .exports
            .get_event_buffer_ptr
            .call1(&JsValue::NULL, &JsValue::from(bytes.len() as i32))
            .unwrap_or(JsValue::from(0));
        let buffer_ptr = buffer_ptr_js.as_f64().unwrap_or(0.0) as u32;

        let buffer = js_sys::Uint8Array::new(&self.exports.memory.buffer());

        for (i, &byte) in bytes.iter().enumerate() {
            buffer.set_index(buffer_ptr + i as u32, byte);
        }

        // 결과를 담을 충분한 버퍼를 애플릿에 요청합니다.
        let result_buffer_ptr_js = self
            .exports
            .get_event_result_buffer_ptr
            .call1(&JsValue::NULL, &JsValue::from(1024))
            .unwrap_or(JsValue::from(0));
        let result_buffer_ptr = result_buffer_ptr_js.as_f64().unwrap_or(0.0) as u32;

        let result_code_js = self
            .exports
            .on_event
            .call0(&JsValue::NULL)
            .unwrap_or(JsValue::from(0));

        let result_code = result_code_js.as_f64().unwrap_or(0.0) as i32;

        // OnEventResult::HasResult 에 해당하는 1을 반환했다면 결과를 역직렬화합니다.
        if result_code == 1 {
            let mut result_bytes = vec![0u8; 1024];
            let memory_buffer = js_sys::Uint8Array::new(&self.exports.memory.buffer());
            for (i, item) in result_bytes.iter_mut().enumerate().take(1024) {
                *item = memory_buffer.get_index(result_buffer_ptr + i as u32);
            }
            if let Ok(result) = postcard::from_bytes(&result_bytes) {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    fn process_frame(&self) -> bool {
        while let Some(event) = self.event_queue.pop() {
            let res = self.send_event(event);
            if let Ok(Some(EventResult::V1(EventResultV1::Update(UpdateResult::ExitApp)))) = res {
                return false;
            }
        }

        let update_result_opt = self.send_event(Event::V1(EventV1::Update)).unwrap_or(None);

        if let Some(EventResult::V1(EventResultV1::Update(res))) = update_result_opt {
            match res {
                UpdateResult::NeedsRedraw | UpdateResult::Unchanged => {
                    // [렌더링 최적화]
                    // SDK 내부에서 NeedsRedraw를 반환하기 전에 이미 스스로 on_draw()를 호출하여 화면 버퍼 렌더링을 마칩니다.
                    // Web의 Canvas API는 그리기 명령 즉시 브라우저 화면에 반영되므로 Native와 같이 명시적인 갱신(show)이나
                    // 추가적인 Draw 이벤트를 발생시킬 필요가 없습니다 (이벤트 왕복 오버헤드 제거).
                }
                UpdateResult::ExitApp => return false,
            }
        }

        true
    }

    fn map_perkins_dot(code: &str) -> Option<u8> {
        match code {
            "KeyD" => Some(1),
            "KeyW" => Some(2),
            "KeyA" => Some(3),
            "ArrowLeft" => Some(4),
            "ArrowUp" => Some(5),
            "ArrowRight" => Some(6),
            _ => None,
        }
    }

    fn map_key_code(code: &str) -> Option<(sdk::api::keypad::KeyCode, KeypadSide)> {
        match code {
            "KeyW" => Some((sdk::api::keypad::KeyCode::Up, KeypadSide::Left)),
            "KeyS" => Some((sdk::api::keypad::KeyCode::Down, KeypadSide::Left)),
            "KeyA" => Some((sdk::api::keypad::KeyCode::Left, KeypadSide::Left)),
            "KeyD" => Some((sdk::api::keypad::KeyCode::Right, KeypadSide::Left)),
            "ArrowUp" => Some((sdk::api::keypad::KeyCode::Up, KeypadSide::Right)),
            "ArrowDown" => Some((sdk::api::keypad::KeyCode::Down, KeypadSide::Right)),
            "ArrowLeft" => Some((sdk::api::keypad::KeyCode::Left, KeypadSide::Right)),
            "ArrowRight" => Some((sdk::api::keypad::KeyCode::Right, KeypadSide::Right)),
            "Enter" => Some((sdk::api::keypad::KeyCode::Center, KeypadSide::Right)),
            "KeyE" => Some((sdk::api::keypad::KeyCode::Function, KeypadSide::Left)),
            "KeyQ" => Some((sdk::api::keypad::KeyCode::Menu, KeypadSide::Left)),
            "Insert" => Some((sdk::api::keypad::KeyCode::Function, KeypadSide::Right)),
            "Delete" => Some((sdk::api::keypad::KeyCode::Menu, KeypadSide::Right)),
            "Space" => Some((sdk::api::keypad::KeyCode::Center, KeypadSide::Left)),
            _ => None,
        }
    }

    fn request_animation_frame(f: &Closure<dyn FnMut()>) -> Result<i32> {
        web_sys::window()
            .ok_or(Error::NoGlobalWindowExistsError)?
            .request_animation_frame(f.as_ref().unchecked_ref())
            .map_err(|e| {
                Error::JsFunctionError("request_animation_frame".to_string(), format!("{:?}", e))
            })
    }
}

struct EventListenerGuard {
    target: web_sys::EventTarget,
    event_type: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

impl EventListenerGuard {
    fn new(
        target: web_sys::EventTarget,
        event_type: &'static str,
        callback: Box<dyn FnMut(web_sys::Event)>,
    ) -> Self {
        let closure = Closure::wrap(callback);
        target
            .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
            .unwrap();
        Self {
            target,
            event_type,
            closure,
        }
    }
}

impl Drop for EventListenerGuard {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(
            self.event_type,
            self.closure.as_ref().unchecked_ref(),
        );
    }
}
