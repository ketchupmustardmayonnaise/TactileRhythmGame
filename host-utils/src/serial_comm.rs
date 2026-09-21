use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::time::{Duration, sleep};
#[cfg(target_os = "linux")]
use tokio_serial::SerialPort;
use tokio_serial::SerialPortBuilderExt;
use tracing::{error, info, warn};

const RETRY_DELAY_SECS: u64 = 5;
const RECONNECT_DELAY_SECS: u64 = 5;
const RX_BUFFER_SIZE: usize = 8192;
const TX_BUFFER_SIZE: usize = 4096;
pub const CHANNEL_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected { port: Option<String> },
    Disconnected,
}

impl ConnectionStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected { .. })
    }
}

#[derive(Error, Debug)]
pub enum SerialCommError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serial port error: {0}")]
    Serial(#[from] tokio_serial::Error),
    #[error("Postcard error: {0}")]
    Postcard(#[from] postcard::Error),
}

pub type Result<T> = std::result::Result<T, SerialCommError>;

pub fn available_ports(filter: Option<(u16, u16)>) -> Vec<String> {
    tokio_serial::available_ports()
        .map(|ports| {
            ports
                .into_iter()
                .filter(|p| {
                    if let Some((vid, pid)) = filter {
                        if let tokio_serial::SerialPortType::UsbPort(info) = &p.port_type {
                            info.vid == vid && info.pid == pid
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                })
                .map(|p| p.port_name)
                .collect()
        })
        .unwrap_or_default()
}

/// 제네릭 타입을 이용해 어떠한 종류의 패킷이라도 COBS 방식으로 안전하게
/// 시리얼 송수신 및 재연결을 처리할 수 있는 공통 모듈입니다.
pub struct SerialComm<TxItem, RxItem> {
    pub tx_sender: Sender<TxItem>,
    pub rx_receiver: Receiver<RxItem>,
    pub status_receiver: Receiver<ConnectionStatus>,
}

impl<TxItem, RxItem> SerialComm<TxItem, RxItem>
where
    TxItem: Serialize + Send + 'static,
    RxItem: DeserializeOwned + Send + 'static,
{
    pub fn new(port_name: &str, baud_rate: u32, filter: Option<(u16, u16)>) -> Result<Self> {
        let (tx_sender, mut tx_receiver) = channel::<TxItem>(CHANNEL_CAPACITY);
        let (rx_sender, rx_receiver) = channel::<RxItem>(CHANNEL_CAPACITY);
        let (status_sender, status_receiver) = channel::<ConnectionStatus>(CHANNEL_CAPACITY);

        let mut port_name = port_name.to_string();

        tokio::spawn(async move {
            loop {
                if status_sender
                    .send(ConnectionStatus::Disconnected)
                    .await
                    .is_err()
                {
                    info!("status_sender disconnected");
                    return; // status_receiver가 드롭되었으므로 완전 종료
                }

                let ports = if filter.is_some() {
                    // 연결 시도 전에 현재 포트가 유효한지 선제적 체크
                    available_ports(filter)
                } else {
                    vec![port_name.clone()]
                };
                info!("ports: {:?}", ports);
                if !ports.contains(&port_name) {
                    info!(
                        "Serial port {} is not available. Connection attempt paused.",
                        port_name
                    );
                    if ports.len() == 1 {
                        let new_port = ports[0].clone();
                        info!(
                            "Found exactly one active port: {}. Switching connection target to it.",
                            new_port
                        );
                        port_name = new_port;
                    } else {
                        sleep(Duration::from_secs(1)).await;
                    }
                    continue;
                }

                let port_name_clone = port_name.clone();
                let open_result = tokio::time::timeout(
                    Duration::from_secs(3),
                    tokio::task::spawn_blocking(move || {
                        tokio_serial::new(&port_name_clone, baud_rate).open_native_async()
                    }),
                )
                .await;

                let mut port = match open_result {
                    Ok(Ok(Ok(p))) => {
                        info!("Serial port opened successfully: {}", port_name);
                        if status_sender
                            .send(ConnectionStatus::Connected {
                                port: Some(port_name.clone()),
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }
                        p
                    }
                    Ok(Ok(Err(e))) => {
                        warn!(
                            "Failed to open serial port {}: {}. Retrying...",
                            port_name, e
                        );
                        // 5초 대기하는 동안 1초 간격으로 포트 이탈 여부를 체크하여 빠른 반응 확보 (윈도우 환경 전용)
                        for _ in 0..RETRY_DELAY_SECS {
                            sleep(Duration::from_secs(1)).await;
                            if !available_ports(filter).contains(&port_name) {
                                break;
                            }
                        }
                        continue;
                    }
                    Ok(Err(join_err)) => {
                        warn!(
                            "Failed to spawn serial open task for {}: {}. Retrying...",
                            port_name, join_err
                        );
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    Err(_) => {
                        warn!(
                            "Connection attempt to {} timed out after 3 seconds. Retrying...",
                            port_name
                        );
                        for _ in 0..RETRY_DELAY_SECS {
                            sleep(Duration::from_secs(1)).await;
                            if !available_ports(filter).contains(&port_name) {
                                break;
                            }
                        }
                        continue;
                    }
                };

                #[cfg(unix)]
                port.set_exclusive(false).unwrap_or_else(|e| {
                    error!("Unable to set serial port exclusive to false: {}", e)
                });

                #[cfg(target_os = "linux")]
                if let Err(e) = port.clear(tokio_serial::ClearBuffer::All) {
                    warn!("Failed to clear serial buffers for {}: {:?}", port_name, e);
                }

                let (mut reader, mut writer) = tokio::io::split(port);

                let mut rx_buf = vec![0u8; RX_BUFFER_SIZE];
                let mut tx_buf = vec![0u8; TX_BUFFER_SIZE];
                let mut rx_len = 0;

                loop {
                    tokio::select! {
                        // 1. 수신 (Rx) 처리
                        res = reader.read(&mut rx_buf[rx_len..]) => {
                            match res {
                                Ok(0) => {
                                    info!("Serial stream closed ({})", port_name);
                                    break; // 연결이 끊기면 외부 루프로 나가서 재연결
                                }
                                Ok(n) => {
                                    rx_len += n;
                                    while let Some(end_idx) = rx_buf[..rx_len].iter().position(|&b| b == 0x00) {
                                    if end_idx > 0 {
                                        match postcard::from_bytes_cobs::<RxItem>(&mut rx_buf[..=end_idx]) {
                                            Ok(item) => {
                                                if rx_sender.send(item).await.is_err() {
                                                    return; // rx_receiver가 드롭되었으므로 완전 종료
                                                }
                                            }
                                            Err(e) => warn!("Postcard Deserialize Error ({}, len={}): {:?}", port_name, end_idx, e),
                                        }
                                    }
                                        let remaining = rx_len - (end_idx + 1);
                                        rx_buf.copy_within(end_idx + 1..rx_len, 0);
                                        rx_len = remaining;
                                    }
                                    if rx_len == rx_buf.len() {
                                        error!("Serial RX buffer overflow, dropping data ({})", port_name);
                                        rx_len = 0;
                                    }
                                }
                                Err(e) => {
                                    error!("Serial Read Error ({}): {:?}", port_name, e);
                                    break;
                                }
                            }
                        }
                        // 2. 송신 (Tx) 처리
                        opt_req = tx_receiver.recv() => {
                            match opt_req {
                                Some(req) => {
                                    match postcard::to_slice_cobs(&req, &mut tx_buf) {
                                        Ok(data) => {
                                            if let Err(e) = writer.write_all(data).await {
                                                error!("Failed to write to serial ({}): {:?}", port_name, e);
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to serialize packet for {} using Postcard COBS: {:?}", port_name, e);
                                        }
                                    }
                                }
                                None => return, // tx_sender가 드롭되었으므로 태스크 완전 종료
                            }
                        }
                    }
                }
                warn!(
                    "Serial connection handler exited. Reconnecting in {} seconds.",
                    RECONNECT_DELAY_SECS
                );
                sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
            }
        });

        Ok(Self {
            tx_sender,
            rx_receiver,
            status_receiver,
        })
    }
}
