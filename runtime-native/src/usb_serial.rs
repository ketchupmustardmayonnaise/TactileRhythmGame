use anyhow::Result;
use host_utils::serial_comm::SerialComm;
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use protocols::esp32::RequestToEsp32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tracing::{error, info};

pub async fn start_usb_serial(
    port_name: &str,
    baud_rate: u32,
    esp32_sender: Sender<RequestToEsp32>,
    applet_sender: UnboundedSender<String>,
    mirror_running: Arc<AtomicBool>,
    shared_console_pins: Option<Arc<std::sync::Mutex<Vec<u8>>>>,
) -> Result<()> {
    // SerialComm 제네릭을 사용하여 통신 링크만 받아옵니다.
    let SerialComm {
        tx_sender: tx,
        rx_receiver: mut rx,
        status_receiver,
    } = SerialComm::<CaptureResponseFromRuntime, CaptureRequestToRuntime>::new(
        port_name, baud_rate, None,
    )?;

    // USB 연결 상태(ConnectionStatus) 변경 이벤트를 감지하여 로깅하는 태스크 추가
    tokio::spawn(async move {
        let mut receiver = status_receiver;
        while let Some(status) = receiver.recv().await {
            tracing::info!("USB Serial Connection Status: {:?}", status);
        }
    });

    // 실제 비즈니스 로직(라우팅 및 응답)을 별도 태스크로 분리하여 동작시킵니다.
    tokio::spawn(async move {
        let mut prev_buffer: Vec<u8> = Vec::new();
        let mut active_wifi_task: Option<tokio::task::JoinHandle<()>> = None;
        while let Some(req) = rx.recv().await {
            match req {
                CaptureRequestToRuntime::LaunchApplet { name } => {
                    prev_buffer.clear(); // 앱 교체 시 화면 초기화 상태이므로 이전 버퍼를 비워 갱신 유도
                    mirror_running.store(name == "mirror", Ordering::SeqCst);
                    if let Err(e) = applet_sender.send(name) {
                        error!("Failed to send applet name through channel: {}", e);
                    }
                    let _ = tx.send(CaptureResponseFromRuntime::Ok).await;
                }
                CaptureRequestToRuntime::UpdateDisplay {
                    width,
                    height,
                    data,
                    bits_per_pixel,
                } => {
                    if !mirror_running.load(Ordering::SeqCst) {
                        let _ = tx.send(CaptureResponseFromRuntime::Paused).await;
                        continue;
                    }

                    // CPU 집약적인 작업을 spawn_blocking으로 분리
                    let shared_pins = shared_console_pins.clone();
                    let (req_opt, current_buffer, returned_prev) =
                        tokio::task::spawn_blocking(move || {
                            let current_buffer = crate::hal::display::unpack_frame(
                                &data,
                                width,
                                height,
                                bits_per_pixel,
                            );

                            if let Some(pins) = shared_pins {
                                match pins.lock() {
                                    Ok(mut p) => {
                                        if p.len() == current_buffer.len() {
                                            p.copy_from_slice(&current_buffer);
                                        }
                                    }
                                    Err(_) => error!("shared_pins mutex is poisoned"),
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
                                            sdk::api::display::Intensity::new(
                                                (val as u16 * 255 / 7) as u8,
                                            )
                                        }
                                    } else {
                                        sdk::api::display::Intensity::new(v)
                                    }
                                })
                                .collect();
                            let current_intensities: Vec<sdk::api::display::Intensity> =
                                current_buffer
                                    .iter()
                                    .map(|&v| {
                                        if bits_per_pixel == 4 {
                                            let val = v / 17;
                                            if val >= 8 {
                                                sdk::api::display::Intensity::new_blink(
                                                    ((val - 8) as u16 * 255 / 7) as u8,
                                                )
                                            } else {
                                                sdk::api::display::Intensity::new(
                                                    (val as u16 * 255 / 7) as u8,
                                                )
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

                    let response = if let Some(r) = req_opt {
                        match esp32_sender.try_send(r) {
                            Ok(()) => {
                                prev_buffer = current_buffer;
                                CaptureResponseFromRuntime::Ok
                            }
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                tracing::warn!(
                                    "ESP32 request queue is full, dropping frame to match UART baudrate"
                                );
                                prev_buffer = returned_prev;
                                CaptureResponseFromRuntime::Ok
                            }
                            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                tracing::error!(
                                    "Failed to send update display request to esp32: channel closed"
                                );
                                prev_buffer = returned_prev;
                                CaptureResponseFromRuntime::Error
                            }
                        }
                    } else {
                        prev_buffer = returned_prev;
                        CaptureResponseFromRuntime::Ok
                    };

                    let _ = tx.send(response).await;
                }
                CaptureRequestToRuntime::ScanWifi => {
                    info!("Received ScanWifi request");
                    if let Some(ref task) = active_wifi_task {
                        task.abort();
                    }
                    let tx_clone = tx.clone();
                    active_wifi_task = Some(tokio::spawn(async move {
                        let networks = scan_wifi_networks().await;
                        let _ = tx_clone
                            .send(CaptureResponseFromRuntime::WifiScanResult(networks))
                            .await;
                    }));
                }
                CaptureRequestToRuntime::ConnectWifi { ssid, passphrase } => {
                    info!("Received ConnectWifi request for SSID: {}", ssid);
                    if let Some(ref task) = active_wifi_task {
                        task.abort();
                    }
                    let tx_clone = tx.clone();
                    active_wifi_task = Some(tokio::spawn(async move {
                        let result = connect_wifi(&ssid, &passphrase).await;
                        let response: CaptureResponseFromRuntime = match result {
                            Ok(msg) => CaptureResponseFromRuntime::WifiConnectResult {
                                success: true,
                                message: msg,
                            },
                            Err(e) => CaptureResponseFromRuntime::WifiConnectResult {
                                success: false,
                                message: e.to_string(),
                            },
                        };
                        let _ = tx_clone.send(response).await;
                    }));
                }
                CaptureRequestToRuntime::DisconnectWifi { ssid } => {
                    info!("Received DisconnectWifi request for SSID: {}", ssid);
                    if let Some(ref task) = active_wifi_task {
                        task.abort();
                    }
                    let tx_clone = tx.clone();
                    active_wifi_task = Some(tokio::spawn(async move {
                        let result = disconnect_wifi(&ssid).await;
                        let response = match result {
                            Ok(_msg) => CaptureResponseFromRuntime::WifiDisconnectResult {
                                success: true,
                                message: "Disconnected successfully".to_string(),
                            },
                            Err(e) => CaptureResponseFromRuntime::WifiDisconnectResult {
                                success: false,
                                message: format!("Disconnect failed: {}", e),
                            },
                        };
                        let _ = tx_clone.send(response).await;
                    }));
                }
                CaptureRequestToRuntime::GetNetworkAddresses => {
                    info!("Received GetNetworkAddresses request");
                    if let Some(ref task) = active_wifi_task {
                        task.abort();
                    }
                    let tx_clone = tx.clone();
                    active_wifi_task = Some(tokio::spawn(async move {
                        let addresses = crate::net_comm::get_connected_network_addresses().await;
                        let _ = tx_clone
                            .send(CaptureResponseFromRuntime::NetworkAddressesResult(
                                addresses,
                            ))
                            .await;
                    }));
                }
                CaptureRequestToRuntime::CancelWifiOperation => {
                    info!("Received CancelWifiOperation request");
                    if let Some(task) = active_wifi_task.take() {
                        task.abort();
                        info!("Active network/wifi task aborted.");
                    }
                    let _ = tx.send(CaptureResponseFromRuntime::Ok).await;
                }
            }
        }
        info!("USB Serial application logic terminated.");
    });

    Ok(())
}

async fn scan_wifi_networks() -> Vec<protocols::capture::WifiNetwork> {
    info!("Starting Wi-Fi network scan via 'sudo nmcli'...");
    // TODO: sudo 사용은 임시적입니다. 장치에서는 sudo 그룹에 NOPASSWD가 적용되어 있습니다.
    let output = match tokio::process::Command::new("sudo")
        .args([
            "nmcli",
            "-t",
            "-f",
            "SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            "yes",
        ])
        .output()
        .await
    {
        Ok(out) => out,
        Err(e) => {
            error!("Failed to execute 'sudo nmcli': {}", e);
            return Vec::new();
        }
    };

    if !output.status.success() {
        error!(
            "'sudo nmcli' exited with error status: {}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return Vec::new();
    }

    info!("Wi-Fi scan command completed successfully. Parsing output...");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let networks = parse_nmcli_wifi_list(&stdout);
    info!(
        "Wi-Fi scan found {} unique networks after de-duplication.",
        networks.len()
    );
    networks
}

fn parse_nmcli_wifi_list(stdout: &str) -> Vec<protocols::capture::WifiNetwork> {
    use std::collections::HashMap;

    let mut networks_map: HashMap<String, protocols::capture::WifiNetwork> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = split_nmcli_line(line);
        if fields.len() >= 3 {
            let ssid = fields[0].trim().to_string();
            if ssid.is_empty() {
                continue; // Skip hidden SSIDs
            }
            let signal_str = fields[1].trim();
            let signal: i32 = signal_str.parse().unwrap_or(0);
            let security = fields[2].trim().to_string();

            // Only insert or update if signal is stronger
            if let Some(existing) = networks_map.get(&ssid) {
                if signal > existing.signal {
                    info!(
                        "Wi-Fi Scan: SSID='{}' (updating signal from {}% to {}%)",
                        ssid, existing.signal, signal
                    );
                    networks_map.insert(
                        ssid.clone(),
                        protocols::capture::WifiNetwork {
                            ssid: ssid.clone(),
                            signal,
                            security,
                        },
                    );
                }
            } else {
                info!(
                    "Wi-Fi Scan: Found SSID='{}', Signal={}%, Sec='{}'",
                    ssid, signal, security
                );
                networks_map.insert(
                    ssid.clone(),
                    protocols::capture::WifiNetwork {
                        ssid: ssid.clone(),
                        signal,
                        security,
                    },
                );
            }
        }
    }

    let mut sorted_networks: Vec<_> = networks_map.into_values().collect();
    // Sort by signal strength descending
    sorted_networks.sort_by_key(|b| std::cmp::Reverse(b.signal));
    sorted_networks
}

fn split_nmcli_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next_c) = chars.peek()
            && (next_c == ':' || next_c == '\\')
        {
            current.push(next_c);
            chars.next(); // consume the escaped char
            continue;
        }
        if c == ':' {
            fields.push(current);
            current = String::new();
        } else {
            current.push(c);
        }
    }
    fields.push(current);
    fields
}

