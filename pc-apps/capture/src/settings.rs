use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn get_settings_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "moredream", "capture") {
        let dir = proj_dirs.config_dir();
        if !dir.exists() {
            let _ = std::fs::create_dir_all(dir);
        }
        dir.join("capture_settings.toml")
    } else {
        PathBuf::from("capture_settings.toml")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureArea {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureSettings {
    pub serial_address: String,
    pub network_address: String,
    pub websocket_address: String,
    pub connection_type: String,
    pub filter_mode: String,
    pub invert_active: bool,
    pub capture_area: Option<CaptureArea>,
    pub threshold: u8,
    pub portrait_mode: bool,
    pub use_high_range: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            serial_address: String::new(),
            network_address: String::new(),
            websocket_address: String::new(),
            connection_type: "Serial".to_string(),
            filter_mode: "HighContrast".to_string(),
            invert_active: false,
            capture_area: None,
            threshold: 128,
            portrait_mode: false,
            use_high_range: false,
        }
    }
}
