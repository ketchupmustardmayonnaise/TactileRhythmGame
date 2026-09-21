use thiserror::Error;

/// 오디오 처리 및 재생 중 발생할 수 있는 공통 에러 타입입니다.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("스토리지 에러: {0}")]
    Storage(String),

    #[error("재생 에러: {0}")]
    Playback(String),

    #[error("네트워크/서버 통신 에러: {0}")]
    Network(String),
}