async fn connect_wifi(ssid: &str, passphrase: &str) -> Result<String, anyhow::Error> {
    info!("Attempting to connect to Wi-Fi SSID: '{}'...", ssid);

    // Deleting any existing connection profile to prevent "802-11-wireless-security.key-mgmt: property is missing" errors on NetworkManager
    // TODO: sudo 사용은 임시적입니다. 장치에서는 sudo 그룹에 NOPASSWD가 적용되어 있습니다.
    let delete_result = tokio::process::Command::new("sudo")
        .args(["nmcli", "connection", "delete", ssid])
        .output()
        .await;
    match delete_result {
        Ok(out) if out.status.success() => {
            info!("Deleted existing Wi-Fi connection profile for '{}'", ssid);
        }
        Ok(out) => {
            let stderr_str = String::from_utf8_lossy(&out.stderr).trim().to_string();
            info!(
                "No existing connection profile deleted or delete failed (normal if connection didn't exist): {}",
                stderr_str
            );
        }
        Err(e) => {
            info!("Failed to attempt connection profile deletion: {}", e);
        }
    }

    // TODO: sudo 사용은 임시적입니다. 장치에서는 sudo 그룹에 NOPASSWD가 적용되어 있습니다.
    let mut cmd = tokio::process::Command::new("sudo");
    let output = if !passphrase.is_empty() {
        info!(
            "Executing: sudo nmcli --ask device wifi connect '{}' (password provided via stdin)",
            ssid
        );
        cmd.arg("nmcli")
            .arg("--ask")
            .arg("device")
            .arg("wifi")
            .arg("connect")
            .arg(ssid)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(format!("{}\n", passphrase).as_bytes())
                .await?;
        }
        child.wait_with_output().await?
    } else {
        info!(
            "Executing: sudo nmcli device wifi connect '{}' (open network)",
            ssid
        );
        cmd.arg("nmcli")
            .arg("device")
            .arg("wifi")
            .arg("connect")
            .arg(ssid);

        cmd.output().await?
    };

    if output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!("Wi-Fi connection successful: {}", stdout_str);
        Ok(stdout_str)
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let err_msg = if !stderr_str.is_empty() {
            stderr_str
        } else if !stdout_str.is_empty() {
            stdout_str
        } else {
            "Unknown nmcli error".to_string()
        };
        error!("Wi-Fi connection failed: {}", err_msg);
        Err(anyhow::anyhow!("{}", err_msg))
    }
}

async fn disconnect_wifi(ssid: &str) -> Result<String, anyhow::Error> {
    info!("Attempting to disconnect from Wi-Fi SSID: '{}'...", ssid);

    // TODO: sudo 사용은 임시적입니다. 장치에서는 sudo 그룹에 NOPASSWD가 적용되어 있습니다.
    let mut cmd = tokio::process::Command::new("sudo");
    cmd.arg("nmcli").arg("connection").arg("down").arg(ssid);

    let output = cmd.output().await?;

    if output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!("Wi-Fi disconnect successful: {}", stdout_str);
        Ok(stdout_str)
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let err_msg = if !stderr_str.is_empty() {
            stderr_str
        } else if !stdout_str.is_empty() {
            stdout_str
        } else {
            "Unknown nmcli error".to_string()
        };
        error!("Wi-Fi disconnect failed: {}", err_msg);
        Err(anyhow::anyhow!("{}", err_msg))
    }
}
