use crate::host::HostState;
use anyhow::Result;
use std::sync::atomic::{AtomicU32, Ordering};
use wasmtime::{Caller, Extern, Linker};

static NEXT_REQUEST_ID: AtomicU32 = AtomicU32::new(1);

/// WASM 모듈에서 호출할 수 있는 네트워크 호스트 함수들을 링커에 추가합니다.
pub fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    let fetch_async_handler = |mut caller: Caller<'_, HostState>,
                               method_ptr: u32,
                               method_len: u32,
                               url_ptr: u32,
                               url_len: u32,
                               headers_ptr: u32,
                               headers_len: u32,
                               body_ptr: u32,
                               body_len: u32|
     -> i32 {
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => {
                tracing::error!("WASM 모듈에서 memory 익스포트를 가져오는데 실패했습니다");
                return -1;
            }
        };

        let data = mem.data(&caller);

        let method = match data.get(method_ptr as usize..(method_ptr + method_len) as usize) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    tracing::error!("유효하지 않은 HTTP 메서드 UTF-8 문자열");
                    return -2;
                }
            },
            None => return -2,
        };

        let url = match data.get(url_ptr as usize..(url_ptr + url_len) as usize) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    tracing::error!("유효하지 않은 URL UTF-8 문자열");
                    return -3;
                }
            },
            None => return -3,
        };

        let headers_bytes =
            match data.get(headers_ptr as usize..(headers_ptr + headers_len) as usize) {
                Some(bytes) => bytes.to_vec(),
                None => return -4,
            };

        let body = match data.get(body_ptr as usize..(body_ptr + body_len) as usize) {
            Some(bytes) => bytes.to_vec(),
            None => return -5,
        };

        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
        let event_sender = caller.data().event_channel.sender();

        // 백그라운드 스레드(Tokio)에서 비동기 HTTP 요청 실행
        tokio::spawn(async move {
            static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
            let client = CLIENT.get_or_init(reqwest::Client::new);
            let method_parsed = match reqwest::Method::from_bytes(method.as_bytes()) {
                Ok(m) => m,
                Err(_) => {
                    let _ = event_sender.send(sdk::event::Event::V1(
                        sdk::event::EventV1::HttpResponse {
                            request_id,
                            status_code: 400,
                            body: b"Invalid HTTP method".to_vec(),
                        },
                    ));
                    return;
                }
            };

            let mut req = client.request(method_parsed, &url);

            // Postcard 직렬화 해제
            if let Ok(headers) = postcard::from_bytes::<Vec<(String, String)>>(&headers_bytes) {
                for (k, v) in headers {
                    req = req.header(k, v);
                }
            }

            if !body.is_empty() {
                req = req.body(body);
            }

            match req.send().await {
                Ok(resp) => {
                    let status_code = resp.status().as_u16();
                    let body_bytes = resp.bytes().await.unwrap_or_default().to_vec();
                    let _ = event_sender.send(sdk::event::Event::V1(
                        sdk::event::EventV1::HttpResponse {
                            request_id,
                            status_code,
                            body: body_bytes,
                        },
                    ));
                }
                Err(e) => {
                    let error_msg = e.to_string().into_bytes();
                    let _ = event_sender.send(sdk::event::Event::V1(
                        sdk::event::EventV1::HttpResponse {
                            request_id,
                            status_code: 500,
                            body: error_msg,
                        },
                    ));
                }
            }
        });

        request_id as i32
    };

    linker.func_wrap("http", "fetch_async", fetch_async_handler)?;

    Ok(())
}


