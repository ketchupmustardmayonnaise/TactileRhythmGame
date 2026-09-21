use bitflags::bitflags;
use sdk::Language;
use sdk::api::keypad::KeyCode;
use tinyvec::TinyVec;

/// 그리기, 삭제, 일반 모드를 나타내는 열거형.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum EditorMode {
    Draw,
    Erase,
    BrushSizeChange,
}

impl EditorMode {
    /// 현재 언어 설정에 맞는 TTS 텍스트를 반환합니다.
    pub fn text(&self, lang: Language) -> &'static str {
        match self {
            Self::Draw => match lang {
                Language::Ko => "그리기 도구",
                Language::En => "Draw Tool",
                Language::Ja => "描画ツール",
            },
            Self::Erase => match lang {
                Language::Ko => "지우기 도구",
                Language::En => "Erase Tool",
                Language::Ja => "消去ツール",
            },
            Self::BrushSizeChange => match lang {
                Language::Ko => "브러시 크기 변경.",
                Language::En => "Change brush size",
                Language::Ja => "ブラシサイズの変更",
            },
        }
    }
}
/// 게임의 전체 상태를 나타내는 열거형
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum GameStatus {
    Playing, // 게임 진행 중
    Menu,
    // Finished, // 게임 종료
}

impl GameStatus {
    pub fn description(&self, lang: Language) -> &'static str {
        match self {
            Self::Playing => match lang {
                Language::Ko => "",
                Language::En => "",
                Language::Ja => "",
            },
            Self::Menu => match lang {
                Language::Ko => "도구선택",
                Language::En => "Tool Selection",
                Language::Ja => "ツール選択",
            },
        }
    }
}

/// 단일 키 입력을 나타내는 구조체
#[derive(Default, Copy, Clone)]
pub struct PressedKey {
    pub key: Option<KeyCode>,
}

/// 키 버퍼의 최대 크기
pub const KEY_BUFFER_CAPACITY: usize = 4;

// 스택 할당을 우선하는 키 버퍼 (최대 4개까지 스택, 초과 시 힙)
pub type KeyBuffer = TinyVec<[PressedKey; KEY_BUFFER_CAPACITY]>;

// 기능키 상태 비트마스크
bitflags! {
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FunctionKeyFlags: u8 {
        const CENTER = 1 << 0;
        const FUN   = 1 << 1;
        const MENU    = 1 << 2;
    }
}
