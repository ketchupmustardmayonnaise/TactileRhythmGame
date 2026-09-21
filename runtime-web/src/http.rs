use crate::Error;
use js_sys::WebAssembly;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

static NEXT_REQUEST_ID: AtomicU32 = AtomicU32::new(1);

type FetchAsyncClosure = Closure<dyn FnMut(i32, i32, i32, i32, i32, i32, i32, i32) -> i32>;

#[allow(dead_code)]
pub struct HttpClosures {
    pub fetch_async: FetchAsyncClosure,
}

pub fn setup_imports(
    imports: &js_sys::Object,
    memory: Rc<RefCell<Option<WebAssembly::Memory>>>,
    event_queue: crossbeam_channel::Sender<sdk::event::Event>,
) -> Result<HttpClosures, Error> {
    let http = js_sys::Object::new();

    let memory_clone = memory.clone();
    let event_queue_clone = event_queue;

    let fetch_async = Closure::wrap(Box::new(
        move |method_ptr: i32,
              method_len: i32,
              url_ptr: i32,
              url_len: i32,
              headers_ptr: i32,
              headers_len: i32,
              body_ptr: i32,
              body_len: i32|
              -> i32 {
            let method_ptr = method_ptr as u32;
            let method_len = method_len as u32;
            let url_ptr = url_ptr as u32;
            let url_len = url_len as u32;
            let headers_ptr = headers_ptr as u32;
            let headers_len = headers_len as u32;
            let body_ptr = body_ptr as u32;
            let body_len = body_len as u32;

            let Some(mem) = memory_clone.borrow().as_ref().cloned() else {
                return -1;
            };

            let mem_buffer = js_sys::Uint8Array::new(&mem.buffer());

            // Extract method
            let method_bytes = mem_buffer
                .slice(method_ptr, method_ptr + method_len)
                .to_vec();
            let Ok(method) = String::from_utf8(method_bytes) else {
                return -2;
            };

            // Extract url
            let url_bytes = mem_buffer.slice(url_ptr, url_ptr + url_len).to_vec();
            let Ok(url) = String::from_utf8(url_bytes) else {
                return -3;
            };

            // Extract headers
            let headers_bytes = mem_buffer
                .slice(headers_ptr, headers_ptr + headers_len)
                .to_vec();

            // Extract body
            let body_bytes = mem_buffer.slice(body_ptr, body_ptr + body_len).to_vec();

            let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
            let event_sender = event_queue_clone.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let opts = web_sys::RequestInit::new();
                opts.set_method(&method);

                // Setup body if not empty
                if !body_bytes.is_empty() {
                    let body_uint8 = js_sys::Uint8Array::from(body_bytes.as_slice());
                    opts.set_body(&body_uint8);
                }

                // Create headers
                let headers_obj = web_sys::Headers::new().unwrap();
                if let Ok(headers) = postcard::from_bytes::<Vec<(String, String)>>(&headers_bytes) {
                    for (k, v) in headers {
                        let _ = headers_obj.append(&k, &v);
                    }
                }
                opts.set_headers(&headers_obj);

                let request = match web_sys::Request::new_with_str_and_init(&url, &opts) {
                    Ok(req) => req,
                    Err(e) => {
                        let err_str = format!("{:?}", e).into_bytes();
                        let _ = event_sender.send(sdk::event::Event::V1(
                            sdk::event::EventV1::HttpResponse {
                                request_id,
                                status_code: 400,
                                body: err_str,
                            },
                        ));
                        return;
                    }
                };

                let window = web_sys::window().unwrap();
                match wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
                    .await
                {
                    Ok(resp_val) => {
                        let resp: web_sys::Response = resp_val.unchecked_into();
                        let status_code = resp.status();

                        // Get response body as ArrayBuffer
                        if let Ok(array_buffer_promise) = resp.array_buffer() {
                            match wasm_bindgen_futures::JsFuture::from(array_buffer_promise).await {
                                Ok(buf_val) => {
                                    let array_buffer: js_sys::ArrayBuffer =
                                        buf_val.unchecked_into();
                                    let uint8_arr = js_sys::Uint8Array::new(&array_buffer);
                                    let response_body = uint8_arr.to_vec();

                                    let _ = event_sender.send(sdk::event::Event::V1(
                                        sdk::event::EventV1::HttpResponse {
                                            request_id,
                                            status_code,
                                            body: response_body,
                                        },
                                    ));
                                }
                                Err(_) => {
                                    let _ = event_sender.send(sdk::event::Event::V1(
                                        sdk::event::EventV1::HttpResponse {
                                            request_id,
                                            status_code,
                                            body: b"Failed to read body".to_vec(),
                                        },
                                    ));
                                }
                            }
                        } else {
                            let _ = event_sender.send(sdk::event::Event::V1(
                                sdk::event::EventV1::HttpResponse {
                                    request_id,
                                    status_code,
                                    body: Vec::new(),
                                },
                            ));
                        }
                    }
                    Err(e) => {
                        let err_str = format!("{:?}", e).into_bytes();
                        let _ = event_sender.send(sdk::event::Event::V1(
                            sdk::event::EventV1::HttpResponse {
                                request_id,
                                status_code: 500,
                                body: err_str,
                            },
                        ));
                    }
                }
            });

            request_id as i32
        },
    )
        as Box<dyn FnMut(i32, i32, i32, i32, i32, i32, i32, i32) -> i32>);

    js_sys::Reflect::set(
        &http,
        &"fetch_async".into(),
        fetch_async.as_ref().unchecked_ref(),
    )
    .map_err(|e| sdk::error::Error::ImportError(format!("fetch_async: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"http".into(), &http)
        .map_err(|e| sdk::error::Error::ImportError(format!("http: {:?}", e)))?;

    Ok(HttpClosures { fetch_async })
}
