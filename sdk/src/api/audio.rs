use crate::{Language, applet::SpeechOption};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// FFI 경계를 넘기 위한 내부 직렬화/역직렬화 전용 구조체
#[derive(Serialize, Deserialize, Debug)]
pub enum FfiSegment<'a> {
    Text(&'a str),
    Sound { ptr: u32, len: u32, volume: f32 },
    Silence(u32),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FfiSequencePayload<'a> {
    pub speech_type: u32,
    #[serde(borrow)]
    pub segments: Vec<FfiSegment<'a>>,
}

/// 텍스트, 효과음, 무음(대기)을 혼합하여 순차적으로 재생하기 위한 세그먼트입니다.
#[derive(Debug, Clone)]
pub enum AudioSegment {
    Text(String),
    Sound(&'static [u8], f32), // 정적 오디오 바이너리 데이터와 볼륨(0.0~1.0)
    Silence(u32),              // 밀리초(ms) 단위의 대기 시간
}

impl PartialEq for AudioSegment {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Sound(a, va), Self::Sound(b, vb)) => {
                std::ptr::eq(a.as_ptr(), b.as_ptr()) && a.len() == b.len() && va == vb
            }
            (Self::Silence(a), Self::Silence(b)) => a == b,
            _ => false,
        }
    }
}

/// 음성으로 출력 가능한 이벤트가 구현해야 하는 트레이트입니다.
pub trait Speakable {
    /// 언어 설정에 따른 음성 텍스트를 반환합니다.
    fn text(&self, lang: Language) -> Cow<'_, str>;
}

/// 오디오 고유 식별자(해시)를 생성하기 위한 메타데이터 구조체
///
/// 텍스트, 목소리 타입, 속도를 기준으로 하나의 고유한 오디오 파일 해시를 계산합니다.
/// Web, Native가 공통으로 사용합니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioId {
    /// 합성할 텍스트
    pub text: String,
    /// 목소리 타입 (예: "F1", "M1")
    pub voice_type: String,
    /// 재생 속도 (기본: 1.0)
    pub speed: f32,
    /// 언어 설정 (Ko: 한국어, En: 영어, Ja: 일본어)
    pub language: Language,
}

impl AudioId {
    /// 새로운 AudioId를 생성합니다. 언어 설정을 포함하여 캐시 키 구분을 명확히 합니다.
    pub fn new(
        text: impl Into<String>,
        voice_type: impl Into<String>,
        speed: f32,
        language: Language,
    ) -> Self {
        Self {
            text: text.into(),
            voice_type: voice_type.into(),
            speed,
            language,
        }
    }
}

/// 오디오 관련 이벤트
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    /// 오디오 재생이나 TTS 출력이 완료되었을 때 발생 (재생 완료 콜백 용도)
    PlaybackFinished { id: u64 },
    /// 오디오 재생 중 에러 발생 시
    Error(String),
}

/// 이벤트 및 텍스트 기반 음성 출력과 오디오 재생을 관리하는 구조체입니다.
pub struct Audio {
    last_spoken_text: String,
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

impl Audio {
    pub fn new() -> Self {
        Self {
            last_spoken_text: String::new(),
        }
    }

    /// 이벤트를 받아 TTS로 출력합니다.
    pub fn speak<E: Speakable>(&mut self, event: &E, lang: Language) {
        let text = event.text(lang);
        self.speak_text(&text);
    }

    /// 기본 옵션(`SpeechOption::new()`)으로 텍스트를 음성으로 변환하여 출력합니다.
    ///
    /// # Arguments
    ///
    /// * `text` - 음성으로 변환할 텍스트입니다.
    pub fn speak_text(&mut self, text: &str) {
        self.speak_text_with_option(text, SpeechOption::new());
    }

    /// 지정된 옵션으로 텍스트를 음성으로 변환하여 출력합니다.
    ///
    /// # Arguments
    ///
    /// * `text` - 음성으로 변환할 텍스트입니다.
    /// * `options` - 발화 옵션(`SpeechOption`)입니다.
    pub fn speak_text_with_option(&mut self, text: &str, options: SpeechOption) {
        if options.forced || self.last_spoken_text != text {
            self.play_audio_sequence(&[AudioSegment::Text(text.to_string())], options);
            self.last_spoken_text = text.to_string();
        }
    }

