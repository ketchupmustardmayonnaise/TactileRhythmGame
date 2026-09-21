use crate::types::EditorMode;
use sdk::Language;
use sdk::api::audio::Speakable;
use std::borrow::Cow;

/// TTS 출력을 위한 이벤트 타입 정의
#[derive(Debug, Clone, PartialEq)]
pub enum SpeakEvent {
    ToolChange(EditorMode),
}

impl Speakable for SpeakEvent {
    fn text(&self, lang: Language) -> Cow<'_, str> {
        match self {
            SpeakEvent::ToolChange(mode) => Cow::Borrowed(mode.text(lang)),
        }
    }
}
