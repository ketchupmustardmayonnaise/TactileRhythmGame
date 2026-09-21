// applets/tactile-game/src/config.rs

use sdk::Language;

/// 5지선다 게임에서 사용할 고정된 5단계 물리 진동 강도 레벨 (0~15 범위)
pub const VIBRATION_LEVELS: [u8; 5] = [3, 6, 9, 12, 15];

/// 촉각 게임의 세부 라이프사이클 화면 모드를 나타내는 상태 열거형입니다.
#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum GameState {
    #[default]
    Ready, // 게임 시작 대기 (메인 메뉴)
    Playing, // 게임 도전 진행 중
    Reset,   // 전체 초기화 확인 경고 화면
    #[allow(dead_code)]
    GameOver, // 실패 종료 화면 (제한시간 초과 시)
    Stop,    // 일시 정지 화면
}

impl GameState {
    /// 각 동작 상태의 한글/다국어 음성 출력 텍스트를 반환합니다.
    pub fn gamestate_text(&self, lang: Language) -> String {
        match lang {
            Language::Ko => match self {
                Self::Ready => "준비",
                Self::Playing => "게임 시작",
                Self::Reset => "다시시작",
                Self::GameOver => "게임 종료",
                Self::Stop => "일시정지",
            },
            Language::En => match self {
                Self::Ready => "Ready",
                Self::Playing => "Game Start",
                Self::Reset => "Reset",
                Self::GameOver => "Game Over",
                Self::Stop => "Stop",
            },
            Language::Ja => match self {
                Self::Ready => "準備",
                Self::Playing => "ゲーム開始",
                Self::Reset => "再スタート",
                Self::GameOver => "ゲーム終了",
                Self::Stop => "一時停止",
            },
        }
        .to_string()
    }
}
