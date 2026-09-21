use host_utils::serial_comm::{ConnectionStatus, SerialComm};
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::app::{LINUX_CDC_ACM_PID, LINUX_CDC_ACM_VID};

pub struct UsbComm {
    tx: mpsc::Sender<CaptureRequestToRuntime>,
    rx: mpsc::Receiver<CaptureResponseFromRuntime>,
    status_rx: mpsc::Receiver<ConnectionStatus>,
    _handle: thread::JoinHandle<()>,
}

impl UsbComm {
    pub fn new(port_name: &str, baud_rate: u32) -> Result<Self, String> {
        let (ui_tx, thread_rx) = mpsc::channel();
        let (thread_tx, ui_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();

        let port_name = port_name.to_string();

        let handle = thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tokio runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                // SerialComm's new function spawns a task that handles connections,
                // so it won't fail here if the port is not immediately available.
                let comm = SerialComm::<CaptureRequestToRuntime, CaptureResponseFromRuntime>::new(
                    &port_name, baud_rate, Some((LINUX_CDC_ACM_VID, LINUX_CDC_ACM_PID)),
                )
                .expect("Failed to create SerialComm");

                let serial_tx = comm.tx_sender;
                let mut serial_rx = comm.rx_receiver;
                let mut serial_status_rx = comm.status_receiver;

                // Task to forward requests from the sync UI thread to the async serial task
                tokio::spawn(async move {
                    let forward_loop = tokio::task::spawn_blocking(move || {
                        for req in thread_rx.iter() {
                            match req {
                                CaptureRequestToRuntime::UpdateDisplay { .. } => {
                                    // 화면 업데이트 요청은 송신 큐가 가득 찬 경우 대기하지 않고 즉시 드롭하여 지연(Latency) 방지
                                    if let Err(e) = serial_tx.try_send(req) {
                                        match e {
                                            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                                                tracing::debug!("Serial TX queue full, dropping display update frame to prevent latency.");
                                            }
                                            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                                                tracing::error!(
                                                    "Serial channel closed. Stopping request forwarding."
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    // 중요 제어 명령(앱릿 런처, 와이파이 연결 등)은 무손실 전송을 위해 대기(블로킹) 전송
                                    if serial_tx.blocking_send(req).is_err() {
                                        tracing::error!(
                                            "Serial channel closed. Stopping request forwarding."
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                    });
                    if let Err(e) = forward_loop.await {
                        tracing::error!("Request forwarding task panicked: {}", e);
                    }
                });

                // Forward responses and status from the serial task to the sync UI thread
                loop {
                    tokio::select! {
                        Some(resp) = serial_rx.recv() => {
                            if thread_tx.send(resp).is_err() {
                                // UI thread's receiver was dropped.
                                tracing::info!("UI thread response channel disconnected. Stopping forwarding.");
                                break;
                            }
                        },
                        Some(status) = serial_status_rx.recv() => {
                            if status_tx.send(status).is_err() {
                                // UI thread's status receiver was dropped.
                                tracing::info!("UI thread status channel disconnected. Stopping status forwarding.");
                                // This is not a fatal error for the whole comm, so we don't break.
                            }
                        },
                        else => break, // Both channels closed.
                    }
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
        // Block for a reasonable amount of time for a response.
        // The original implementation was also blocking, but in a polling loop.
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
