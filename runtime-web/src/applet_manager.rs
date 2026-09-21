use crate::Error;

use js_sys::WebAssembly;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

pub struct AppletManagerClosures {
    pub list_applets: Closure<dyn FnMut(i32, i32) -> i32>,
    pub start_applet: Closure<dyn FnMut(i32, i32) -> i32>,
    pub get_current_applet: Closure<dyn FnMut(i32, i32) -> i32>,
}

pub fn setup_imports<F>(
    imports: &js_sys::Object,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
    mut on_start_applet: F,
    applets: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    current_app_name: String,
) -> Result<AppletManagerClosures, Error>
where
    F: FnMut(String) + 'static,
{
    let applet_manager = js_sys::Object::new();

    let memory_clone = memory.clone();
    let applets_clone = applets.clone();
    let list_applets = Closure::wrap(Box::new(move |ptr: i32, len: i32| -> i32 {
        let ptr = ptr as u32;
        let len = len as usize;

        if let Some(mem) = memory_clone.borrow().as_ref() {
            let mut applet_infos = Vec::new();

            for (name, wasm_bytes) in applets_clone.lock().unwrap().iter() {
                let applet_info = runtime_common::applet::parse_applet_info(name, wasm_bytes);
                if !applet_info.hidden {
                    applet_infos.push(applet_info);
                }
            }

            applet_infos.sort_by_key(|info| info.priority);

            let serialized = match postcard::to_allocvec(&applet_infos) {
                Ok(data) => data,
                Err(_) => return -1,
            };

            if serialized.len() > len {
                return serialized.len() as i32; // Not enough buffer, return size needed (to trigger err)
            }

            let mem_buf = js_sys::Uint8Array::new(&mem.buffer());
            let serialized_arr = js_sys::Uint8Array::from(serialized.as_slice());
            mem_buf.set(&serialized_arr, ptr);

            serialized.len() as i32
        } else {
            -1
        }
    }) as Box<dyn FnMut(i32, i32) -> i32>);

    let memory_clone2 = memory.clone();
    let start_applet = Closure::wrap(Box::new(move |ptr: i32, len: i32| -> i32 {
        let ptr = ptr as u32;
        let len = len as u32;
        if let Some(mem) = memory_clone2.borrow().as_ref() {
            let buffer = js_sys::Uint8Array::new(&mem.buffer());
            let bytes = buffer.slice(ptr, ptr + len).to_vec();
            if let Ok(name) = String::from_utf8(bytes) {
                leptos::logging::log!("애플릿 실행 요청 수신: {}", name);
                on_start_applet(name);
                return 0;
            }
        }
        -1
    }) as Box<dyn FnMut(i32, i32) -> i32>);

    let memory_clone3 = memory.clone();
    let applets_clone2 = applets.clone();
    let current_app_name_clone = current_app_name.clone();
    let get_current_applet = Closure::wrap(Box::new(move |ptr: i32, len: i32| -> i32 {
        let ptr = ptr as u32;
        let len = len as usize;

        if let Some(mem) = memory_clone3.borrow().as_ref() {
            let applet_info = {
                let applets_guard = applets_clone2.lock().unwrap();
                if let Some(wasm_bytes) = applets_guard.get(&current_app_name_clone) {
                    runtime_common::applet::parse_applet_info(&current_app_name_clone, wasm_bytes)
                } else {
                    sdk::applet::AppletInfo::default()
                }
            };

            let serialized = match postcard::to_allocvec(&applet_info) {
                Ok(data) => data,
                Err(_) => return -1,
            };

            if serialized.len() > len {
                return serialized.len() as i32; // Not enough buffer
            }

            let mem_buf = js_sys::Uint8Array::new(&mem.buffer());
            let serialized_arr = js_sys::Uint8Array::from(serialized.as_slice());
            mem_buf.set(&serialized_arr, ptr);

            serialized.len() as i32
        } else {
            -1
        }
    }) as Box<dyn FnMut(i32, i32) -> i32>);

    js_sys::Reflect::set(
        &applet_manager,
        &"list_applets".into(),
        list_applets.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("list_applets: {:?}", e)))?;

    js_sys::Reflect::set(
        &applet_manager,
        &"start_applet".into(),
        start_applet.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("start_applet: {:?}", e)))?;

    js_sys::Reflect::set(
        &applet_manager,
        &"get_current_applet".into(),
        get_current_applet.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("get_current_applet: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"applet_manager".into(), &applet_manager)
        .map_err(|e| sdk::error::Error::ImportError(format!("applet_manager: {:?}", e)))?;

    Ok(AppletManagerClosures {
        list_applets,
        start_applet,
        get_current_applet,
    })
}
