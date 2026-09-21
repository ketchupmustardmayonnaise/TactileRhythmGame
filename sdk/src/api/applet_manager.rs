use crate::applet::AppletInfo;
use crate::bridge::host_functions::{applet_manager_list_applets, applet_manager_start_applet};
use crate::error::{Error, Result};

// 버퍼 사이즈는 필요에 따라 조절할 수 있습니다. 16KB로 확장하여 앱 수가 많아져도 안전하게 수신합니다.
const APP_LIST_BUFFER_SIZE: usize = 16384;


#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "applet_manager")]
unsafe extern "C" {
    fn get_current_applet(ptr: *mut u8, len: usize) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_current_applet(_ptr: *mut u8, _len: usize) -> i32 { -1 }

/// `AppletManager`: WASM 앱에서 호스트 시스템의 런처 기능을 호출하기 위한 API 인터페이스입니다.
#[derive(Default)]
pub struct AppletManager {}

impl AppletManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 호스트에 설치된 앱 리스트를 요청하여 반환합니다.
    pub fn list_applets(&mut self) -> Result<Vec<AppletInfo>> {
        // 호스트의 응답(설치된 앱 목록)을 받을 버퍼를 준비합니다.
        let mut buffer: [u8; APP_LIST_BUFFER_SIZE] = [0; APP_LIST_BUFFER_SIZE];

        // 외부(Host) 런타임에 정의된 `applet_manager_list_applets` 함수를 호출합니다.
        let result = unsafe { applet_manager_list_applets(buffer.as_mut_ptr(), buffer.len()) };

        if result >= 0 {
            let len = result as usize;

            // 반환된 길이가 우리가 준비한 버퍼보다 크다면 데이터가 잘린 것이므로 에러를 반환합니다.
            if len > buffer.len() {
                return Err(Error::RuntimeError(format!(
                    "호스트에서 반환된 앱 목록 크기({})가 버퍼 크기({})를 초과합니다.",
                    len,
                    buffer.len()
                )));
            }

            // 버퍼에 담긴 데이터를 `Vec<AppletInfo>` 형태로 다시 객체화(역직렬화)합니다.
            match postcard::from_bytes(&buffer[..len]) {
                Ok(applet_list) => Ok(applet_list),
                Err(e) => Err(Error::RuntimeError(format!("앱 목록 역직렬화 실패: {}", e))),
            }
        } else {
            Err(Error::RuntimeError(format!(
                "앱 목록 가져오기 실패, 에러 코드: {}",
                result
            )))
        }
    }

    /// 호스트에게 특정 앱을 실행하도록 요청합니다.
    pub fn start_applet(&mut self, applet_name: &str) -> Result<()> {
        let result = unsafe {
            // 앱 이름을 호스트가 읽을 수 있도록 포인터와 길이로 변환하여 넘깁니다.
            applet_manager_start_applet(applet_name.as_ptr(), applet_name.len())
        };

        if result == 0 {
            Ok(())
        } else {
            Err(Error::RuntimeError(format!(
                "앱 '{}' 실행 실패, 에러 코드: {}",
                applet_name, result
            )))
        }
    }

    /// 현재 실행 중인 애플릿의 메타데이터(AppletInfo)를 가져옵니다.
    pub fn get_current_applet_info(&self) -> Result<AppletInfo> {
        let mut buffer: [u8; APP_LIST_BUFFER_SIZE] = [0; APP_LIST_BUFFER_SIZE];

        let result = unsafe { get_current_applet(buffer.as_mut_ptr(), buffer.len()) };

        if result >= 0 {
            let len = result as usize;
            if len > buffer.len() {
                return Err(Error::RuntimeError(format!("버퍼 크기 초과: {}", len)));
            }

            match postcard::from_bytes(&buffer[..len]) {
                Ok(info) => Ok(info),
                Err(e) => Err(Error::RuntimeError(format!("앱 정보 역직렬화 실패: {}", e))),
            }
        } else {
            Err(Error::RuntimeError(
                "현재 애플릿 정보를 가져오는데 실패했습니다.".to_string(),
            ))
        }
    }
}
