use host_utils::serial_comm::ConnectionStatus;
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};

use crate::net_comm::NetComm;
use crate::usb_comm::UsbComm;
use crate::ws_comm::WsComm;

pub enum AppComm {
    Usb(UsbComm),
    Net(NetComm),
    Ws(WsComm),
}

impl AppComm {
    pub fn send_request(&mut self, req: &CaptureRequestToRuntime) -> std::io::Result<()> {
        match self {
            AppComm::Usb(c) => c.send_request(req),
            AppComm::Net(c) => c.send_request(req),
            AppComm::Ws(c) => c.send_request(req),
        }
    }
    pub fn try_receive_status(&mut self) -> Option<ConnectionStatus> {
        match self {
            AppComm::Usb(c) => c.try_receive_status(),
            AppComm::Net(c) => c.try_receive_status(),
            AppComm::Ws(c) => c.try_receive_status(),
        }
    }
    pub fn try_receive_response(&mut self) -> Option<std::io::Result<CaptureResponseFromRuntime>> {
        match self {
            AppComm::Usb(c) => c.try_receive_response(),
            AppComm::Net(c) => c.try_receive_response(),
            AppComm::Ws(c) => c.try_receive_response(),
        }
    }
    pub fn receive_response(&mut self) -> std::io::Result<CaptureResponseFromRuntime> {
        match self {
            AppComm::Usb(c) => c.receive_response(),
            AppComm::Net(c) => c.receive_response(),
            AppComm::Ws(c) => c.receive_response(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionType {
    Serial,
    Network,
    WebSocket,
}
