#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "display")]
unsafe extern "C" {
    /// 지정된 위치의 픽셀(점자 핀) 상태를 호스트에 설정합니다.
    #[link_name = "set_pin"]
    pub fn display_set_pin(x: i16, y: i16, intensity: u8, blink: bool);
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn display_set_pin(_x: i16, _y: i16, _intensity: u8, _blink: bool) {}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "keypad")]
unsafe extern "C" {
    /// `KeypadMode`를 설정합니다. (mode가 true이면 perkins 모드, false이면 일반 모드)
    #[link_name = "set_perkins_mode"]
    pub fn keypad_set_perkins_mode(mode: bool);
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn keypad_set_perkins_mode(_mode: bool) {}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "log")]
unsafe extern "C" {
    #[link_name = "message"]
    pub fn log_message(level: u32, ptr: *const u8, len: usize);
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn log_message(_level: u32, _ptr: *const u8, _len: usize) {}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "time")]
unsafe extern "C" {
    #[link_name = "get_time_seconds"]
    pub fn time_get_time_seconds() -> u64;
    #[link_name = "get_timezone_offset_seconds"]
    pub fn time_get_timezone_offset_seconds() -> i32;
    #[link_name = "get_monotonic_time_nanos"]
    pub fn time_get_monotonic_time_nanos() -> u64;
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn time_get_time_seconds() -> u64 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn time_get_timezone_offset_seconds() -> i32 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn time_get_monotonic_time_nanos() -> u64 { 0 }

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "preferences")]
unsafe extern "C" {
    #[link_name = "set_string"]
    pub fn preferences_set_string(key_ptr: *const u8, key_len: usize, val_ptr: *const u8, val_len: usize) -> i32;
    #[link_name = "get_string"]
    pub fn preferences_get_string(key_ptr: *const u8, key_len: usize, buf_ptr: *mut u8, buf_len: usize) -> i32;
    #[link_name = "set_int"]
    pub fn preferences_set_int(key_ptr: *const u8, key_len: usize, val: i32) -> i32;
    #[link_name = "get_int"]
    pub fn preferences_get_int(key_ptr: *const u8, key_len: usize, out_val: *mut i32) -> i32;
    #[link_name = "set_float"]
    pub fn preferences_set_float(key_ptr: *const u8, key_len: usize, val: f32) -> i32;
    #[link_name = "get_float"]
    pub fn preferences_get_float(key_ptr: *const u8, key_len: usize, out_val: *mut f32) -> i32;
    #[link_name = "set_bytes"]
    pub fn preferences_set_bytes(key_ptr: *const u8, key_len: usize, val_ptr: *const u8, val_len: usize) -> i32;
    #[link_name = "get_bytes"]
    pub fn preferences_get_bytes(key_ptr: *const u8, key_len: usize, buf_ptr: *mut u8, buf_len: usize) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_set_string(_key_ptr: *const u8, _key_len: usize, _val_ptr: *const u8, _val_len: usize) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_get_string(_key_ptr: *const u8, _key_len: usize, _buf_ptr: *mut u8, _buf_len: usize) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_set_int(_key_ptr: *const u8, _key_len: usize, _val: i32) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_get_int(_key_ptr: *const u8, _key_len: usize, _out_val: *mut i32) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_set_float(_key_ptr: *const u8, _key_len: usize, _val: f32) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_get_float(_key_ptr: *const u8, _key_len: usize, _out_val: *mut f32) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_set_bytes(_key_ptr: *const u8, _key_len: usize, _val_ptr: *const u8, _val_len: usize) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn preferences_get_bytes(_key_ptr: *const u8, _key_len: usize, _buf_ptr: *mut u8, _buf_len: usize) -> i32 { -1 }

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "settings")]
unsafe extern "C" {
    #[link_name = "get"]
    pub fn settings_get(key_ptr: *const u8, key_len: usize, buf_ptr: *mut u8, buf_len: usize) -> i32;
    #[link_name = "set"]
    pub fn settings_set(key_ptr: *const u8, key_len: usize, val_ptr: *const u8, val_len: usize) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn settings_get(_key_ptr: *const u8, _key_len: usize, _buf_ptr: *mut u8, _buf_len: usize) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn settings_set(_key_ptr: *const u8, _key_len: usize, _val_ptr: *const u8, _val_len: usize) -> i32 { -1 }

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "audio")]
unsafe extern "C" {
    #[link_name = "play"]
    pub fn audio_play(ptr: *const u8, len: usize);
    #[link_name = "play_sequence"]
    pub fn audio_play_sequence(ptr: *const u8, len: usize);
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn audio_play(_ptr: *const u8, _len: usize) {}
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn audio_play_sequence(_ptr: *const u8, _len: usize) {}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "applet_manager")]
unsafe extern "C" {
    #[link_name = "start_applet"]
    pub fn applet_manager_start_applet(applet_id_ptr: *const u8, applet_id_len: usize) -> i32;
    #[link_name = "list_applets"]
    pub fn applet_manager_list_applets(buf_ptr: *mut u8, buf_len: usize) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn applet_manager_start_applet(_applet_id_ptr: *const u8, _applet_id_len: usize) -> i32 { -1 }
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn applet_manager_list_applets(_buf_ptr: *mut u8, _buf_len: usize) -> i32 { -1 }

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "http")]
unsafe extern "C" {
    #[link_name = "fetch_async"]
    pub fn network_fetch_async(
        method_ptr: *const u8,
        method_len: usize,
        url_ptr: *const u8,
        url_len: usize,
        headers_ptr: *const u8,
        headers_len: usize,
        body_ptr: *const u8,
        body_len: usize,
    ) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn network_fetch_async(
    _method_ptr: *const u8,
    _method_len: usize,
    _url_ptr: *const u8,
    _url_len: usize,
    _headers_ptr: *const u8,
    _headers_len: usize,
    _body_ptr: *const u8,
    _body_len: usize,
) -> i32 { -1 }
