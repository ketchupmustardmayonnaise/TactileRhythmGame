use esp_hal::{
    Async, Blocking, DriverMode,
    gpio::interconnect::{PeripheralInput, PeripheralOutput},
    uart::{Config, Instance, Uart},
};

use crate::Result;
use crate::board_config::*;

pub struct UartCommConfig<UART, TX, RX> {
    pub uart: UART,
    pub tx: TX,
    pub rx: RX,
}

/// 어플리케이션 전용 UART 래퍼(Wrapper)
pub struct UartComm<'d, M: DriverMode> {
    pub inner: Uart<'d, M>,
}

impl<'d> UartComm<'d, Blocking> {
    /// 새로운 UART 인스턴스를 생성합니다.
    pub fn new<UART, TX, RX>(config: UartCommConfig<UART, TX, RX>) -> Result<Self>
    where
        UART: Instance + 'd,
        TX: PeripheralOutput<'d>,
        RX: PeripheralInput<'d>,
    {
        let uart_config = Config::default().with_baudrate(UART_BAUDRATE);

        let inner = Uart::new(config.uart, uart_config)?
            .with_tx(config.tx)
            .with_rx(config.rx);

        Ok(Self { inner })
    }

    /// UART를 비동기(Async) 모드로 변환합니다.
    pub fn into_async(self) -> UartComm<'d, Async> {
        UartComm {
            inner: self.inner.into_async(),
        }
    }
}