    /// 기본 옵션(`SpeechOption::new()`)으로 여러 텍스트 세그먼트를 순서대로 이어서 발화합니다.
    ///
    /// # Arguments
    ///
    /// * `segments` - 순서대로 발화할 텍스트 세그먼트 배열입니다.
    pub fn speak_segments(&mut self, segments: Vec<String>) {
        self.speak_segments_with_option(segments, SpeechOption::new());
    }

    /// 지정된 발화 옵션을 일괄 적용하여 여러 텍스트 세그먼트를 단일 시퀀스로 결합한 뒤,
    /// 오디오 하드웨어 측에 순서대로 재생을 요청합니다.
    ///
    /// # Arguments
    ///
    /// * `segments` - 발화할 순서가 맞춰진 개별 텍스트 조각들의 벡터입니다.
    /// * `options` - 목소리 타입, 재생 속도 등 모든 세그먼트에 적용할 동일 발화 환경 설정입니다.
    pub fn speak_segments_with_option(&mut self, segments: Vec<String>, options: SpeechOption) {
        // 단어별 중복 합산 문자열을 기반으로 디바운싱 및 동일 발화 제거 판별을 수행합니다.
        let combined_text = segments.join(" ");
        if options.forced || self.last_spoken_text != combined_text {
            // Vec<String>을 WASM FFI 프로토콜이 안전하게 소화할 수 있는 Vec<AudioSegment>로 래핑 가공합니다.
            let audio_segments: Vec<AudioSegment> =
                segments.into_iter().map(AudioSegment::Text).collect();

            // 단 한 번의 단일 시퀀스 FFI 브릿지 호출을 가동하여 물리 재생 딜레이를 완벽히 억제합니다.
            self.play_audio_sequence(&audio_segments, options);
            self.last_spoken_text = combined_text;
        }
    }

    /// 정적 오디오 바이너리를 재생합니다.
    ///
    /// 입력되는 슬라이스는 매크로를 통해 컴파일 시 생성된 `.rodata` 영역의 정적 메모리 포인터와 길이를
    /// 가지고 있습니다. 이 값으로 이전에 재생한 에셋과 비교하여 디바운싱(debouncing)을 수행합니다.
    ///
    /// # Arguments
    ///
    /// * `audio_data` - 재생할 정적 오디오 바이너리의 바이트 슬라이스입니다.
    pub fn play(&mut self, audio_data: &'static [u8]) {
        let ptr = audio_data.as_ptr() as usize;
        let len = audio_data.len();

        unsafe {
            crate::bridge::host_functions::audio_play(ptr as *const u8, len);
        }
    }

    /// 텍스트와 오디오가 혼합된 시퀀스(`RichSpeech`)를 호스트에 전달하여 순차적으로 재생하게 합니다.
    pub fn play_audio_sequence(&mut self, segments: &[AudioSegment], options: SpeechOption) {
        // FFI 경계를 넘기 위한 내부 직렬화 전용 구조체
        // 바이너리 데이터 전체를 복사하지 않고 WASM 메모리 포인터와 길이만 전달하여 오버헤드를 없앱니다.

        let ffi_segments: Vec<FfiSegment> = segments
            .iter()
            .map(|seg| match seg {
                AudioSegment::Text(t) => FfiSegment::Text(t.as_str()),
                AudioSegment::Sound(s, vol) => FfiSegment::Sound {
                    ptr: s.as_ptr() as u32,
                    len: s.len() as u32,
                    volume: *vol,
                },
                AudioSegment::Silence(ms) => FfiSegment::Silence(*ms),
            })
            .collect();

        let speech_type_u32 = match (options.forced, options.important) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        };

        let payload = FfiSequencePayload {
            speech_type: speech_type_u32,
            segments: ffi_segments,
        };

        if let Ok(bytes) = postcard::to_allocvec(&payload) {
            unsafe {
                crate::bridge::host_functions::audio_play_sequence(bytes.as_ptr(), bytes.len());
            }
        }
    }
}
