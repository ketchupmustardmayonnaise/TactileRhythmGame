use futures_util::{SinkExt, StreamExt};
use host_utils::serial_comm::ConnectionStatus;
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

pub struct WsComm {
    tx: mpsc::Sender<CaptureRequestToRuntime>,
    rx: mpsc::Receiver<CaptureResponseFromRuntime>,
    status_rx: mpsc::Receiver<ConnectionStatus>,
    _handle: thread::JoinHandle<()>,
}

impl WsComm {
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

                // 동기(UI) 채널에서 비동기(Tokio) 채널로 데이터 전달
                tokio::spawn(async move {
                    let forward_loop = tokio::task::spawn_blocking(move || {
                        for req in thread_rx.iter() {
                            match async_tx.try_send(req) {
                                Ok(_) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(failed_req)) => {
                                    if let CaptureRequestToRuntime::LaunchApplet { .. } = failed_req
                                    {
                                        let _ = async_tx.blocking_send(failed_req);
                                    }
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    tracing::error!(
                                        "Async channel closed. Stopping request forwarding."
                                    );
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

                    tracing::info!("Starting WebSocket server on {}", addr);
                    let listener = match TcpListener::bind(&addr).await {
                        Ok(l) => l,
                        Err(e) => {
                            tracing::warn!("Failed to bind to {}: {}. Retrying in 1s.", addr, e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    };

                    // 클라이언트(runtime-web) 접속 대기
                    match listener.accept().await {
                        Ok((stream, peer_addr)) => {
                            tracing::info!("TCP connected from {}", peer_addr);

                            // WebSocket 핸드쉐이크
                            let ws_stream = match accept_async(stream).await {
                                Ok(ws) => ws,
                                Err(e) => {
                                    tracing::error!("Error during websocket handshake: {}", e);
                                    continue;
                                }
                            };

                            tracing::info!("WebSocket connection established with {}", peer_addr);
                            if status_tx.send(ConnectionStatus::Connected { port: None }).is_err() {
                                break;
                            }

                            // 연결 재개 시 쌓여있던 이전 프레임 삭제
                            while async_rx.try_recv().is_ok() {}

                            let (mut write, mut read) = ws_stream.split();
                            let mut tx_buf = vec![0u8; 4096];

                            loop {
                                tokio::select! {
                                    // 1. WebSocket 수신 처리
                                    msg = read.next() => {
                                        match msg {
                                            Some(Ok(Message::Binary(data))) => {
                                                let mut data_vec = data.to_vec();
                                                // 런타임에서 COBS 형식으로 전송했다고 가정합니다.
                                                match postcard::from_bytes_cobs::<CaptureResponseFromRuntime>(&mut data_vec) {
                                                    Ok(resp) => {
                                                        if thread_tx.send(resp).is_err() {
                                                            tracing::info!("UI thread response channel disconnected.");
                                                            return;
                                                        }
                                                    }
                                                    Err(e) => tracing::warn!("Postcard Deserialize Error: {:?}", e),
                                                }
                                            }
                                            Some(Ok(Message::Close(_))) | None => {
                                                tracing::info!("WebSocket stream closed ({})", peer_addr);
                                                break;
                                            }
                                            Some(Err(e)) => {
                                                tracing::error!("WebSocket Read Error: {:?}", e);
                                                break;
                                            }
                                            _ => {} // Text 메시지 등은 무시
                                        }
                                    }
                                    // 2. WebSocket 송신 처리 (화면 캡처 데이터)
                                    opt_req = async_rx.recv() => {
                                        match opt_req {
                                            Some(req) => {
                                                match postcard::to_slice_cobs(&req, &mut tx_buf) {
                                                    Ok(data) => {
                                                        if let Err(e) = write.send(Message::Binary(data.to_vec().into())).await {
                                                            tracing::error!("Failed to write to WebSocket: {:?}", e);
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
                        }
                        Err(e) => tracing::warn!("Accept failed: {:?}", e),
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
