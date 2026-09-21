use crate::config::RuntimeConfig;
use crate::host::HostState;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use wasmtime::{Caller, Extern, Linker};

fn get_data_dir() -> PathBuf {
    RuntimeConfig::init_default_settings_dir()
}

fn get_prefs_path() -> PathBuf {
    get_data_dir().join("preferences.json")
}

fn get_settings_path() -> PathBuf {
    get_data_dir().join("settings.json")
}

// 전역 캐시 인스턴스
static PREF_STORE_CACHE: OnceLock<RwLock<PreferenceStore>> = OnceLock::new();
static APP_SETTINGS_CACHE: OnceLock<RwLock<AppSettings>> = OnceLock::new();

#[derive(Serialize, Deserialize, Default, Clone)]
struct PreferenceStore {
    #[serde(default)]
    strings: HashMap<String, String>,
    #[serde(default)]
    ints: HashMap<String, i32>,
    #[serde(default)]
    floats: HashMap<String, f32>,
    #[serde(default)]
    bytes: HashMap<String, String>,
}

impl PreferenceStore {
    /// 메모리에 로드된 전역 인스턴스의 읽기 락을 반환합니다. 초기화되지 않았다면 디스크에서 읽어옵니다.
    fn get() -> std::sync::RwLockReadGuard<'static, Self> {
        let rwlock = PREF_STORE_CACHE.get_or_init(|| {
            let data = fs::read_to_string(get_prefs_path()).unwrap_or_default();
            let store = serde_json::from_str(&data).unwrap_or_default();
            RwLock::new(store)
        });
        rwlock.read().unwrap()
    }

    /// 메모리에 로드된 전역 인스턴스의 쓰기 락을 반환합니다.
    fn get_mut() -> std::sync::RwLockWriteGuard<'static, Self> {
        let rwlock = PREF_STORE_CACHE.get_or_init(|| {
            let data = fs::read_to_string(get_prefs_path()).unwrap_or_default();
            let store = serde_json::from_str(&data).unwrap_or_default();
            RwLock::new(store)
        });
        rwlock.write().unwrap()
    }

    /// 현재 상태를 디스크에 저장합니다.
    // 백그라운드에 던지고 종료합니다. 메모리 캐시에는 반영되어도 실제 json파일에 저장되는 시점은 보장되지 않습니다.
    fn save_async(store: &PreferenceStore) {
        let store_clone = store.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(data) = serde_json::to_string_pretty(&store_clone) {
                let _ = fs::write(get_prefs_path(), data);
            }
        });
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    #[serde(default)]
    pub language: i32,
    #[serde(default)]
    pub tts_voice: i32,
    #[serde(default)]
    pub volume: f32,
    #[serde(default)]
    pub tts_speed: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: 0,
            tts_voice: 0,
            volume: 50.0,
            tts_speed: 1,
        }
    }
}

impl AppSettings {
    fn init_from_disk() -> Self {
        if let Ok(data) = fs::read_to_string(get_settings_path()) {
            let mut settings: AppSettings = serde_json::from_str(&data).unwrap_or_default();

            // value_list 검증 (범위 초과 시 default 값으로 변경)
            if !(0..=4).contains(&settings.language) {
                settings.language = 0;
            }
            if !(0..=9).contains(&settings.tts_voice) {
                settings.tts_voice = 0;
            }
            if settings.volume <= 9.0 {
                settings.volume *= 10.0;
            }
            if !(10.0..=100.0).contains(&settings.volume) {
                settings.volume = 50.0;
            }
            if !(0..=2).contains(&settings.tts_speed) {
                settings.tts_speed = 1;
            }
            settings
        } else {
            // 파일이 없다면 default 값으로 생성 후 동기적으로 최초 1회 저장

            let settings = Self::default();
            if let Ok(data) = serde_json::to_string_pretty(&settings) {
                let _ = fs::write(get_settings_path(), data);
            }
            settings
        }
    }

    /// 외부에서 이전 API 호환성을 위해 사용하는 동기식 load.
    /// (실제로는 메모리 캐시의 클론을 반환하여 I/O 비용이 없음)
    pub fn load() -> Self {
        Self::get().clone()
    }

