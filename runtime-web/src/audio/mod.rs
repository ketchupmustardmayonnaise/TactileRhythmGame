pub mod wasm_bindings;

use runtime_common::audio::client::AudioServiceClient;
use runtime_common::audio::protocol::{PlaySequenceRequest, SequenceSegment};
use sdk::api::audio::{FfiSegment, FfiSequencePayload};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;

/// 서버의 audio-service /play 엔드포인트로 오디오 재생을 요청합니다.
pub async fn play_static_audio(audio_data: Vec<u8>, volume: f32) -> Result<(), String> {
    let client = AudioServiceClient::new();
    client
        .request_play_sound(audio_data, volume)
        .await
        .map_err(|e| e.to_string())
}

/// WASM에서 전달된 RichSpeech 시퀀스를 파싱하고 audio-service를 통해 순차적으로 서버 측 재생을 요청합니다.
pub fn handle_play_sequence(
    memory: &Rc<RefCell<Option<js_sys::WebAssembly::Memory>>>,
    ptr: i32,
    len: i32,
) {
    let mem_guard = memory.borrow();
    let mem = match mem_guard.as_ref() {
        Some(m) => m,
        None => return,
    };

    let buffer = js_sys::Uint8Array::new(&mem.buffer());
    let start = ptr as u32;
    let end = start.saturating_add(len as u32);

    if end > buffer.length() {
        leptos::logging::error!("play_sequence: 오디오 데이터 크기 초과");
        return;
    }

    let bytes = buffer.slice(start, end).to_vec();

    let payload: FfiSequencePayload = match postcard::from_bytes(&bytes) {
        Ok(seg) => seg,
        Err(e) => {
            leptos::logging::error!("play_sequence 디코딩 실패: {:?}", e);
            return;
        }
    };

    let mut segments = Vec::new();
    let options = match payload.speech_type {
        1 => sdk::applet::SpeechOption::forced(),
        2 => sdk::applet::SpeechOption::important(),
        3 => sdk::applet::SpeechOption::forced_important(),
        _ => sdk::applet::SpeechOption::new(),
    };

    let mut full_text = String::new();
    for seg in payload.segments {
        match seg {
            FfiSegment::Text(t) => {
                segments.push(SequenceSegment::Text(t.to_string()));
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(t);
            }
            FfiSegment::Sound {
                ptr: s_ptr,
                len: s_len,
                volume,
            } => {
                let s_start = s_ptr;
                let s_end = s_start.saturating_add(s_len);
                if s_end <= buffer.length() {
                    segments.push(SequenceSegment::Sound(
                        buffer.slice(s_start, s_end).to_vec(),
                        volume,
                    ));
                }
            }
            FfiSegment::Silence(ms) => segments.push(SequenceSegment::Silence(ms)),
        }
    }

    if !full_text.is_empty()
        && let Some(window) = web_sys::window()
    {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&wasm_bindgen::JsValue::from_str(&full_text));
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("tts_played", &init) {
            let _ = window.dispatch_event(&event);
        }
    }

    // 백그라운드에서 단일 HTTP 요청을 스폰하여 서버 측 병합 재생 실행
    spawn_local(async move {
        let _ = play_sequence(segments, options).await;
    });
}

/// 오디오 시퀀스를 서버로 전송하여 단일 트랙으로 이어서 재생하도록 요청합니다.
pub async fn play_sequence(
    segments: Vec<SequenceSegment>,
    options: sdk::applet::SpeechOption,
) -> Result<(), String> {
    let (voice_type, speed, language) = if let Ok(storage) = crate::storage::Storage::new() {
        let settings = storage.get_app_settings().unwrap_or_default();
        (
            format!("{:?}", settings.tts.voice),
            settings.tts.speed.as_f32(),
            settings.language,
        )
    } else {
        ("F1".to_string(), 1.0, sdk::Language::default())
    };

    let is_important = options.important;
    let is_forced = options.forced;

    let request = PlaySequenceRequest::builder()
        .segments(segments)
        .voice_type(voice_type)
        .speed(speed)
        .language(language)
        .is_important(is_important)
        .is_forced(is_forced)
        .build();

    let client = AudioServiceClient::new();
    client.request_audio_segments(&request).await
}

/// 오디오 볼륨을 서버의 `audio-service`로 요청하여 설정합니다 (0.0 ~ 1.0).
pub async fn set_volume(volume: f32) -> Result<(), String> {
    let client = AudioServiceClient::new();
    client
        .request_set_volume(volume)
        .await
        .map_err(|e| e.to_string())
}
