use sdk::api::keypad::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GameMode {
    #[default]
    Tutorial,
    Length,
    Speed,
}

impl GameMode {
    pub fn next(&self) -> Self {
        match self {
            GameMode::Tutorial => GameMode::Length,
            GameMode::Length => GameMode::Speed,
            GameMode::Speed => GameMode::Tutorial,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            GameMode::Tutorial => GameMode::Speed,
            GameMode::Length => GameMode::Tutorial,
            GameMode::Speed => GameMode::Length,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            GameMode::Tutorial => "튜토리얼 모드",
            GameMode::Length => "길이 증가 모드",
            GameMode::Speed => "속도 증가 모드",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Center,
}

impl Direction {
    pub fn sound_bytes(&self) -> &'static [u8] {
        match self {
            Direction::Up => include_bytes!("../assets/sounds/up.mp3"),
            Direction::Down => include_bytes!("../assets/sounds/down.mp3"),
            Direction::Left => include_bytes!("../assets/sounds/left.mp3"),
            Direction::Right => include_bytes!("../assets/sounds/right.mp3"),
            Direction::Center => include_bytes!("../assets/sounds/center.mp3"),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Direction::Up => "위쪽",
            Direction::Down => "아래쪽",
            Direction::Left => "왼쪽",
            Direction::Right => "오른쪽",
            Direction::Center => "가운데",
        }
    }
}

pub const ALL_DIRECTIONS: [Direction; 5] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
    Direction::Center,
];

pub fn keycode_to_direction(code: KeyCode) -> Option<Direction> {
    match code {
        KeyCode::Up => Some(Direction::Up),
        KeyCode::Down => Some(Direction::Down),
        KeyCode::Left => Some(Direction::Left),
        KeyCode::Right => Some(Direction::Right),
        KeyCode::Center => Some(Direction::Center),
        _ => None,
    }
}

#[derive(Default, PartialEq)]
pub enum GameState {
    #[default]
    Ready,
    Locating,
    Showing,
    Waiting,
    Input,
    Tutorial(TutorialStep),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TutorialStep {
    #[default]
    Introduction,
    IdentifyCross,
    SinglePracticeInit(usize),
    SinglePracticeWait(usize),
    SequencePracticeInit,
    SequencePracticeShow(usize),
    SequencePracticeInput(usize),
    Complete,
}

pub enum InputResult {
    Correct,
    Wrong,
    RoundComplete,
    Ignored,
}

#[derive(Default)]
pub enum PendingTransition {
    #[default]
    None,
    NextRound,
    GameOver(String),
}
