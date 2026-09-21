mod audio;
mod chunk;
mod config;
mod error;
mod tts;

use audio::{AudioPlayer, PlaybackCategory};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use config::ServiceConfig;
use error::Result;
use runtime_common::audio::protocol::{PlaySequenceRequest, SequenceSegment};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tower_http::cors::CorsLayer;
use tracing::info;
use tts::{TtsActor, TtsCommand, TtsMessage};
#[derive(Clone)]
struct AppState {
    tx: mpsc::Sender<TtsCommand>,
    audio_player: AudioPlayer,
}

#[derive(Deserialize)]
struct VolumeRequest {
    volume: f32,
}

#[derive(Deserialize)]
struct SettingsRequest {
    voice: String,
    language: String,
}

#[derive(Deserialize)]
struct SoundQuery {
    #[serde(default = "default_sound_volume")]
    volume: f32,
}

fn default_sound_volume() -> f32 {
    1.0
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 환경 설정 및 로깅 초기화
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    // 2. 서비스 설정 로드
    let config = ServiceConfig::from_env()?;
    info!("TTS 서비스 시작 (포트: {})", config.port);

    // 3. TTS 액터 실행 (엔진 초기화 포함)
    info!("TTS 엔진 초기화 대기 중...");
    let tx = TtsActor::spawn(config.clone()).await?;
    info!("TTS 엔진 초기화 완료.");
    let audio_player = AudioPlayer::new();

    let state = AppState { tx, audio_player };

    // 5. 웹 서버 설정
    let app = Router::new()
        .route("/", get(root))
        .route("/sound", post(handle_sound))
        .route("/audio_segments", post(handle_audio_segments))
        .route("/volume", post(handle_set_volume))
        .route("/settings", post(handle_update_settings))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("127.0.0.1:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("수신 대기 중: {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "TTS Service is running (Modular Structure)."
}

async fn get_tts_audio_data(
    state: &AppState,
    text: &str,
    voice_type: &str,
    speed: f32,
    language: sdk::Language,
    category: PlaybackCategory,
    clear_first_track: bool,
) -> Result<Vec<f32>> {
    // 런타임 유입 텍스트에 포함될 수 있는 불필요한 앞뒤 공백에 따른 캐시 불일치(미스)를 원천 차단하기 위해
    // 텍스트를 가장 먼저 trim(정제) 처리합니다.
    let text = text.trim();

    // let audio_id = AudioId::new(text, voice_type, speed, language);
    // let hash = HashGenerator::generate(&audio_id);

    // let mut audio_data_opt = None;

    // 2. 캐시 미스(Miss) 시: 문장을 청크 단위로 분할하여 실시간 스트리밍 음성 합성 및 무지연 큐잉을 처리합니다.
    use crate::chunk::TextChunker;
    let chunker = TextChunker::new(30); // 최소 10자 단위 스마트 청킹 적용
    let chunks = chunker.chunk_text(text);

    let mut full_samples = Vec::new();
    let target_sample_rate = state.audio_player.sample_rate;

    for (index, chunk) in chunks.into_iter().enumerate() {
        if chunk.trim().is_empty() {
            continue;
        }

        // 각 청크를 순차적으로 합성 요청 (말소리 출력 순서 보장)
        let (resp_tx, resp_rx) = oneshot::channel();
        state
            .tx
            .send(TtsCommand::Synthesize(TtsMessage {
                text: chunk.to_string(),
                speed,
                voice_type: voice_type.to_string(),
                language,
                target_sample_rate,
                respond_to: resp_tx,
            }))
            .await
            .map_err(|e| error::Error::ActorSend(format!("TTS 액터 명령어 전송 실패: {}", e)))?;

        let chunk_samples = resp_rx
            .await
            .map_err(|e| error::Error::ActorReceive(format!("TTS 액터 응답 수신 실패: {}", e)))??;

        if !chunk_samples.is_empty() {
            // 첫 번째 청크가 합성 완료되자마자 기존 오디오 재생 트랙들을 한 번 청소합니다.
            // 이후 후속 청크들은 트랙 초기화 없이 자연스럽게 뒤로 붙어 스트리밍됩니다.
            if index == 0 && clear_first_track {
                let _ = state.audio_player.clear(category);
            }

            // [추가] 실시간 합성된 음성 조각(PCM 샘플)을 오디오 플레이어의 장치 큐에 즉시 등록하여 소리를 재생합니다.
            let _ = state.audio_player.play_raw(
                chunk_samples.clone(),
                target_sample_rate,
                category,
                1.0,
                false,
            );

            // [추가] 최종적으로 전체 반환 데이터 세트에도 합성 완료된 음성 조각들을 합쳐줍니다.
            full_samples.extend(chunk_samples);
        }
    }

    Ok(full_samples)
}

async fn handle_audio_segments(
    State(state): State<AppState>,
    Json(payload): Json<PlaySequenceRequest>,
) -> Result<impl IntoResponse> {
    info!(
        "시퀀스 재생 요청 수신: {}개의 세그먼트 (Streaming 모드)",
        payload.segments.len()
    );

    let target_sample_rate = state.audio_player.sample_rate; // 병합 기준 샘플 레이트

    let category = if payload.is_important {
        PlaybackCategory::TtsImportant
    } else {
        PlaybackCategory::TtsNormal
    };

    // [추가] 신규 일반 음성 안내(TtsNormal) 요청이 유입되었을 때, 현재 재생 대기 중이거나 작동 중인 실제 활성 태스크가 있는지 사전에 확인합니다.
    // 활성 상태로 실행 중인 이전 일반 음성 태스크가 없는 경우에만 '첫 번째 요청'으로 판정합니다.
    let is_first_request = if category == PlaybackCategory::TtsNormal {
        !state.audio_player.has_active_normal_tasks().await
    } else {
        false
    };

    // 신규 시퀀스 재생 요청이 유입되었으므로, 백그라운드 비동기 스레드를 스폰하기 전에
    if category == PlaybackCategory::TtsNormal {
        state.audio_player.clear_normal_tasks().await; // 음성 합성 루프 종료
        // 오디오 트랙은 즉시 날리지 않고, 디바운스 대기(Sleep)가 무사히 완료된 후에 날려 부드러운 소리 전환을 도모합니다.
        // let _ = state.audio_player.clear(category); // 트랙도 날림
    }

    let state_clone = state.clone();

    // 웹 브라우저의 대기(블로킹) 렉을 해소하기 위해 오디오 디코딩/리샘플링 및 재생 처리를 완전히 백그라운드로 넘깁니다.
    let task_handle = tokio::spawn(async move {
        // 연속 요청시, 마지막 요청만 재생하기 위해, 일반 음성 안내(TtsNormal) 카테고리에 대해 대기 시간을 가집니다.
        // [개선] 단, 이번 요청이 고요한 상태에서 최초로 유입된 첫 요청인 경우에는 대기(Sleep)를 전면 생략하여 즉시 반응합니다.
        if category == PlaybackCategory::TtsNormal && !is_first_request {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        // 디바운스 대기가 정상 종료(새로운 연속 요청에 의해 중간에 abort 되지 않음)되었거나 첫 요청인 시점에만
        // 비로소 실제 사운드 하드웨어 재생 트랙을 깨끗하게 정리(Clear)합니다.
        if category == PlaybackCategory::TtsNormal {
            let _ = state_clone.audio_player.clear(category);
        }

        let mut is_first_segment = true;
        // 각 세그먼트를 순회하며 준비되는 '즉시' 플레이어 큐에 넣습니다. (도중에 멈추지 않고 끝까지 빌드하여 캐시합니다.)
        for segment in payload.segments {
            match segment {
                SequenceSegment::Text(text) => {
                    // 첫 번째 청크가 합성 완료되는 즉시 clear
                    if let Err(e) = get_tts_audio_data(
                        &state_clone,
                        &text,
                        &payload.voice_type,
                        payload.speed,
                        payload.language,
                        category,
                        is_first_segment && !payload.is_forced, // 시퀀스의 첫 세그먼트일 때만 기존 트랙을 clear 합니다.
                                                                // true, // clear_first_track 인자 추가 (첫 청크 완료 시 한 번 초기화)
                    )
                    .await
                    {
                        tracing::error!("스트리밍 음성 합성 및 재생 연동 중 에러 발생: {:?}", e);
                    }
                    is_first_segment = false; // 첫 번째 세그먼트 이후에는 clear하지 않도록 플래그를 업데이트합니다.
                }
                SequenceSegment::Sound(audio_data, volume) => {
                    if is_first_segment {
                        // 첫 번째 세그먼트가 물리 사운드일 경우, 믹서 큐를 정리하여 무지연으로 즉시 교체합니다.
                        // (비동기 태스크는 이미 위 동기 진입점에서 즉각 abort 완료되었습니다.)
                        if !payload.is_forced {
                            let _ = state_clone.audio_player.clear(category);
                        }
                        is_first_segment = false;
                    }
                    let vol = if volume.is_nan() { 1.0 } else { volume };
                    match crate::audio::decode_to_pcm(audio_data, target_sample_rate, vol) {
                        Ok(sound_samples) => {
                             if !sound_samples.is_empty() {
                                 let _ = state_clone.audio_player.play_raw(
                                    sound_samples,
                                    target_sample_rate,
                                    category,
                                    1.0,
                                    false,
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("사운드 세그먼트 디코딩 실패: {:?}", e);
                        }
                    }
                }
                SequenceSegment::Silence(ms) => {
                    if is_first_segment {
                        // 첫 번째 세그먼트가 묵음일 경우에도 믹서 큐를 정리하여 무지연으로 즉시 교체합니다.
                        if !payload.is_forced {
                            let _ = state_clone.audio_player.clear(category);
                        }
                        is_first_segment = false;
                    }
                    let sample_count = ((ms as f32 / 1000.0) * target_sample_rate) as usize;
                    if sample_count > 0 {
                        let silence_samples = vec![0.0; sample_count];
                        let _ = state_clone.audio_player.play_raw(
                            silence_samples,
                            target_sample_rate,
                            category,
                            1.0,
                            false,
                        );
                    }
                }
            }
        }
    });

    // state.audio_player.clear_active_tasks().await;
    // 백그라운드 재생 태스크 큐 관리 책임을 AudioPlayer 내부의 register_task로 위임
    state
        .audio_player
        .register_task(category, task_handle)
        .await;

    // 백그라운드 연산 시작과 동시에 브라우저 클라이언트에는 지연 없이 즉각 완료 응답을 반환합니다.
    let mut response = ().into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("x-sequence-played"),
        axum::http::HeaderValue::from_static("true"),
    );
    Ok(response)
}

async fn handle_update_settings(
    State(state): State<AppState>,
    Json(payload): Json<SettingsRequest>,
) -> Result<impl IntoResponse> {
    info!(
        "설정 업데이트 요청 수신: voice={}, language={}",
        payload.voice, payload.language
    );

    let (resp_tx, resp_rx) = oneshot::channel();

    let msg = tts::UpdateSettingsMessage {
        voice: payload.voice,
        language: payload.language,
        respond_to: resp_tx,
    };

    state
        .tx
        .send(TtsCommand::UpdateSettings(msg))
        .await
        .map_err(|e| error::Error::ActorSend(format!("Channel closed: {}", e)))?;

    resp_rx
        .await
        .map_err(|e| error::Error::ActorReceive(format!("Response channel dropped: {}", e)))??;

    let mut response = ().into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("x-settings-updated"),
        axum::http::HeaderValue::from_static("true"),
    );
    Ok(response)
}

async fn handle_set_volume(
    State(state): State<AppState>,
    Json(payload): Json<VolumeRequest>,
) -> Result<impl IntoResponse> {
    info!("볼륨 변경 요청 수신: {:.2}", payload.volume);
    state.audio_player.set_volume(payload.volume)?;

    let mut response = ().into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("x-volume-changed"),
        axum::http::HeaderValue::from_static("true"),
    );
    Ok(response)
}

async fn handle_sound(
    State(state): State<AppState>,
    Query(query): Query<SoundQuery>,
    body: Bytes,
) -> Result<impl IntoResponse> {
    info!(
        "오디오 바이너리 재생 요청 수신: {} bytes, volume: {}",
        body.len(),
        query.volume
    );
    // 이전 효과음들을 명시적으로 청소한 뒤 새 효과음을 즉각 재생합니다.
    state.audio_player.clear(PlaybackCategory::SoundEffect)?;
    let volume = if query.volume.is_nan() { 1.0 } else { query.volume };
    state.audio_player.play(
        body.to_vec(),
        PlaybackCategory::SoundEffect,
        volume,
        false,
    )?;

    let mut response = ().into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("x-audio-played"),
        axum::http::HeaderValue::from_static("true"),
    );
    Ok(response)
}
