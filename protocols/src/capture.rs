use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

extern crate alloc;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: i32,
    pub security: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CaptureRequestToRuntime {
    LaunchApplet {
        name: String,
    },
    UpdateDisplay {
        width: u16,
        height: u16,
        data: Vec<u8>,
        bits_per_pixel: u8,
    },
    ScanWifi,
    ConnectWifi {
        ssid: String,
        passphrase: String,
    },
    DisconnectWifi {
        ssid: String,
    },
    GetNetworkAddresses,
    CancelWifiOperation,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CaptureResponseFromRuntime {
    Ok,
    Error,
    Paused,
    WifiScanResult(Vec<WifiNetwork>),
    WifiConnectResult { success: bool, message: String },
    WifiDisconnectResult { success: bool, message: String },
    NetworkAddressesResult(Vec<String>),
}
