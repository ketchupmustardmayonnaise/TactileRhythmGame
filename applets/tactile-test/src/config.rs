// applets/tactile-game/src/config.rs

use sdk::Language;

/// 촉각 게임의 세부 라이프사이클 화면 모드를 나타내는 상태 열거형입니다.
#[derive(Default, PartialEq, Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum GameState {
    #[default]
    Ready, // 게임 시작 대기 (메인 메뉴)
    Guide,   // 가이드 화면 진행 중
    Playing, // 게임 도전 진행 중
}

impl GameState {
    /// 각 동작 상태의 한글/다국어 음성 출력 텍스트를 반환합니다.
    #[allow(dead_code)]
    pub fn gamestate_text(&self, lang: Language) -> String {
        match lang {
            Language::Ko => match self {
                Self::Ready => "준비",
                Self::Guide => "가이드",
                Self::Playing => "게임 시작",
            },
            Language::En => match self {
                Self::Ready => "Ready",
                Self::Guide => "Guide",
                Self::Playing => "Game Start",
            },
            Language::Ja => match self {
                Self::Ready => "準備",
                Self::Guide => "ガイド",
                Self::Playing => "ゲーム開始",
            },
        }
        .to_string()
    }
}
