use sdk::error::Error;
use std::cell::Cell;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

thread_local! {
    pub static PERKINS_MODE: Cell<bool> = const { Cell::new(false) };
}

pub struct KeypadClosures {
    pub _set_perkins_mode: Closure<dyn FnMut(i32)>,
}

pub fn setup_imports(imports: &js_sys::Object) -> Result<KeypadClosures, Error> {
    let keypad = js_sys::Object::new();

    let set_perkins_mode = Closure::wrap(Box::new(move |mode: i32| {
        PERKINS_MODE.with(|p| p.set(mode == 1));
        leptos::logging::log!("keypad_set_perkins_mode: {}", mode == 1);
    }) as Box<dyn FnMut(i32)>);

    js_sys::Reflect::set(
        &keypad,
        &"set_perkins_mode".into(),
        set_perkins_mode.as_ref().unchecked_ref(),
    )
    .map_err(|e| Error::ImportError(format!("set_perkins_mode: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"keypad".into(), &keypad)
        .map_err(|e| Error::ImportError(format!("keypad: {:?}", e)))?;

    Ok(KeypadClosures {
        _set_perkins_mode: set_perkins_mode,
    })
}
