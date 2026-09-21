use crate::host::HostState;
use anyhow::Result;
use log::error;
use runtime_common::audio::protocol::SequenceSegment;
use runtime_common::audio::protocol::TTS_SERVICE_MAX_AUDIO_SIZE;
use sdk::api::audio::{FfiSegment, FfiSequencePayload};
use sdk::applet::SpeechOption;
use wasmtime::{Caller, Linker};

fn handle_play(mut caller: Caller<'_, HostState>, ptr: i32, len: i32) {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => return,
    };
    let (ptr, len) = (ptr as usize, len as usize);

    if len > TTS_SERVICE_MAX_AUDIO_SIZE {
        error!(
            "TTS play 요청 거부: 오디오 데이터 크기 초과 ({} bytes)",
            len
        );
        return;
    }

    // 1. 가변 대여(mutable borrow)가 필요한 볼륨 값을 먼저 획득합니다.
    let settings = crate::host::preferences::AppSettings::load();
    let volume = settings.volume / 100.0;


    // 2. 그 이후 불변 대여(immutable borrow)를 통해 WASM 메모리 데이터를 가져옵니다.
    let data = mem.data(&caller);

    if let Some(bytes) = data.get(ptr..ptr + len) {
        // WASM 메모리에서 데이터를 가져와서 Vec<u8>로 복사
        // (rodio Sink가 백그라운드 스레드에서 재생하므로 'static 수명이 필요)
        crate::audio::play(bytes.to_vec(), volume);
    }
}

fn handle_play_sequence(mut caller: Caller<'_, HostState>, ptr: u32, len: u32) {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => return,
    };
    let (ptr, len) = (ptr as usize, len as usize);

    if len > TTS_SERVICE_MAX_AUDIO_SIZE {
        error!("TTS play_sequence 요청 거부: 오디오 데이터 크기 초과");
        return;
    }

    let data = mem.data(&caller);
    let bytes = match data.get(ptr..ptr + len) {
        Some(b) => b,
        None => return,
    };

    // 1. Postcard를 사용해 WASM 구조체 포인터들을 FfiSegment로 파싱
    let payload: FfiSequencePayload = match postcard::from_bytes(bytes) {
        Ok(seg) => seg,
        Err(e) => {
            error!("play_sequence 페이로드 디코딩 실패: {}", e);
            return;
        }
    };

    // 2. WASM 메모리에서 데이터를 추출하여 HTTP 요청용 SequenceSegment로 변환
    let mut segments = Vec::new();
    let options = match payload.speech_type {
        1 => SpeechOption::forced(),
        2 => SpeechOption::important(),
        3 => SpeechOption::forced_important(),
        _ => SpeechOption::new(),
    };

    for seg in payload.segments {
        match seg {
            FfiSegment::Text(t) => segments.push(SequenceSegment::Text(t.to_string())),
            FfiSegment::Sound {
                ptr: s_ptr,
                len: s_len,
                volume,
            } => {
                let (s_ptr, s_len) = (s_ptr as usize, s_len as usize);
                // Sound의 바이트 포인터 주소를 사용해 WASM 메모리에서 원본 바이너리를 긁어옵니다.
                if let Some(sound_bytes) = data.get(s_ptr..s_ptr + s_len) {
                    segments.push(SequenceSegment::Sound(sound_bytes.to_vec(), volume));
                }
            }
            FfiSegment::Silence(ms) => segments.push(SequenceSegment::Silence(ms)),
        }
    }

    // 3. 하나의 HTTP 요청으로 시퀀스 전체를 오디오 서비스에 전달
    tokio::spawn(async move {
        let _ = crate::audio::play_sequence(segments, options).await;
    });
}

/// `wasmtime::Linker`에 TTS 관련 호스트 함수들을 추가합니다.
pub fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap("audio", "play", handle_play)?;
    linker.func_wrap("audio", "play_sequence", handle_play_sequence)?;

    Ok(())
}
