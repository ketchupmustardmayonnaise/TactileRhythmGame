#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// WASM Export 설정 중 발생한 에러를 나타냅니다.
    #[error("Export error `{0}`: {1}")]
    ExportError(String, String),
    /// 내보낸 함수가 예상한 유형이 아닐 때 발생하는 에러를 나타냅니다.
    #[error("Not export function error: {0}")]
    NotExportFunctionError(String),
    /// SDK 관련 에러를 래핑합니다.
    #[error("SDK error: {0}")]
    SdkError(#[from] sdk::error::Error),
    /// 전역 윈도우 객체가 존재하지 않을 때 발생하는 에러를 나타냅니다.
    #[error("No global `window` exists error")]
    NoGlobalWindowExistsError,
    /// JS 함수를 가져오지 못했을 때 발생하는 에러를 나타냅니다.
    #[error("Failed to call JS function `{0}`: {1}")]
    JsFunctionError(String, String),
    /// 내보낸 함수 실행에 실패했을 때 발생하는 에러를 나타냅니다.
    #[error("Failed to run export function `{0}`: {1}")]
    ExportFunctionRunError(String, String),
    /// 초기화 중에 발생한 에러를 나타냅니다.
    #[error("Storage error: {0}")]
    StorageError(String),
    /// 직렬화 또는 역직렬화 중에 발생한 에러를 나타냅니다.
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// 너무 큰 데이터 요청이 있을 때 발생하는 에러를 나타냅니다.
    #[error(
        "Too large data request in `{from}`: requested length {len}, expected max {expected_max}"
    )]
    TooLargeDataRequestError {
        from: String,
        len: usize,
        expected_max: usize,
    },
}

/// `sdk` 크레이트 전반에 걸쳐 사용되는 결과 타입의 별칭입니다.
pub type Result<T> = std::result::Result<T, Error>;