    /// 메모리에 로드된 전역 인스턴스의 읽기 락을 반환합니다.
    fn get() -> std::sync::RwLockReadGuard<'static, Self> {
        let rwlock = APP_SETTINGS_CACHE.get_or_init(|| RwLock::new(Self::init_from_disk()));
        rwlock.read().unwrap()
    }

    /// 메모리에 로드된 전역 인스턴스의 쓰기 락을 반환합니다.
    fn get_mut() -> std::sync::RwLockWriteGuard<'static, Self> {
        let rwlock = APP_SETTINGS_CACHE.get_or_init(|| RwLock::new(Self::init_from_disk()));
        rwlock.write().unwrap()
    }

    /// 현재 상태를 디스크에 저장합니다 (비동기 처리).
    fn save_async(settings: &AppSettings) {
        let settings_clone = settings.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(data) = serde_json::to_string_pretty(&settings_clone) {
                let _ = fs::write(get_settings_path(), data);
            }
        });
    }
}

/// WASM 인스턴스의 메모리를 가져오는 헬퍼 함수
fn get_memory(caller: &mut Caller<'_, HostState>) -> Option<wasmtime::Memory> {
    if let Some(Extern::Memory(mem)) = caller.get_export("memory") {
        Some(mem)
    } else {
        None
    }
}

