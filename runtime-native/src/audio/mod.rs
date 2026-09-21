use runtime_common::audio::client::AudioServiceClient;
use runtime_common::audio::protocol::{PlaySequenceRequest, SequenceSegment};
use tokio::sync::OnceCell;

/// 전역 오디오 처리 Facade 인터페이스 (OnceCell을 통해 필요할 때 비동기 초기화)
static AUDIO_FACADE: OnceCell<AudioFacade> = OnceCell::const_new();

async fn get_audio_facade() -> anyhow::Result<&'static AudioFacade> {
    AUDIO_FACADE.get_or_try_init(AudioFacade::new).await
}

/// 오디오 관련 요청을 오디오 서비스 클라이언트로 중계하는 역할을 수행합니다.
pub struct AudioFacade {
    client: AudioServiceClient,
}

impl AudioFacade {
    pub async fn new() -> anyhow::Result<Self> {
        let client = AudioServiceClient::new();
        Ok(Self { client })
    }

    /// WASM 앱에서 넘어온 바이너리 오디오 재생 요청을 처리합니다.
    pub async fn play_audio_data(&self, audio_data: Vec<u8>, volume: f32) -> anyhow::Result<()> {
        self.client
            .request_play_sound(audio_data, volume)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// WASM 앱에서 넘어온 시퀀스(텍스트+오디오+묵음) 재생 요청을 처리합니다.
    pub async fn handle_sequence_request(
        &self,
        segments: Vec<SequenceSegment>,
        options: sdk::applet::SpeechOption,
    ) -> anyhow::Result<()> {
        let settings = crate::host::preferences::AppSettings::load();
        let voice_type = format!("{:?}", sdk::types::Voice::from(settings.tts_voice));
        let is_important = options.important;
        let is_forced = options.forced;

        // 설정된 tts_speed 값을 바탕으로 실제 f32 배속으로 매핑합니다 (0: 느리게, 1: 보통, 2: 빠르게)
        let speed = sdk::types::Speed::from(settings.tts_speed).as_f32();

        // 설정된 언어 설정을 sdk::Language 타입으로 변환합니다.
        let language = sdk::Language::from(settings.language);

        let request = PlaySequenceRequest::builder()
            .segments(segments)
            .voice_type(voice_type)
            .speed(speed)
            .language(language)
            .is_important(is_important)
            .is_forced(is_forced)
            .build();

        self.client
            .request_audio_segments(&request)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}

/// 호스트 환경에서 호출되는 raw 오디오(Vec<u8>) 재생 엔트리포인트.
pub fn play(audio_data: Vec<u8>, volume: f32) {
    // 백그라운드 태스크로 오디오 재생 요청 전송
    tokio::spawn(async move {
        if let Ok(facade) = get_audio_facade().await {
            if let Err(e) = facade.play_audio_data(audio_data, volume).await {
                tracing::error!("오디오 바이너리 재생 중 오류 발생: {:?}", e);
            }
        } else {
            tracing::error!("AudioFacade 초기화 실패");
        }
    });
}

/// TTS 엔진의 언어 및 목소리 설정을 런타임에 즉시 업데이트(모델 교체)합니다.
pub fn update_tts_settings(language: i32, tts_voice: i32, tts_speed: i32) {
    // sdk::types에 정의된 Enum과 From<i32>를 사용하여 문자열로 깔끔하게 변환합니다.
    let lang_str = format!("{:?}", sdk::types::Language::from(language));
    let voice_str = format!("{:?}", sdk::types::Voice::from(tts_voice));
    let speed_str = format!("{:?}", sdk::types::Speed::from(tts_speed));

    // 백그라운드 태스크로 오디오 모듈에 설정 업데이트 요청 전송
    tokio::spawn(async move {
        if let Ok(facade) = get_audio_facade().await
            && let Err(e) = facade
                .client
                .request_update_settings(&voice_str, &lang_str, &speed_str)
                .await
        {
            tracing::error!("TTS 설정 업데이트(모델 교체) 중 오류 발생: {:?}", e);
        }
    });
}

/// 시스템 전역 오디오 볼륨을 설정합니다. (0.0 ~ 1.0)
pub fn set_volume(volume: f32) {
    // 백그라운드 태스크로 오디오 볼륨 설정 로직 전송
    tokio::spawn(async move {
        if let Ok(facade) = get_audio_facade().await {
            if let Err(e) = facade.client.request_set_volume(volume).await {
                tracing::error!("볼륨 설정 중 오류 발생: {:?}", e);
            } else {
                tracing::info!("시스템 오디오 볼륨이 {:.2}로 변경되었습니다.", volume);
            }
        } else {
            tracing::error!("AudioFacade 초기화 실패로 볼륨을 설정할 수 없습니다.");
        }
    });
}

/// WASM 모듈에서 오디오 시퀀스 재생을 요청하는 진입점입니다.
pub async fn play_sequence(
    segments: Vec<SequenceSegment>,
    options: sdk::applet::SpeechOption,
) -> anyhow::Result<()> {
    get_audio_facade()
        .await?
        .handle_sequence_request(segments, options)
        .await
}
