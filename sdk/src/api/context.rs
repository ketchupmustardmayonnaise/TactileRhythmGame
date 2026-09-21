use crate::{
    AppletInfo, Language,
    api::{
        audio::Audio, keypad::Keypad, preferences::Preferences, settings::Settings, time::Time,
        window::Window,
    },
};

#[derive(Default)]
pub struct ContextV1 {
    pub audio: Audio,
    pub keypad: Keypad,
    pub language: Language,
    pub preferences: Preferences,
    pub settings: Settings,
    pub time: Time,
    pub window: Window,
    pub info: AppletInfo,
    pub http: crate::api::http::Http,
}

impl ContextV1 {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type Context = ContextV1;
