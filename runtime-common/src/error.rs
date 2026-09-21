#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HTTP request error: {0}")]
    HttpRequestError(#[from] reqwest::Error),
    #[error("HTTP response error with context: error={0} msg={1}")]
    HttpRequestErrorWithContext(reqwest::Error, String),
    #[error("HTTP response error: {0}")]
    HttpResponseError(reqwest::StatusCode),
    #[error("HTTP response error with context: code={0} msg={1}")]
    HttpResponseErrorWithContext(reqwest::StatusCode, String),
    #[error("TTS audio is not played on server")]
    TtsAudioNotPlayedOnServer,
}

pub type Result<T> = std::result::Result<T, Error>;
