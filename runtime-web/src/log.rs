use crate::Error;
use js_sys::WebAssembly;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

pub struct LogClosures {
    pub message: Closure<dyn FnMut(i32, i32, i32)>,
}

pub fn setup_imports(
    imports: &js_sys::Object,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Result<LogClosures, Error> {
    let log = js_sys::Object::new();

    let memory_clone = memory.clone();
    let message = Closure::wrap(Box::new(move |level: i32, ptr: i32, len: i32| {
        let ptr = ptr as u32;
        let len = len as u32;
        if let Some(mem) = memory_clone.borrow().as_ref() {
            let max_len = runtime_common::log::MAX_LOG_MESSAGE_SIZE as u32;
            if len > max_len {
                leptos::logging::error!(
                    "{}",
                    Error::TooLargeDataRequestError {
                        from: "message".to_string(),
                        len: len as usize,
                        expected_max: max_len as usize,
                    }
                );
                return;
            }
            let buffer = js_sys::Uint8Array::new(&mem.buffer());
            let bytes = buffer.slice(ptr, ptr + len).to_vec();
            if let Ok(msg) = String::from_utf8(bytes) {
                leptos::logging::log!("{}: Applet: {}", level, msg);
            }
        }
    }) as Box<dyn FnMut(i32, i32, i32)>);

    js_sys::Reflect::set(&log, &"message".into(), message.as_ref().unchecked_ref())
        .map_err(|e| sdk::Error::ImportError(format!("message: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"log".into(), &log)
        .map_err(|e| sdk::Error::ImportError(format!("log: {:?}", e)))?;

    Ok(LogClosures { message })
}
