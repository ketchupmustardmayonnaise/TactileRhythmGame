use js_sys::WebAssembly;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::Error;

pub struct AudioClosures {
    pub play: Closure<dyn FnMut(i32, i32)>,
    pub play_sequence: Closure<dyn FnMut(i32, i32)>,
}

pub fn setup_imports(
    imports: &js_sys::Object,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Result<AudioClosures, Error> {
    let audio_obj = js_sys::Object::new();

    let memory_clone2 = memory.clone();
    let play = Closure::wrap(Box::new(move |ptr: i32, len: i32| {
        let ptr = ptr as usize;
        let len = len as usize;

        if len > runtime_common::audio::protocol::TTS_SERVICE_MAX_AUDIO_SIZE {
            leptos::logging::error!(
                "TTS play 요청 거부: 오디오 데이터 크기 초과 ({} bytes)",
                len
            );
            return;
        }

        if let Some(mem) = memory_clone2.borrow().as_ref() {
            let buffer = js_sys::Uint8Array::new(&mem.buffer());
            let bytes = buffer.slice(ptr as u32, (ptr + len) as u32).to_vec();
            let volume = if let Ok(storage) = crate::storage::Storage::new() {
                let s = storage.get_app_settings().unwrap_or_default();
                if s.volume == 0.0 {
                    0.5
                } else if s.volume <= 9.0 {
                    (s.volume * 10.0) / 100.0
                } else {
                    s.volume / 100.0
                }
            } else {
                0.5
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = crate::audio::play_static_audio(bytes, volume).await {
                    leptos::logging::error!("오디오 재생 에러: {:?}", e);
                }
            });
        }
    }) as Box<dyn FnMut(i32, i32)>);


    let memory_clone3 = memory.clone();
    let play_sequence = Closure::wrap(Box::new(move |ptr: i32, len: i32| {
        crate::audio::handle_play_sequence(&memory_clone3, ptr, len);
    }) as Box<dyn FnMut(i32, i32)>);

    js_sys::Reflect::set(&audio_obj, &"play".into(), play.as_ref().unchecked_ref())
        .map_err(|e| sdk::Error::ImportError(format!("play: {:?}", e)))?;

    js_sys::Reflect::set(
        &audio_obj,
        &"play_sequence".into(),
        play_sequence.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::Error::ImportError(format!("play_sequence: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"audio".into(), &audio_obj)
        .map_err(|e| sdk::Error::ImportError(format!("audio: {:?}", e)))?;

    // 하위 호환성: 기존에 tts 모듈로 빌드된 WASM 애플릿 지원을 위해 동일 객체를 별칭으로 등록합니다.
    js_sys::Reflect::set(imports, &"tts".into(), &audio_obj)
        .map_err(|e| sdk::Error::ImportError(format!("tts: {:?}", e)))?;

    Ok(AudioClosures {
        play,
        play_sequence,
    })
}
