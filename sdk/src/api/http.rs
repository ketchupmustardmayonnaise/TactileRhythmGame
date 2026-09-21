use crate::bridge::host_functions::network_fetch_async;
use std::cell::RefCell;
use std::collections::HashMap;

/// WASM 애플릿이 호스트의 HTTP 비동기 통신 기능과 상호작용하기 위한 안전한 래퍼(wrapper) 구조체입니다.
#[derive(Default)]
pub struct Http {
    responses: RefCell<HashMap<u32, (u16, Vec<u8>)>>,
}

impl Http {
    /// 새로운 `Network` 인스턴스를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// 호스트에 HTTP 비동기 요청을 위임합니다.
    /// 성공 시 생성된 request_id를 반환하고, 실패 시 음수를 반환합니다.
    pub fn fetch_async(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<u32, i32> {
        let headers_bytes = postcard::to_allocvec(&headers).unwrap_or_default();
        let res = unsafe {
            network_fetch_async(
                method.as_ptr(),
                method.len(),
                url.as_ptr(),
                url.len(),
                headers_bytes.as_ptr(),
                headers_bytes.len(),
                body.as_ptr(),
                body.len(),
            )
        };
        if res >= 0 { Ok(res as u32) } else { Err(res) }
    }

    /// 특정 request_id의 응답을 수신했는지 확인하고 가져옵니다.
    /// 응답이 준비되었다면 `Some((status_code, body))`를 반환하고 큐에서 제거합니다.
    pub fn take_response(&self, request_id: u32) -> Option<(u16, Vec<u8>)> {
        self.responses.borrow_mut().remove(&request_id)
    }

    /// [SDK 내부용] 호스트로부터 전달받은 네트워크 결과를 캐시에 추가합니다.
    #[allow(dead_code)]
    pub(crate) fn handle_response(&self, request_id: u32, status_code: u16, body: Vec<u8>) {
        self.responses
            .borrow_mut()
            .insert(request_id, (status_code, body));
    }
}
