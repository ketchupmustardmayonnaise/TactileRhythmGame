use crate::Error;
use base64::Engine;
use js_sys::WebAssembly;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

pub struct PreferencesClosure {
    pub _get_string: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
    pub _set_string: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
    pub _get_int: Closure<dyn FnMut(i32, i32, i32) -> i32>,
    pub _set_int: Closure<dyn FnMut(i32, i32, i32) -> i32>,
    pub _get_float: Closure<dyn FnMut(i32, i32, i32) -> i32>,
    pub _set_float: Closure<dyn FnMut(i32, i32, f32) -> i32>,
    pub _get_bytes: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
    pub _set_bytes: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
    pub _settings_get: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
    pub _settings_set: Closure<dyn FnMut(i32, i32, i32, i32) -> i32>,
}

fn get_local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// 타입별 로컬 스토리지 키 접두사(prefix)를 정의하기 위한 트레이트입니다.
pub trait PreferenceAccess {
    const PREFIX: &'static str;
}

/// 문자열 타입 설정을 위한 마커 구조체
pub struct StringPref;
impl PreferenceAccess for StringPref {
    const PREFIX: &'static str = "pref:str:";
}

/// 정수형 타입 설정을 위한 마커 구조체
pub struct IntPref;
impl PreferenceAccess for IntPref {
    const PREFIX: &'static str = "pref:int:";
}

/// 실수형 타입 설정을 위한 마커 구조체
pub struct FloatPref;
impl PreferenceAccess for FloatPref {
    const PREFIX: &'static str = "pref:float:";
}

/// 바이트 배열 타입 설정을 위한 마커 구조체
pub struct BytesPref;
impl PreferenceAccess for BytesPref {
    const PREFIX: &'static str = "pref:bytes:";
}

/// WASM 메모리와 직접 주고받을 수 있는 원시 타입(Primitive Type)을 위한 트레이트입니다.
/// 문자열 파싱 및 바이트 배열 변환 로직을 추상화하여 중복 코드를 제거합니다.
pub trait PrimitiveType: Copy {
    /// 문자열에서 값을 파싱합니다.
    fn parse(s: &str) -> Option<Self>;
    /// 값을 리틀 엔디안 바이트 배열로 변환합니다. (WASM 메모리 기록용)
    fn to_le_bytes(self) -> Vec<u8>;
    /// 값을 문자열로 변환합니다. (로컬 스토리지 저장용)
    fn to_string_val(self) -> String;
}

impl PrimitiveType for i32 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
    fn to_le_bytes(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
    fn to_string_val(self) -> String {
        self.to_string()
    }
}

impl PrimitiveType for f32 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
    fn to_le_bytes(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
    fn to_string_val(self) -> String {
        self.to_string()
    }
}

/// WASM 메모리에 접근하여 주어진 포인터와 길이로부터 키 문자열(key)을 읽어오고,
/// 클로저 `f`에 메모리 버퍼 뷰와 읽어온 키를 전달하는 공통 헬퍼 함수입니다.
fn with_memory_and_key<R>(
    memory: &Rc<RefCell<Option<WebAssembly::Memory>>>,
    key_ptr: i32,
    key_len: i32,
    f: impl FnOnce(js_sys::Uint8Array, String) -> R,
) -> Option<R> {
    let mem_guard = memory.borrow();
    let mem = mem_guard.as_ref()?;
    let buffer = js_sys::Uint8Array::new(&mem.buffer());

    let key_start = key_ptr as u32;
    let key_end = key_start.saturating_add(key_len as u32);
    if key_end > buffer.length() {
        return None;
    }

    let key_bytes = buffer.slice(key_start, key_end).to_vec();
    let key = String::from_utf8(key_bytes).ok()?;
    Some(f(buffer, key))
}

/// 지정된 `PreferenceAccess` 타입의 접두사를 사용하여 로컬 스토리지에서 값을 읽어옵니다.
fn read_from_storage<T: PreferenceAccess>(key: &str) -> Option<String> {
    let ls = get_local_storage()?;
    let storage_key = format!("{}{}", T::PREFIX, key);
    ls.get_item(&storage_key).ok().flatten()
}

/// 지정된 `PreferenceAccess` 타입의 접두사를 사용하여 로컬 스토리지에 값을 저장합니다.
fn write_to_storage<T: PreferenceAccess>(key: &str, value: &str) -> Result<(), ()> {
    let ls = get_local_storage().ok_or(())?;
    let storage_key = format!("{}{}", T::PREFIX, key);
    ls.set_item(&storage_key, value).map_err(|_| ())
}

