use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// TTS 서비스 전반에서 사용하는 에러 타입
#[derive(Error, Debug)]
pub enum Error {
    /// Piper TTS 엔진 오류
    #[error("Piper TTS 엔진 오류: {0}")]
    Piper(String),

    /// 액터(Actor) 통신 오류 (요청 전송 실패)
    #[error("TTS 액터로 요청 전송 실패: {0}")]
    ActorSend(String),

    /// 액터(Actor) 통신 오류 (응답 수신 실패)
    #[error("TTS 액터로부터 응답 수신 실패: {0}")]
    ActorReceive(String),

    /// 오디오 장치 없음 (웹으로 오디오 데이터 반환 Fallback)
    #[error("오디오 장치를 찾을 수 없음")]
    AudioDeviceNotFound(Vec<u8>),

    /// 네트워크/서버 바인딩 등 I/O 오류
    #[error("I/O 오류: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Serialize, Deserialize)]
pub struct TtsErrorResponse {
    pub error: String,
}

impl TtsErrorResponse {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

// 웹 서버 응답을 위한 변환 구현
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // [특수 케이스] 오디오 장치가 없는 경우 (Web Fallback)
        if let Error::AudioDeviceNotFound(data) = self {
            return Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "audio/wav")
                .header("X-Audio-Status", "DeviceNotFound-Fallback")
                .body(axum::body::Body::from(data))
                .unwrap_or_else(|e| {
                    tracing::error!("Fallback 응답 생성 실패: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "내부 서버 오류").into_response()
                });
        }

        let (status, error_message) = match self {
            // 500 Internal Server Error: 서버 내부 문제
            Error::Piper(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Piper TTS 엔진 실패: {}", e),
            ),
            Error::ActorSend(_) | Error::ActorReceive(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "내부 통신 오류".to_string(),
            ),
            Error::Io(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "내부 I/O 오류".to_string(),
            ),
            Error::AudioDeviceNotFound(_) => unreachable!(), // 위에서 처리됨
        };

        (status, axum::Json(TtsErrorResponse::new(error_message))).into_response()
    }
}
