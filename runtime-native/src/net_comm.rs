use anyhow::Result;
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use protocols::esp32::RequestToEsp32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tracing::{error, info, warn};

pub async fn start_net_comm(
    addr: &str,
    esp32_sender: Sender<RequestToEsp32>,
    applet_sender: UnboundedSender<String>,
    mirror_running: Arc<AtomicBool>,
    shared_console_pins: Option<Arc<std::sync::Mutex<Vec<u8>>>>,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP NetComm server listening on {}", addr);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, peer_addr)) => {
                    info!("TCP client connected from {}", peer_addr);
                    let esp32_tx = esp32_sender.clone();
                    let applet_tx = applet_sender.clone();
                    let mirror_running_conn = mirror_running.clone();
                    let shared_console_pins_conn = shared_console_pins.clone();

                    tokio::spawn(async move {
                        let (mut reader, mut writer) = stream.split();
                        let mut rx_buf = vec![0u8; 8192];
                        let mut tx_buf = vec![0u8; 4096];
                        let mut rx_len = 0;
                        let mut prev_buffer: Vec<u8> = Vec::new();

                        loop {
                            match reader.read(&mut rx_buf[rx_len..]).await {
                                Ok(0) => {
                                    info!("TCP client {} disconnected", peer_addr);
                                    break;
                                }
                                Ok(n) => {
                                    rx_len += n;
                                    while let Some(end_idx) =
                                        rx_buf[..rx_len].iter().position(|&b| b == 0x00)
                                    {
                                        if end_idx > 0 {
                                            match postcard::from_bytes_cobs::<CaptureRequestToRuntime>(
                                                &mut rx_buf[..=end_idx],
                                            ) {
                                                Ok(req) => {
                                                    // 비동기 처리 및 소유권 이동을 통한 prev_buffer 갱신
                                                    let shared_pins =
                                                        shared_console_pins_conn.clone();
                                                    let (response, updated_buffer) =
                                                        handle_request(
                                                            req,
                                                            prev_buffer,
                                                            &esp32_tx,
                                                            &applet_tx,
                                                            &mirror_running_conn,
                                                            shared_pins,
                                                        )
                                                        .await;
                                                    prev_buffer = updated_buffer;

                                                    match postcard::to_slice_cobs(
                                                        &response,
                                                        &mut tx_buf,
                                                    ) {
                                                        Ok(data) => {
                                                            if let Err(e) =
                                                                writer.write_all(data).await
                                                            {
                                                                error!(
                                                                    "Failed to write to TCP {}: {:?}",
                                                                    peer_addr, e
                                                                );
                                                                break;
                                                            }
                                                        }
                                                        Err(e) => warn!(
                                                            "Postcard Serialize Error to {}: {:?}",
                                                            peer_addr, e
                                                        ),
                                                    }
                                                }
                                                Err(e) => warn!(
                                                    "Postcard Deserialize Error from {}: {:?}",
                                                    peer_addr, e
                                                ),
                                            }
                                        }
                                        let remaining = rx_len - (end_idx + 1);
                                        rx_buf.copy_within(end_idx + 1..rx_len, 0);
                                        rx_len = remaining;
                                    }
                                    if rx_len == rx_buf.len() {
                                        error!(
                                            "TCP RX buffer overflow from {}, dropping data",
                                            peer_addr
                                        );
                                        rx_len = 0;
                                    }
                                }
                                Err(e) => {
                                    error!("TCP Read Error from {}: {:?}", peer_addr, e);
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept TCP connection: {:?}", e);
                }
            }
        }
    });

    Ok(())
}