/// 로컬 스토리지에서 현재 볼륨 설정을 읽어와 전역 GainNode에 적용합니다.
pub fn apply_current_volume() {
    let vol_val = if let Ok(storage) = crate::storage::Storage::new() {
        if let Ok(settings) = storage.get_app_settings() {
            settings.volume
        } else {
            50.0
        }
    } else {
        50.0
    };
    let vol_ratio = if vol_val <= 9.0 { (vol_val * 10.0) / 100.0 } else { vol_val / 100.0 };
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::audio::set_volume(vol_ratio).await {
            leptos::logging::error!("서버 볼륨 설정 실패: {}", e);
        }
    });
}


/// 동적 길이 데이터(String, Bytes)를 스토리지에서 읽어와 WASM 버퍼에 쓰는 클로저를 생성합니다.
/// `decode` 클로저를 통해 스토리지의 문자열을 바이트 배열로 변환합니다.
fn build_get_buffer<T: PreferenceAccess, F>(
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
    decode: F,
) -> Closure<dyn FnMut(i32, i32, i32, i32) -> i32>
where
    F: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    Closure::wrap(Box::new(
        move |key_ptr: i32, key_len: i32, buf_ptr: i32, buf_len: i32| -> i32 {
            with_memory_and_key(&memory, key_ptr, key_len, |buffer, key| {
                if let Some(val_str) = read_from_storage::<T>(&key)
                    && let Some(val_bytes) = decode(&val_str)
                {
                    let len = val_bytes.len() as u32;
                    if len <= buf_len as u32 {
                        let buf_start = buf_ptr as u32;
                        let buf_end = buf_start.saturating_add(len);
                        if buf_end <= buffer.length() {
                            buffer.set(&js_sys::Uint8Array::from(&val_bytes[..]), buf_start);
                            return len as i32;
                        }
                    }
                    return -2; // BufferTooSmall
                }
                -1
            })
            .unwrap_or(-3)
        },
    ) as Box<dyn FnMut(i32, i32, i32, i32) -> i32>)
}

/// WASM 버퍼에서 동적 길이 데이터(String, Bytes)를 읽어와 스토리지에 저장하는 클로저를 생성합니다.
/// `encode` 클로저를 통해 바이트 배열을 스토리지에 저장할 문자열로 변환합니다.
fn build_set_buffer<T: PreferenceAccess, F>(
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
    encode: F,
) -> Closure<dyn FnMut(i32, i32, i32, i32) -> i32>
where
    F: Fn(&[u8]) -> Option<String> + 'static,
{
    Closure::wrap(Box::new(
        move |key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32| -> i32 {
            with_memory_and_key(&memory, key_ptr, key_len, |buffer, key| {
                let val_start = val_ptr as u32;
                let val_end = val_start.saturating_add(val_len as u32);
                if val_end <= buffer.length() {
                    let val_bytes = buffer.slice(val_start, val_end).to_vec();
                    if let Some(val_str) = encode(&val_bytes)
                        && write_to_storage::<T>(&key, &val_str).is_ok()
                    {
                        return 0;
                    }
                }
                -1
            })
            .unwrap_or(-1)
        },
    ) as Box<dyn FnMut(i32, i32, i32, i32) -> i32>)
}

/// 원시 타입 데이터(int, float)를 스토리지에서 읽어와 WASM 메모리에 쓰는 클로저를 생성합니다.
/// `PrimitiveType` 트레이트를 활용하여 파싱 및 바이트 변환을 수행합니다.
fn build_get_primitive<T: PreferenceAccess, V: PrimitiveType + 'static>(
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Closure<dyn FnMut(i32, i32, i32) -> i32> {
    Closure::wrap(
        Box::new(move |key_ptr: i32, key_len: i32, out_ptr: i32| -> i32 {
            with_memory_and_key(&memory, key_ptr, key_len, |buffer, key| {
                if let Some(val_str) = read_from_storage::<T>(&key)
                    && let Some(val) = V::parse(&val_str)
                {
                    let bytes = val.to_le_bytes();
                    let out_start = out_ptr as u32;
                    let out_end = out_start.saturating_add(bytes.len() as u32);
                    if out_end <= buffer.length() {
                        buffer.set(&js_sys::Uint8Array::from(&bytes[..]), out_start);
                        return 0;
                    }
                }
                -1
            })
            .unwrap_or(-3)
        }) as Box<dyn FnMut(i32, i32, i32) -> i32>,
    )
}