fn read_string(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Option<String> {
    let mem = get_memory(caller)?;
    let data = mem.data(caller);
    let (ptr, len) = (ptr as usize, len as usize);

    data.get(ptr..ptr + len)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map(|s| s.to_string())
}

fn read_bytes(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Option<Vec<u8>> {
    let mem = get_memory(caller)?;
    let data = mem.data(caller);
    let (ptr, len) = (ptr as usize, len as usize);

    data.get(ptr..ptr + len).map(|bytes| bytes.to_vec())
}

fn write_bytes(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32, bytes: &[u8]) -> i32 {
    let Some(mem) = get_memory(caller) else {
        return -1;
    };
    if bytes.len() > len as usize {
        return -2; // 버퍼 부족
    }
    match mem.write(caller, ptr as usize, bytes) {
        Ok(_) => bytes.len() as i32,
        Err(_) => -1,
    }
}

pub fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    // 앱/런타임 초기화 시점에 저장된 설정을 불러와 오디오 시스템에 볼륨을 미리 적용합니다.
    let settings = AppSettings::load();
    crate::audio::set_volume(settings.volume / 100.0);

    // TTS 엔진에 초기 언어/목소리 설정을 적용(초기화)합니다.
    crate::audio::update_tts_settings(settings.language, settings.tts_voice, settings.tts_speed);

    linker.func_wrap(
        "preferences",
        "get_string",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         buf_ptr: u32,
         buf_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let store = PreferenceStore::get();
            let Some(val) = store.strings.get(&key) else {
                return -1;
            };

            write_bytes(&mut caller, buf_ptr, buf_len, val.as_bytes())
        },
    )?;

    linker.func_wrap(
        "preferences",
        "set_string",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         val_ptr: u32,
         val_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let Some(val) = read_string(&mut caller, val_ptr, val_len) else {
                return -1;
            };

            let mut store = PreferenceStore::get_mut();
            store.strings.insert(key, val);
            PreferenceStore::save_async(&store);
            0
        },
    )?;

    linker.func_wrap(
        "preferences",
        "get_int",
        |mut caller: Caller<'_, HostState>, key_ptr: u32, key_len: u32, out_ptr: u32| -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let store = PreferenceStore::get();
            let Some(&val) = store.ints.get(&key) else {
                return -1;
            };
            let Some(mem) = get_memory(&mut caller) else {
                return -1;
            };

            match mem.write(&mut caller, out_ptr as usize, &val.to_le_bytes()) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    linker.func_wrap(
        "preferences",
        "set_int",
        |mut caller: Caller<'_, HostState>, key_ptr: u32, key_len: u32, val: i32| -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let mut store = PreferenceStore::get_mut();
            store.ints.insert(key, val);
            PreferenceStore::save_async(&store);
            0
        },
    )?;

    linker.func_wrap(
        "preferences",
        "get_float",
        |mut caller: Caller<'_, HostState>, key_ptr: u32, key_len: u32, out_ptr: u32| -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let store = PreferenceStore::get();
            let Some(&val) = store.floats.get(&key) else {
                return -1;
            };
            let Some(mem) = get_memory(&mut caller) else {
                return -1;
            };

            match mem.write(&mut caller, out_ptr as usize, &val.to_le_bytes()) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    linker.func_wrap(
        "preferences",
        "set_float",
        |mut caller: Caller<'_, HostState>, key_ptr: u32, key_len: u32, val: f32| -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let mut store = PreferenceStore::get_mut();
            store.floats.insert(key, val);
            PreferenceStore::save_async(&store);
            0
        },
    )?;

    linker.func_wrap(
        "preferences",
        "get_bytes",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         buf_ptr: u32,
         buf_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let store = PreferenceStore::get();
            let Some(val_b64) = store.bytes.get(&key) else {
                return -1;
            };

            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            let Ok(bytes) = BASE64.decode(val_b64) else {
                return -1;
            };

            write_bytes(&mut caller, buf_ptr, buf_len, &bytes)
        },
    )?;

    linker.func_wrap(
        "preferences",
        "set_bytes",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         val_ptr: u32,
         val_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let Some(bytes) = read_bytes(&mut caller, val_ptr, val_len) else {
                return -1;
            };

            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            let val_b64 = BASE64.encode(bytes);
            let mut store = PreferenceStore::get_mut();
            store.bytes.insert(key, val_b64);
            PreferenceStore::save_async(&store);
            0
        },
    )?;
    linker.func_wrap(
        "settings",
        "get",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         buf_ptr: u32,
         buf_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let settings = AppSettings::get();

            let val_str = if key == "app_settings_options" {
                let lang_val = settings.language.to_string();
                let voice_val = settings.tts_voice.to_string();
                let vol_val = (settings.volume as i32).to_string();
                let speed_val = settings.tts_speed.to_string();

                Some(runtime_common::settings::generate_app_settings_json(
                    &lang_val, &voice_val, &vol_val, &speed_val,
                ))
            } else {
                match key.as_str() {
                    "language" => Some(settings.language.to_string()),
                    "tts_voice" => Some(settings.tts_voice.to_string()),
                    "audio_volume" => Some((settings.volume as i32).to_string()),
                    "speech_rate" => Some(settings.tts_speed.to_string()),
                    _ => None,
                }
            };

            let Some(val) = val_str else {
                return -1;
            };
            write_bytes(&mut caller, buf_ptr, buf_len, val.as_bytes())
        },
    )?;

    linker.func_wrap(
        "settings",
        "set",
        |mut caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         val_ptr: u32,
         val_len: u32|
         -> i32 {
            let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };
            let Some(val_str) = read_string(&mut caller, val_ptr, val_len) else {
                return -1;
            };

            let mut settings = AppSettings::get_mut();
            let mut updated = false;

            match key.as_str() {
                "language" => {
                    if let Ok(val) = val_str.parse::<i32>()
                        && (0..=4).contains(&val)
                    {
                        settings.language = val;
                        updated = true;
                        // 언어 설정 변경 시 즉시 오디오 모듈에 업데이트 신호 전송
                        crate::audio::update_tts_settings(
                            settings.language,
                            settings.tts_voice,
                            settings.tts_speed,
                        );
                    }
                }
                "tts_voice" => {
                    if let Ok(val) = val_str.parse::<i32>()
                        && (0..=9).contains(&val)
                    {
                        settings.tts_voice = val;
                        updated = true;
                        // 목소리 설정 변경 시 즉시 오디오 모듈에 업데이트(모델 교체) 신호 전송
                        crate::audio::update_tts_settings(
                            settings.language,
                            settings.tts_voice,
                            settings.tts_speed,
                        );
                    }
                }
                "audio_volume" => {
                    if let Ok(val) = val_str.parse::<f32>()
                        && (10.0..=100.0).contains(&val)
                    {
                        settings.volume = val;
                        updated = true;
                        // 오디오 모듈에 볼륨 변경 즉시 반영 (0.0 ~ 1.0 스케일로 변환)
                        crate::audio::set_volume(val / 100.0);
                    }
                }

                "speech_rate" => {
                    if let Ok(val) = val_str.parse::<i32>()
                        && (0..=2).contains(&val)
                    {
                        settings.tts_speed = val;
                        updated = true;
                        // TTS 엔진 모델 설정에도 속도 변경을 동기화
                        crate::audio::update_tts_settings(
                            settings.language,
                            settings.tts_voice,
                            settings.tts_speed,
                        );
                    }
                }
                _ => {}
            }

            if updated {
                AppSettings::save_async(&settings);
            }
            0
        },
    )?;

    Ok(())
}
