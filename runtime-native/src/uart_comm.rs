use anyhow::Result;
use host_utils::serial_comm::{ConnectionStatus, SerialComm};
use protocols::esp32::{RequestToEsp32, ResponseFromEsp32};
use tokio::sync::mpsc::{Receiver, Sender};

pub struct UartComm {
    pub esp32_request_sender: Sender<RequestToEsp32>,
    pub esp32_response_receiver: Receiver<ResponseFromEsp32>,
    pub status_receiver: Receiver<ConnectionStatus>,
}

impl UartComm {
    pub fn new<S: Into<String>>(path: S, baudrate: u32) -> Result<Self> {
        let path = path.into();
        let SerialComm {
            tx_sender,
            rx_receiver,
            status_receiver,
        } = SerialComm::<RequestToEsp32, ResponseFromEsp32>::new(&path, baudrate, None)?;

        Ok(Self {
            esp32_request_sender: tx_sender,
            esp32_response_receiver: rx_receiver,
            status_receiver,
        })
    }
}
