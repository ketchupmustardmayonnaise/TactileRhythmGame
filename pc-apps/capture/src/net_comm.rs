use host_utils::serial_comm::ConnectionStatus;
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;

pub struct NetComm {
    tx: mpsc::Sender<CaptureRequestToRuntime>,
    rx: mpsc::Receiver<CaptureResponseFromRuntime>,
    status_rx: mpsc::Receiver<ConnectionStatus>,
    _handle: thread::JoinHandle<()>,
}

impl NetComm {
    pub fn new(addr: &str) -> Result<Self, String> {
        let (ui_tx, thread_rx) = mpsc::channel::<CaptureRequestToRuntime>();
        let (thread_tx, ui_rx) = mpsc::channel::<CaptureResponseFromRuntime>();
        let (status_tx, status_rx) = mpsc::channel::<ConnectionStatus>();

        let addr = addr.to_string();

        let handle = thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tokio runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let (async_tx, mut async_rx) =
                    tokio::sync::mpsc::channel::<CaptureRequestToRuntime>(100);

                // Forward from std::sync::mpsc to tokio::sync::mpsc
                tokio::spawn(async move {
                    let forward_loop = tokio::task::spawn_blocking(move || {
                        for req in thread_rx.iter() {
                            match async_tx.try_send(req) {
                                Ok(_) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(failed_req)) => {
                                    // 큐가 가득 찬 상태(네트워크 단절 등)일 때 오래된 디스플레이 프레임은 드롭하여 지연을 방지합니다.
                                    // 단, 앱 론칭 등 중요한 명령 패킷은 블로킹을 감수하더라도 전송하도록 처리합니다.
                                    if let CaptureRequestToRuntime::LaunchApplet { .. } = failed_req {
                                        let _ = async_tx.blocking_send(failed_req);
                                    }
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    tracing::error!("Async channel closed. Stopping request forwarding.");
                                    break;
                                }
                            }
                        }
                    });
                    if let Err(e) = forward_loop.await {
                        tracing::error!("Request forwarding task panicked: {}", e);
                    }
                });

                loop {
                    if status_tx.send(ConnectionStatus::Disconnected).is_err() {
                        tracing::info!("UI thread status channel disconnected.");
                        break;
                    }

                    tracing::info!("Connecting to TCP server at {}", addr);

                    // 타임아웃을 걸어 무한정 블로킹되는 현상을 방지합니다.
                    let mut stream = match tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(&addr)).await {
                        Ok(Ok(s)) => {
                            tracing::info!("TCP connected to {}", addr);
                            if status_tx.send(ConnectionStatus::Connected { port: None }).is_err() {
                                break;
                            }
                            // 연결 재개 시, 오프라인 상태일 때 쌓였던 오래된 화면 업데이트 요청을 비워내어 지연(Lag)을 방지합니다.
                            while async_rx.try_recv().is_ok() {}
                            s
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Failed to connect to {}: {}. Retrying in 1s.", addr, e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                        Err(_) => {
                            tracing::warn!("Connection timeout to {}. Retrying...", addr);
                            continue;
                        }
                    };

                    let (mut reader, mut writer) = stream.split();
                    let mut rx_buf = vec![0u8; 8192];
                    let mut tx_buf = vec![0u8; 4096];
                    let mut rx_len = 0;

                    loop {
                        tokio::select! {
                            res = reader.read(&mut rx_buf[rx_len..]) => {
                                match res {
                                    Ok(0) => {
                                        tracing::info!("TCP stream closed ({})", addr);
                                        break;
                                    }
                                    Ok(n) => {
                                        rx_len += n;
                                        while let Some(end_idx) = rx_buf[..rx_len].iter().position(|&b| b == 0x00) {
                                            if end_idx > 0 {
                                                match postcard::from_bytes_cobs::<CaptureResponseFromRuntime>(&mut rx_buf[..=end_idx]) {
                                                    Ok(resp) => {
                                                        if thread_tx.send(resp).is_err() {
                                                            tracing::info!("UI thread response channel disconnected.");
                                                            return;
                                                        }
                                                    }
                                                    Err(e) => tracing::warn!("Postcard Deserialize Error: {:?}", e),
                                                }
                                            }
                                            let remaining = rx_len - (end_idx + 1);
                                            rx_buf.copy_within(end_idx + 1..rx_len, 0);
                                            rx_len = remaining;
                                        }
                                        if rx_len == rx_buf.len() {
                                            tracing::error!("TCP RX buffer overflow, dropping data");
                                            rx_len = 0;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("TCP Read Error: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            opt_req = async_rx.recv() => {
                                match opt_req {
                                    Some(req) => {
                                        match postcard::to_slice_cobs(&req, &mut tx_buf) {
                                            Ok(data) => {
                                                if let Err(e) = writer.write_all(data).await {
                                                    tracing::error!("Failed to write to TCP: {:?}", e);
                                                    break;
                                                }
                                            }
                                            Err(e) => tracing::error!("Postcard Serialize Error: {:?}", e),
                                        }
                                    }
                                    None => return, // UI thread dropped sender
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
        });

        Ok(Self {
            tx: ui_tx,
            rx: ui_rx,
            status_rx,
            _handle: handle,
        })
    }

    pub fn send_request(&mut self, req: &CaptureRequestToRuntime) -> std::io::Result<()> {
        self.tx
            .send(req.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()))
    }

    pub fn try_receive_status(&mut self) -> Option<ConnectionStatus> {
        self.status_rx.try_recv().ok()
    }

    pub fn try_receive_response(&mut self) -> Option<std::io::Result<CaptureResponseFromRuntime>> {
        match self.rx.try_recv() {
            Ok(resp) => {
                tracing::debug!("Received Response: {:?}", resp);
                if let CaptureResponseFromRuntime::Error = resp {
                    return Some(Err(std::io::Error::other(
                        "Received error response from runtime",
                    )));
                }
                Some(Ok(resp))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Communication thread disconnected",
            ))),
        }
    }

    pub fn receive_response(&mut self) -> std::io::Result<CaptureResponseFromRuntime> {
        match self.rx.recv_timeout(Duration::from_millis(500)) {
            Ok(resp) => {
                tracing::debug!("Received Response: {:?}", resp);
                if let CaptureResponseFromRuntime::Error = resp {
                    return Err(std::io::Error::other(
                        "Received error response from runtime",
                    ));
                }
                Ok(resp)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Response timed out",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Communication thread disconnected",
            )),
        }
    }
}
