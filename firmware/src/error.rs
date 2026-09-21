use alloc::string::String;

extern crate alloc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SPI error: {0}")]
    SpiError(String),

    #[error("UART configuration error: {0}")]
    UartConfigError(#[from] esp_hal::uart::ConfigError),
}

pub type Result<T> = core::result::Result<T, Error>;
