use crate::types::{EditorMode, GameStatus};
use sdk::api::audio::Speakable;
use sdk::{Language, tts_segment};
use std::borrow::Cow;

/// TTS 출력을 위한 이벤트 타입 정의
#[derive(Debug, Clone, PartialEq)]
pub enum SpeakEvent {
    GameStatusChange(GameStatus), // 게임 상태 변화
    ToolChange(EditorMode),
    BrushSizeChangeGuide,
    BrushSizeChanged(bool), // true: 크게, false: 작게
    BrushSizeConfirmed,
    // MovementChange(&'static str),
}

impl SpeakEvent {
    /// 각 언어에 맞게 이벤트 메시지를 문장 또는 단어 단위 세그먼트 벡터로 반환합니다.
    /// 긴 안내 문장의 경우, 쪼개진 세그먼트 배열을 반환하여 순차 발화(speak_segments)가 가능하게 합니다.
    pub fn segments(&self, lang: Language) -> Vec<String> {
        match self {
            SpeakEvent::BrushSizeChangeGuide => match lang {
                Language::Ko => tts_segment!(
                    "브러시 크기 변경 모드입니다. ",
                    "위아래 방향키로 크기를 조절하고, ",
                    "중앙 키를 눌러 적용하거나, ",
                    "메뉴 키를 눌러 취소할 수 있습니다."
                ),
                Language::En => tts_segment!(
                    "This is the brush size change mode. ",
                    "Adjust the size using the up and down arrow keys, ",
                    "press the center key to apply, or ",
                    "press the menu key to cancel."
                ),
                Language::Ja => tts_segment!(
                    "ブラシサイズ変更モードです。 ",
                    "上下方向キーでサイズを調節し、 ",
                    "中央キーを押して適用するか、 ",
                    "メニューキーを押してキャンセルできます。"
                ),
            },
            _ => vec![self.text(lang).into_owned()],
        }
    }
}

impl Speakable for SpeakEvent {
    fn text(&self, lang: Language) -> Cow<'_, str> {
        match self {
            SpeakEvent::GameStatusChange(status) => Cow::Borrowed(status.description(lang)),
            SpeakEvent::ToolChange(mode) => Cow::Borrowed(mode.text(lang)),
            SpeakEvent::BrushSizeChangeGuide => {
                // segments 결과를 하나로 결합하여 반환합니다.
                Cow::Owned(self.segments(lang).join(""))
            }
            SpeakEvent::BrushSizeChanged(is_increased) => {
                if *is_increased {
                    Cow::Owned(match lang {
                        Language::Ko => "크게".to_string(),
                        Language::En => "Larger".to_string(),
                        Language::Ja => "大きく".to_string(),
                    })
                } else {
                    Cow::Owned(match lang {
                        Language::Ko => "작게".to_string(),
                        Language::En => "Smaller".to_string(),
                        Language::Ja => "小さく".to_string(),
                    })
                }
            }
            SpeakEvent::BrushSizeConfirmed => Cow::Owned(match lang {
                Language::Ko => "브러시 크기가 변경되었습니다.".to_string(),
                Language::En => "Brush size has been changed.".to_string(),
                Language::Ja => "ブラシサイズが変更されました。".to_string(),
            }),
        }
    }
}
