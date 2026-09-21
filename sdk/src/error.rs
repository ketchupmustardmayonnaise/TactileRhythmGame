/// `sdk` 크레이트에서 발생할 수 있는 사용자 정의 에러 타입을 정의하는 열거형입니다.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// 디스플레이 관련 작업 중 발생한 에러를 나타냅니다.
    #[error("Display error: {0}")]
    DisplayError(String),
    /// 초기화 과정에서 발생한 에러를 나타냅니다.
    #[error("Initialization error: {0}")]
    InitializationError(String),
    /// 런타임 실행 중 발생한 에러를 나타냅니다.
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    /// 값이 허용된 범위를 벗어났을 때 발생하는 에러를 나타냅니다.
    #[error("Out of range error")]
    OutOfRangeError,
    /// WASM 임포트 설정 중 발생한 에러를 나타냅니다.
    #[error("Failed to set import: {0}")]
    ImportError(String),
    /// 설정 키가 없습니다.
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    /// 설정 값이 유효하지 않습니다.
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    /// 버퍼 크기가 작습니다.
    #[error("Buffer too small")]
    BufferTooSmall,
    /// Failed to set preference
    #[error("Failed to set preference: {0}")]
    FailedToSetPreference(String),
    /// Failed to get preference
    #[error("Failed to get preference: {0}")]
    FailedToGetPreference(String),
    /// Set logger error
    #[error("Set logger error: {0}")]
    SetLoggerError(String),
}

/// `sdk` 크레이트 전반에 걸쳐 사용되는 결과 타입의 별칭입니다.
/// 성공 시 `T` 값을, 실패 시 `sdk::Error`를 반환합니다.
pub type Result<T> = core::result::Result<T, Error>;
