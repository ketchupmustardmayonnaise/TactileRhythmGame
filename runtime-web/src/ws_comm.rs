use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

const TX_BUF_SIZE: usize = 1024;

pub struct WsClosures {
    pub onmessage: Closure<dyn FnMut(MessageEvent)>,
    pub onopen: Closure<dyn FnMut()>,
    pub onerror: Closure<dyn FnMut(ErrorEvent)>,
    pub onclose: Closure<dyn FnMut(web_sys::Event)>,
}

/// WebSocket 연결을 시도하고 캡처 프로그램의 요청을 수신합니다.
pub fn start_websocket_comm(
    url: &str,
    on_launch_applet: impl Fn(String) + 'static,
    on_update_display: impl Fn(u32, u32, Vec<u8>, u8) -> bool + 'static,
    on_connect: impl Fn() + 'static,
    on_disconnect: impl Fn() + 'static,
) -> Result<(WebSocket, WsClosures), JsValue> {
    let ws = WebSocket::new(url)?;

    // 바이너리 데이터 형식 지정 (ArrayBuffer로 수신)
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let ws_clone = ws.clone();

    // 1. 메시지 수신 (onmessage)
    let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
        if let Ok(ab) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
            let array = js_sys::Uint8Array::new(&ab);
            let mut bytes = array.to_vec();

            // PC에서 보낸 COBS 형식의 Postcard 데이터를 디코딩합니다.
            if let Ok(req) = postcard::from_bytes_cobs::<CaptureRequestToRuntime>(&mut bytes) {
                match req {
                    CaptureRequestToRuntime::LaunchApplet { name } => {
                        leptos::logging::log!("WebSocket: Launching applet {}", name);
                        on_launch_applet(name);
                        send_response(&ws_clone, CaptureResponseFromRuntime::Ok);
                    }
                    CaptureRequestToRuntime::UpdateDisplay {
                        width,
                        height,
                        data,
                        bits_per_pixel,
                    } => {
                        if on_update_display(width.into(), height.into(), data, bits_per_pixel) {
                            send_response(&ws_clone, CaptureResponseFromRuntime::Ok);
                        } else {
                            send_response(&ws_clone, CaptureResponseFromRuntime::Paused);
                        }
                    }
                    CaptureRequestToRuntime::ScanWifi => {
                        leptos::logging::warn!(
                            "WebSocket: ScanWifi request ignored (only supported via serial)"
                        );
                        send_response(&ws_clone, CaptureResponseFromRuntime::Error);
                    }
                    CaptureRequestToRuntime::ConnectWifi { .. } => {
                        leptos::logging::warn!(
                            "WebSocket: ConnectWifi request ignored (only supported via serial)"
                        );
                        send_response(&ws_clone, CaptureResponseFromRuntime::Error);
                    }
                    CaptureRequestToRuntime::DisconnectWifi { .. } => {
                        leptos::logging::warn!(
                            "WebSocket: DisconnectWifi request ignored (only supported via serial)"
                        );
                        send_response(&ws_clone, CaptureResponseFromRuntime::Error);
                    }
                    CaptureRequestToRuntime::GetNetworkAddresses => {
                        leptos::logging::warn!(
                            "WebSocket: GetNetworkAddresses request ignored (only supported on native)"
                        );
                        send_response(&ws_clone, CaptureResponseFromRuntime::Error);
                    }
                    CaptureRequestToRuntime::CancelWifiOperation => {
                        leptos::logging::warn!(
                            "WebSocket: CancelWifiOperation request ignored (only supported on native)"
                        );
                        send_response(&ws_clone, CaptureResponseFromRuntime::Error);
                    }
                }
            } else {
                leptos::logging::warn!("WebSocket: Failed to deserialize message");
            }
        }
    });
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));

    // 2. 연결 성공 (onopen)
    let onopen_callback = Closure::<dyn FnMut()>::new(move || {
        leptos::logging::log!("WebSocket Connected to PC Capture App");
        on_connect();
    });
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));

    // 3. 에러 (onerror)
    let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
        leptos::logging::error!("WebSocket Error: {:?}", e);
    });
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));

    // 4. 연결 끊김 (onclose)
    let onclose_callback = Closure::<dyn FnMut(_)>::new(move |e: web_sys::Event| {
        leptos::logging::warn!("WebSocket Closed: {:?}", e);
        on_disconnect();
    });
    ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));

    Ok((
        ws,
        WsClosures {
            onmessage: onmessage_callback,
            onopen: onopen_callback,
            onerror: onerror_callback,
            onclose: onclose_callback,
        },
    ))
}

fn send_response(ws: &WebSocket, response: CaptureResponseFromRuntime) {
    let mut tx_buf = [0u8; TX_BUF_SIZE];
    match postcard::to_slice_cobs(&response, &mut tx_buf) {
        Ok(data) => {
            let array = js_sys::Uint8Array::from(&*data);
            if let Err(e) = ws.send_with_array_buffer(&array.buffer()) {
                leptos::logging::error!("Failed to send WebSocket response: {:?}", e);
            }
        }
        Err(e) => leptos::logging::error!("Postcard Serialize Error: {:?}", e),
    }
}
