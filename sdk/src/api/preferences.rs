use crate::bridge::host_functions::{
    preferences_get_bytes, preferences_get_float, preferences_get_int, preferences_get_string,
    preferences_set_bytes, preferences_set_float, preferences_set_int, preferences_set_string,
};
use crate::error::{Error, Result};
use std::cell::RefCell;

/// 호스트 스토리지에 다양한 타입의 데이터를 영구 저장하고 불러오기 위한 Preferences API입니다.
pub struct Preferences {
    buffer: RefCell<Vec<u8>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            buffer: RefCell::new(vec![0u8; 1024]),
        }
    }
}

impl Preferences {
    pub fn new() -> Self {
        Self::default()
    }

    /// 문자열(String) 데이터를 저장합니다.
    pub fn set_string<S: Into<String>>(&self, key: S, value: S) -> Result<()> {
        let key = key.into();
        let value = value.into();
        let result =
            unsafe { preferences_set_string(key.as_ptr(), key.len(), value.as_ptr(), value.len()) };
        if result < 0 {
            return Err(Error::FailedToSetPreference(key));
        }
        Ok(())
    }

    /// 문자열(String) 데이터를 불러옵니다.
    pub fn get_string<S: Into<String>>(&self, key: S) -> Result<String> {
        let key = key.into();
        let mut buffer = self.buffer.borrow_mut();
        loop {
            let result = unsafe {
                preferences_get_string(key.as_ptr(), key.len(), buffer.as_mut_ptr(), buffer.len())
            };

            if result >= 0 {
                let len = result as usize;
                return Ok(String::from_utf8_lossy(&buffer[..len]).into_owned());
            }

            match result {
                -1 => return Err(Error::KeyNotFound(key)),
                -2 => {
                    // 버퍼 크기가 작아 데이터를 온전히 받지 못했을 때 크기를 2배로 늘려 다시 시도합니다.
                    let current_len = buffer.len();
                    if current_len >= 1024 * 1024 {
                        // 최대 1MB까지만 확장
                        return Err(Error::BufferTooSmall);
                    }
                    buffer.resize(current_len * 2, 0);
                }
                _ => return Err(Error::FailedToGetPreference(key)),
            }
        }
    }

    /// 정수(i32) 데이터를 저장합니다.
    pub fn set_int<S: Into<String>>(&self, key: S, value: i32) -> Result<()> {
        let key = key.into();
        let result = unsafe { preferences_set_int(key.as_ptr(), key.len(), value) };
        if result < 0 {
            return Err(Error::FailedToSetPreference(key));
        }
        Ok(())
    }

    /// 정수(i32) 데이터를 불러옵니다.
    pub fn get_int<S: Into<String>>(&self, key: S) -> Result<i32> {
        let key = key.into();
        let mut out_val = 0;
        let result = unsafe { preferences_get_int(key.as_ptr(), key.len(), &mut out_val) };
        if result >= 0 {
            Ok(out_val)
        } else if result == -1 {
            Err(Error::KeyNotFound(key))
        } else {
            Err(Error::FailedToGetPreference(key))
        }
    }

    /// 실수(f32) 데이터를 저장합니다.
    pub fn set_float<S: Into<String>>(&self, key: S, value: f32) -> Result<()> {
        let key = key.into();
        let result = unsafe { preferences_set_float(key.as_ptr(), key.len(), value) };
        if result < 0 {
            return Err(Error::FailedToSetPreference(key));
        }
        Ok(())
    }

    /// 실수(f32) 데이터를 불러옵니다.
    pub fn get_float<S: Into<String>>(&self, key: S) -> Result<f32> {
        let key = key.into();
        let mut out_val = 0.0;
        let result = unsafe { preferences_get_float(key.as_ptr(), key.len(), &mut out_val) };
        if result >= 0 {
            Ok(out_val)
        } else if result == -1 {
            Err(Error::KeyNotFound(key))
        } else {
            Err(Error::FailedToGetPreference(key))
        }
    }

    /// 바이트 배열(vec<u8>) 데이터를 저장합니다.
    pub fn set_bytes<S: Into<String>>(&self, key: S, value: &[u8]) -> Result<()> {
        let key = key.into();
        let result =
            unsafe { preferences_set_bytes(key.as_ptr(), key.len(), value.as_ptr(), value.len()) };
        if result < 0 {
            return Err(Error::FailedToSetPreference(key));
        }
        Ok(())
    }

    /// 바이트 배열(vec<u8>) 데이터를 불러옵니다.
    pub fn get_bytes<S: Into<String>>(&self, key: S) -> Result<Vec<u8>> {
        let key = key.into();
        let mut buffer = self.buffer.borrow_mut();
        loop {
            let result = unsafe {
                preferences_get_bytes(key.as_ptr(), key.len(), buffer.as_mut_ptr(), buffer.len())
            };

            if result >= 0 {
                let len = result as usize;
                return Ok(buffer[..len].to_vec());
            }

            match result {
                -1 => return Err(Error::KeyNotFound(key)),
                -2 => {
                    // 버퍼 크기가 작아 데이터를 온전히 받지 못했을 때 크기를 2배로 늘려 다시 시도합니다.
                    let current_len = buffer.len();
                    if current_len >= 1024 * 1024 {
                        // 최대 1MB까지만 확장
                        return Err(Error::BufferTooSmall);
                    }
                    buffer.resize(current_len * 2, 0);
                }
                _ => return Err(Error::FailedToGetPreference(key)),
            }
        }
    }
}
