use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::Error;

pub struct TimeClosures {
    pub get_time_seconds: Closure<dyn FnMut() -> u64>,
    pub get_timezone_offset_seconds: Closure<dyn FnMut() -> i32>,
    pub get_monotonic_time_nanos: Closure<dyn FnMut() -> u64>,
}

pub fn setup_imports(imports: &js_sys::Object) -> Result<TimeClosures, Error> {
    let time = js_sys::Object::new();

    let get_time_seconds = Closure::wrap(
        Box::new(move || (js_sys::Date::now() / 1000.0) as u64) as Box<dyn FnMut() -> u64>
    );
    js_sys::Reflect::set(
        &time,
        &"get_time_seconds".into(),
        get_time_seconds.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::Error::ImportError(format!("get_time_seconds: {:?}", e)))?;

    let get_timezone_offset_seconds =
        Closure::wrap(
            Box::new(move || (-js_sys::Date::new_0().get_timezone_offset() * 60.0) as i32)
                as Box<dyn FnMut() -> i32>,
        );
    js_sys::Reflect::set(
        &time,
        &"get_timezone_offset_seconds".into(),
        get_timezone_offset_seconds.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::Error::ImportError(format!("get_timezone_offset_seconds: {:?}", e)))?;

    let window = web_sys::window().ok_or(Error::NoGlobalWindowExistsError)?;
    let performance = window.performance().ok_or_else(|| {
        Error::JsFunctionError("performance".to_string(), "not available".to_string())
    })?;

    let get_monotonic_time_nanos =
        Closure::wrap(
            Box::new(move || (performance.now() * 1_000_000.0) as u64) as Box<dyn FnMut() -> u64>
        );
    js_sys::Reflect::set(
        &time,
        &"get_monotonic_time_nanos".into(),
        get_monotonic_time_nanos.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::Error::ImportError(format!("get_monotonic_time_nanos: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"time".into(), &time)
        .map_err(|e| sdk::Error::ImportError(format!("time: {:?}", e)))?;

    Ok(TimeClosures {
        get_time_seconds,
        get_timezone_offset_seconds,
        get_monotonic_time_nanos,
    })
}