async fn handle_request(
    req: CaptureRequestToRuntime,
    mut prev_buffer: Vec<u8>,
    esp32_sender: &Sender<RequestToEsp32>,
    applet_sender: &UnboundedSender<String>,
    mirror_running: &Arc<AtomicBool>,
    shared_console_pins: Option<Arc<std::sync::Mutex<Vec<u8>>>>,
) -> (CaptureResponseFromRuntime, Vec<u8>) {
    match req {
        CaptureRequestToRuntime::LaunchApplet { name } => {
            prev_buffer.clear();
            mirror_running.store(name == "mirror", Ordering::SeqCst);
            if let Err(e) = applet_sender.send(name) {
                error!("Failed to send applet name through channel: {}", e);
            }
            (CaptureResponseFromRuntime::Ok, prev_buffer)
        }
        CaptureRequestToRuntime::UpdateDisplay {
            width,
            height,
            data,
            bits_per_pixel,
        } => {
            if !mirror_running.load(Ordering::SeqCst) {
                return (CaptureResponseFromRuntime::Paused, prev_buffer);
            }

            // CPU 집약적인 작업을 spawn_blocking으로 분리
            let (req_opt, current_buffer, returned_prev) = tokio::task::spawn_blocking(move || {
                let current_buffer =
                    crate::hal::display::unpack_frame(&data, width, height, bits_per_pixel);

                if let Some(pins) = shared_console_pins {
                    match pins.lock() {
                        Ok(mut p) => {
                            if p.len() == current_buffer.len() {
                                p.copy_from_slice(&current_buffer);
                            }
                        }
                        Err(_) => tracing::error!("shared_console_pins mutex is poisoned"),
                    }
                    return (None, current_buffer.clone(), current_buffer);
                }

                if prev_buffer.len() != current_buffer.len() {
                    prev_buffer = vec![0; current_buffer.len()];
                }

                let prev_intensities: Vec<sdk::api::display::Intensity> = prev_buffer
                    .iter()
                    .map(|&v| {
                        if bits_per_pixel == 4 {
                            let val = v / 17;
                            if val >= 8 {
                                sdk::api::display::Intensity::new_blink(
                                    ((val - 8) as u16 * 255 / 7) as u8,
                                )
                            } else {
                                sdk::api::display::Intensity::new((val as u16 * 255 / 7) as u8)
                            }
                        } else {
                            sdk::api::display::Intensity::new(v)
                        }
                    })
                    .collect();
                let current_intensities: Vec<sdk::api::display::Intensity> = current_buffer
                    .iter()
                    .map(|&v| {
                        if bits_per_pixel == 4 {
                            let val = v / 17;
                            if val >= 8 {
                                sdk::api::display::Intensity::new_blink(
                                    ((val - 8) as u16 * 255 / 7) as u8,
                                )
                            } else {
                                sdk::api::display::Intensity::new((val as u16 * 255 / 7) as u8)
                            }
                        } else {
                            sdk::api::display::Intensity::new(v)
                        }
                    })
                    .collect();

                let diff_data = crate::hal::display::calculate_diff(
                    &prev_intensities,
                    &current_intensities,
                    width,
                    height,
                    bits_per_pixel,
                );

                let is_full_frame = diff_data.len() * 2 >= data.len();

                let req_to_esp32 = if diff_data.is_empty() {
                    None
                } else if !is_full_frame {
                    Some(RequestToEsp32::UpdateDisplayByDiff { data: diff_data })
                } else {
                    Some(RequestToEsp32::UpdateDisplay {
                        width,
                        height,
                        data,
                    })
                };

                (req_to_esp32, current_buffer, prev_buffer)
            })
            .await
            .unwrap_or_else(|e| {
                error!("spawn_blocking failed: {}", e);
                (None, Vec::new(), Vec::new())
            });

            let (response, next_prev) = if let Some(r) = req_opt {
                match esp32_sender.try_send(r) {
                    Ok(()) => (CaptureResponseFromRuntime::Ok, current_buffer),
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        tracing::warn!(
                            "ESP32 request queue is full, dropping frame to match UART baudrate"
                        );
                        (CaptureResponseFromRuntime::Ok, returned_prev)
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        tracing::error!(
                            "Failed to send update display request to esp32: channel closed"
                        );
                        (CaptureResponseFromRuntime::Error, returned_prev)
                    }
                }
            } else {
                (CaptureResponseFromRuntime::Ok, returned_prev)
            };
            (response, next_prev)
        }
        CaptureRequestToRuntime::ScanWifi => (CaptureResponseFromRuntime::Error, prev_buffer),
        CaptureRequestToRuntime::ConnectWifi { .. } => {
            (CaptureResponseFromRuntime::Error, prev_buffer)
        }
        CaptureRequestToRuntime::GetNetworkAddresses => {
            let addresses = get_connected_network_addresses().await;
            (
                CaptureResponseFromRuntime::NetworkAddressesResult(addresses),
                prev_buffer,
            )
        }
        CaptureRequestToRuntime::CancelWifiOperation => {
            (CaptureResponseFromRuntime::Ok, prev_buffer)
        }
        CaptureRequestToRuntime::DisconnectWifi { .. } => {
            (CaptureResponseFromRuntime::Error, prev_buffer)
        }
    }
}

pub async fn get_connected_network_addresses() -> Vec<String> {
    let mut ips = Vec::new();

    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        for iface in interfaces {
            if !iface.is_loopback() {
                ips.push(iface.ip().to_string());
            }
        }
    }

    ips
}
