use crate::config::RuntimeConfig;
use crate::host::HostState;
use anyhow::Result;
use sdk::applet::AppletInfo;
use std::fs;
use wasmtime::{Caller, Extern, Linker, Memory};

/// WASM 인스턴스의 메모리를 가져오는 헬퍼 함수
fn get_memory(caller: &mut Caller<'_, HostState>) -> Option<Memory> {
    if let Some(Extern::Memory(mem)) = caller.get_export("memory") {
        Some(mem)
    } else {
        None
    }
}

/// 지정된 포인터와 길이로 WASM 메모리에서 문자열을 읽어오는 헬퍼 함수
fn read_string(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Option<String> {
    let mem = get_memory(caller)?;
    let data = mem.data(caller);
    let (ptr, len) = (ptr as usize, len as usize);

    data.get(ptr..ptr + len)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map(|s| s.to_string())
}

/// 설치된 앱 디렉토리를 탐색하여 애플릿 목록을 가져오는 헬퍼 함수
fn get_installed_applets() -> Vec<AppletInfo> {
    let wasm_dir = RuntimeConfig::load()
        .map(|s| s.get_applet_dir())
        .unwrap_or_else(|_| RuntimeConfig::init_default_applet_dir());
    let mut applets = Vec::new();

    if let Ok(entries) = fs::read_dir(wasm_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && let Ok(wasm_bytes) = fs::read(&path)
            {
                let applet_info = runtime_common::applet::parse_applet_info(stem, &wasm_bytes);
                if !applet_info.hidden {
                    applets.push(applet_info);
                }
            }
        }
    }
    applets.sort_by_key(|info| info.priority);
    applets
}

/// `wasmtime::Linker`에 애플릿 관리(런처) 관련 호스트 함수들을 추가합니다.
pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "applet_manager",
        "list_applets",
        |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> i32 {
            let Some(mem) = get_memory(&mut caller) else {
                return -1;
            };

            let applets = get_installed_applets();

            // 수집된 앱 목록을 직렬화합니다.
            let Ok(serialized) = postcard::to_allocvec(&applets) else {
                return -1;
            };

            // 버퍼 크기가 부족한 경우, 필요한 크기를 반환해 WASM 측에 알립니다.
            if serialized.len() > len as usize {
                return serialized.len() as i32;
            }

            // WASM 메모리에 직렬화된 데이터를 덮어씁니다.
            if mem.write(&mut caller, ptr as usize, &serialized).is_err() {
                return -1;
            }

            serialized.len() as i32
        },
    )?;

    linker.func_wrap(
        "applet_manager",
        "start_applet",
        |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> i32 {
            if let Some(name) = read_string(&mut caller, ptr, len) {
                let _ = caller.data().applet_sender.send(name);
                return 0; // 성공
            }
            -1
        },
    )?;

    linker.func_wrap(
        "applet_manager",
        "get_current_applet",
        |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> i32 {
            let Some(mem) = get_memory(&mut caller) else {
                return -1;
            };

            let current_app_name = caller.data().current_app_name.clone();
            let applets = get_installed_applets();

            // 현재 실행 중인 앱의 ID와 일치하는 AppletInfo를 찾습니다.
            let current_applet = applets
                .into_iter()
                .find(|a| a.id == current_app_name)
                .unwrap_or_default();

            let Ok(serialized) = postcard::to_allocvec(&current_applet) else {
                return -1;
            };

            if serialized.len() > len as usize {
                return serialized.len() as i32;
            }

            if mem.write(&mut caller, ptr as usize, &serialized).is_err() {
                return -1;
            }

            serialized.len() as i32
        },
    )?;

    Ok(())
}
