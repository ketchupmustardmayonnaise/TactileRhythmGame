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
    /// 애플릿이 `context.preferences.set_string` 으로 저장한 값을 PC 에서 읽어 갑니다.
    /// (예: `tactile-experiment` 의 측정 결과)
    ///
    /// postcard 는 변형 순서로 직렬화하므로 **반드시 맨 뒤에** 추가해야 기존 도구와 호환됩니다.
    GetPreferenceString {
        key: String,
    },
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
    /// `GetPreferenceString` 의 응답. 키가 없으면 `None`.
    PreferenceStringResult(Option<String>),
}