/// 원시 타입 데이터(int, float)를 파라미터로 직접 전달받아 스토리지에 저장하는 클로저를 생성합니다.
fn build_set_primitive<T: PreferenceAccess, V>(
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Closure<dyn FnMut(i32, i32, V) -> i32>
where
    V: PrimitiveType + wasm_bindgen::convert::FromWasmAbi + 'static,
{
    Closure::wrap(Box::new(move |key_ptr: i32, key_len: i32, val: V| -> i32 {
        with_memory_and_key(&memory, key_ptr, key_len, |_, key| {
            if write_to_storage::<T>(&key, &val.to_string_val()).is_ok() {
                return 0;
            }
            -1
        })
        .unwrap_or(-1)
    }) as Box<dyn FnMut(i32, i32, V) -> i32>)
}

/// WASM 모듈에 주입할 preferences 관련 JS 임포트 함수들을 설정하고 클로저 묶음을 반환합니다.
pub fn setup_imports(
    imports: &js_sys::Object,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Result<PreferencesClosure, Error> {
    let prefs = js_sys::Object::new();

    let get_string =
        build_get_buffer::<StringPref, _>(memory.clone(), |s| Some(s.as_bytes().to_vec()));
    let set_string =
        build_set_buffer::<StringPref, _>(memory.clone(), |b| String::from_utf8(b.to_vec()).ok());

    let get_int = build_get_primitive::<IntPref, i32>(memory.clone());
    let set_int = build_set_primitive::<IntPref, i32>(memory.clone());

    let get_float = build_get_primitive::<FloatPref, f32>(memory.clone());
    let set_float = build_set_primitive::<FloatPref, f32>(memory.clone());

    let get_bytes = build_get_buffer::<BytesPref, _>(memory.clone(), |s| {
        base64::engine::general_purpose::STANDARD.decode(s).ok()
    });
    let set_bytes = build_set_buffer::<BytesPref, _>(memory.clone(), |b| {
        Some(base64::engine::general_purpose::STANDARD.encode(b))
    });

    // Settings API 클로저 생성 (String 변환 로직 재사용)
    let memory_clone_for_settings_get = memory.clone();
    let settings_get = Closure::wrap(Box::new(
        move |key_ptr: i32, key_len: i32, buf_ptr: i32, buf_len: i32| -> i32 {
            with_memory_and_key(
                &memory_clone_for_settings_get,
                key_ptr,
                key_len,
                |buffer, key| {
                    let val_str: Option<String> = if key == "app_settings_options" {
                        let (lang_val, voice_val, vol_val, speed_val) =
                            if let Ok(storage) = crate::storage::Storage::new() {
                                let settings = storage.get_app_settings().unwrap_or_default();
                                (
                                    (settings.language as i32).to_string(),
                                    (settings.tts.voice as i32).to_string(),
                                    (settings.volume as i32).to_string(),
                                    (settings.tts.speed as i32).to_string(),
                                )
                            } else {
                                (
                                    "0".to_string(),
                                    "0".to_string(),
                                    "50".to_string(),

                                    "1".to_string(),
                                )
                            };
                        // 추후 TTS서버에서 직접 받아사용
                        Some(runtime_common::settings::generate_app_settings_json(
                            &lang_val, &voice_val, &vol_val, &speed_val,
                        ))
                    } else {
                        if let Ok(storage) = crate::storage::Storage::new() {
                            let settings = storage.get_app_settings().unwrap_or_default();
                            match key.as_str() {
                                "language" => Some((settings.language as i32).to_string()),
                                "tts_voice" => Some((settings.tts.voice as i32).to_string()),
                                "audio_volume" => Some((settings.volume as i32).to_string()),
                                "speech_rate" => Some((settings.tts.speed as i32).to_string()),
                                _ => None::<String>,
                            }
                        } else {
                            None::<String>
                        }
                    };

                    if let Some(val_str) = val_str {
                        let val_bytes = val_str.as_bytes();
                        let len = val_bytes.len() as u32;
                        if len <= buf_len as u32 {
                            let buf_start = buf_ptr as u32;
                            let buf_end = buf_start.saturating_add(len);
                            if buf_end <= buffer.length() {
                                buffer.set(&js_sys::Uint8Array::from(val_bytes), buf_start);
                                return len as i32;
                            }
                        }
                        return -2; // BufferTooSmall
                    }
                    -1
                },
            )
            .unwrap_or(-3)
        },
    ) as Box<dyn FnMut(i32, i32, i32, i32) -> i32>);

    let memory_clone_for_settings_set = memory.clone();
    let settings_set = Closure::wrap(Box::new(
        move |key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32| -> i32 {
            with_memory_and_key(
                &memory_clone_for_settings_set,
                key_ptr,
                key_len,
                |buffer, key| {
                    let val_start = val_ptr as u32;
                    let val_end = val_start.saturating_add(val_len as u32);
                    if val_end > buffer.length() {
                        return -1;
                    }
                    let val_bytes = buffer.slice(val_start, val_end).to_vec();
                    if let Ok(val_str) = String::from_utf8(val_bytes) {
                        if let Ok(storage) = crate::storage::Storage::new()
                            && let Ok(mut app_settings) = storage.get_app_settings()
                        {
                            let mut updated = false;
                            match key.as_str() {
                                "language" => {
                                    if let Ok(idx) = val_str.parse::<i32>() {
                                        app_settings.language = sdk::types::Language::from(idx);
                                        updated = true;
                                    }
                                }
                                "tts_voice" => {
                                    if let Ok(idx) = val_str.parse::<i32>() {
                                        app_settings.tts.voice = sdk::types::Voice::from(idx);
                                        updated = true;
                                    }
                                }
                                "audio_volume" => {
                                    if let Ok(vol) = val_str.parse::<f32>() {
                                        app_settings.volume = vol;
                                        updated = true;
                                    }
                                }
                                "speech_rate" => {
                                    if let Ok(speed) = val_str.parse::<i32>() {
                                        app_settings.tts.speed = sdk::types::Speed::from(speed);
                                        updated = true;
                                    }
                                }
                                _ => {} // 스키마에 정의되지 않은 알 수 없는 키는 무시합니다.
                            }
                            if updated {
                                let _ = storage.save_app_settings(&app_settings);
                            }
                        }

                        // 2. 런타임 UI가 변경을 감지할 수 있도록 단순 이벤트 발생
                        if let Some(window) = web_sys::window()
                            && let Ok(event) = web_sys::CustomEvent::new("setting_changed")
                        {
                            let _ = window.dispatch_event(&event);
                        }
                        return 0; // 성공
                    }
                    -1 // 에러
                },
            )
            .unwrap_or(-1)
        },
    ) as Box<dyn FnMut(i32, i32, i32, i32) -> i32>);

    // Reflect bindings
    let exports = [
        ("get_string", get_string.as_ref()),
        ("set_string", set_string.as_ref()),
        ("get_int", get_int.as_ref()),
        ("set_int", set_int.as_ref()),
        ("get_float", get_float.as_ref()),
        ("set_float", set_float.as_ref()),
        ("get_bytes", get_bytes.as_ref()),
        ("set_bytes", set_bytes.as_ref()),
    ];

    for (name, func) in exports {
        js_sys::Reflect::set(&prefs, &name.into(), func.unchecked_ref()).map_err(|e| {
            sdk::error::Error::ImportError(format!("preferences_{}: {:?}", name, e))
        })?;
    }

    js_sys::Reflect::set(imports, &"preferences".into(), &prefs)
        .map_err(|e| sdk::error::Error::ImportError(format!("preferences module: {:?}", e)))?;

    // Settings 객체를 별도로 생성하여 imports에 바인딩
    let settings_obj = js_sys::Object::new();
    js_sys::Reflect::set(
        &settings_obj,
        &"get".into(),
        settings_get.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("settings_get: {:?}", e)))?;
    js_sys::Reflect::set(
        &settings_obj,
        &"set".into(),
        settings_set.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("settings_set: {:?}", e)))?;
    js_sys::Reflect::set(imports, &"settings".into(), &settings_obj)
        .map_err(|e| sdk::error::Error::ImportError(format!("settings module: {:?}", e)))?;

    Ok(PreferencesClosure {
        _get_string: get_string,
        _set_string: set_string,
        _get_int: get_int,
        _set_int: set_int,
        _get_float: get_float,
        _set_float: set_float,
        _get_bytes: get_bytes,
        _set_bytes: set_bytes,
        _settings_get: settings_get,
        _settings_set: settings_set,
    })
}
