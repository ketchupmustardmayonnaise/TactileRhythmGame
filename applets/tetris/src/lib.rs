use sdk::Applet;
use sdk::Language;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;

const GAME_OVER_SOUND: &[u8] = include_bytes!("../assets/game_over.mp3");
const TETRIS_CLEAR_SOUND: &[u8] = include_bytes!("../assets/tetris_clear.mp3");
const FIXED_SOUND: &[u8] = include_bytes!("../assets/fixed_block.mp3");
const LINE_CLEAR_SOUND: &[u8] = include_bytes!("../assets/line_clear.mp3");
const ROTATE_SOUND: &[u8] = include_bytes!("../assets/rotate.mp3");
const DONT_SOUND: &[u8] = include_bytes!("../assets/dont.mp3");
const MOVE_SOUND: &[u8] = include_bytes!("../assets/move.mp3");

// 배경 음악은 오디오 플레이어에 처리하는게 없음
// const BACKGROUND_MUSIC: &[u8] = include_bytes!("../assets/background.mp3");

// 좌측 UI 패널 너비
pub const UI_SIDE_WIDTH_CELLS: usize = 4;
// 블록 크기
pub const CELL_SIZE: usize = 2;
// 블럭고정전 유예시간, 천장에 닿았을경우 gameover유에시간 프레임마다++
pub const LOCK_DELAY: usize = 10;

// 블록 종류
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockType {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}
impl From<i32> for BlockType {
    fn from(value: i32) -> Self {
        match value {
            0 => BlockType::I,
            1 => BlockType::J,
            2 => BlockType::L,
            3 => BlockType::O,
            4 => BlockType::S,
            5 => BlockType::T,
            6 => BlockType::Z,
            _ => unreachable!(),
        }
    }
}

// 3. 테트로미노(블록) 구조체 정의 (논리적 좌표 사용)
#[derive(Debug, Clone)]
pub struct Tetromino {
    pub block_type: BlockType,
    pub x: i32, // 논리적 x 좌표 (0 ~ logical_height-1)
    pub y: i32, // 논리적 y 좌표 (0 ~ logical_width-1)
    pub rotation: u8,
}

impl Tetromino {
    pub fn new(block_type: BlockType, board_height: usize) -> Self {
        Self {
            block_type,
            x: -4,                            // 왼쪽 밖에서 시작
            y: (board_height as i32 / 2) - 1, // 세로 중앙
            rotation: 0,
        }
    }

    /// 현재 블록 종류와 회전 상태(0~3)에 따른 4개의 칸의 상대 좌표(dx, dy)를 반환합니다.
    /// 4칸의 좌표만 반환하므로 메모리와 반복문 성능 면에서 매우 효율적입니다.
    pub fn get_cells(&self) -> [(i32, i32); 4] {
        let r = (self.rotation % 4) as usize;
        match self.block_type {
            BlockType::I => [
                [(0, 1), (1, 1), (2, 1), (3, 1)],
                [(2, 0), (2, 1), (2, 2), (2, 3)],
                [(0, 2), (1, 2), (2, 2), (3, 2)],
                [(1, 0), (1, 1), (1, 2), (1, 3)],
            ][r],
            BlockType::J => [
                [(0, 0), (0, 1), (1, 1), (2, 1)],
                [(1, 0), (2, 0), (1, 1), (1, 2)],
                [(0, 1), (1, 1), (2, 1), (2, 2)],
                [(1, 0), (1, 1), (0, 2), (1, 2)],
            ][r],
            BlockType::L => [
                [(2, 0), (0, 1), (1, 1), (2, 1)],
                [(1, 0), (1, 1), (1, 2), (2, 2)],
                [(0, 1), (1, 1), (2, 1), (0, 2)],
                [(0, 0), (1, 0), (1, 1), (1, 2)],
            ][r],
            BlockType::O => [
                [(1, 0), (2, 0), (1, 1), (2, 1)],
                [(1, 0), (2, 0), (1, 1), (2, 1)],
                [(1, 0), (2, 0), (1, 1), (2, 1)],
                [(1, 0), (2, 0), (1, 1), (2, 1)],
            ][r],
            BlockType::S => [
                [(1, 0), (2, 0), (0, 1), (1, 1)],
                [(1, 0), (1, 1), (2, 1), (2, 2)],
                [(1, 1), (2, 1), (0, 2), (1, 2)],
                [(0, 0), (0, 1), (1, 1), (1, 2)],
            ][r],
            BlockType::T => [
                [(1, 0), (0, 1), (1, 1), (2, 1)],
                [(1, 0), (1, 1), (2, 1), (1, 2)],
                [(0, 1), (1, 1), (2, 1), (1, 2)],
                [(1, 0), (0, 1), (1, 1), (1, 2)],
            ][r],
            BlockType::Z => [
                [(0, 0), (1, 0), (1, 1), (2, 1)],
                [(2, 0), (1, 1), (2, 1), (1, 2)],
                [(0, 1), (1, 1), (1, 2), (2, 2)],
                [(1, 0), (0, 1), (1, 1), (0, 2)],
            ][r],
        }
    }
}

// 4. 게임 진행 상태 정의
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GameState {
    Ready,    // 게임 진입 직후, 시작 대기
    Playing,  // 게임 진행 중
    Reset,    // 다시시작
    GameOver, // 게임 종료
    Stop,     // 일시정지
}

impl GameState {
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

pub struct SidePanel {
    pub next_pieces: BlockType, // 다음 블록 종류
    pub score: usize,           // 현재 점수
    pub level: usize,           // 현재 레벨
    pub cleared_lines: usize,   // 현재 레벨에서 삭제한 줄 수
}
// 5. 게임 본체 구조체 정의
pub struct Tetris {
    // 논리적 게임 보드 상태
    pub board: Vec<Vec<Option<BlockType>>>,
    // 현재 조종 중인 블록 (논리적 위치)
    pub current_piece: Option<Tetromino>,
    // 현재 게임 상태
    pub state: GameState,
    // 블록의 중력 처리를 위한 마지막 업데이트 시간 기록
    pub last_tick_ms: u64,
    pub logical_height: usize,
    pub logical_width: usize,
    // 다음 블록들을 보관하는 대기열 (7-bag 시스템)
    pub next_pieces: Vec<BlockType>,
    // 난수 생성을 위한 시드
    pub rng_seed: u32,
    // 점수와 레벨을 관리하는 사이드 패널
    // pub side_panel: SidePanel,
    pub score: u64,
    pub total_lines: u64,
    pub level: u64,
    pub active_key: Option<KeyCode>, // 현재 꾹 누르고 있는 방향키
    pub key_hold_frames: u32,        // 방향키가 눌려있던 시간(프레임 단위)
    pub gravity_frames: u64,         // 레벨별 중력 지연 프레임 캐시
    pub lock_timer: usize,           // 블록 고정 전 유예 시간 타이머
}

impl Tetris {
    pub fn new() -> Self {
        let initial_lw = 20;
        let initial_lh = UI_SIDE_WIDTH_CELLS;
        let mut tetris = Tetris {
            // 빈 공간으로 채워진 논리적 보드 초기화
            board: vec![vec![None; initial_lh]; initial_lw],
            current_piece: None, // 아래에서 첫 블록을 대기열에서 꺼내어 초기화합니다.
            state: GameState::Ready,
            last_tick_ms: 0,
            logical_height: initial_lw,
            logical_width: initial_lh,
            next_pieces: Vec::new(),
            rng_seed: 12345, // 기본 시드 값
            score: 0,
            total_lines: 0,
            level: 1,
            active_key: None,
            key_hold_frames: 0,
            gravity_frames: 0,
            lock_timer: 0,
        };
        tetris.fill_piece_queue();
        let first_block = tetris.next_pieces.remove(0);
        tetris.current_piece = Some(Tetromino::new(first_block, initial_lh));
        tetris.update_gravity_frames();
        tetris
    }

    /// 게임 상태를 변경하고 적절한 음성을 출력합니다.
    pub fn set_state(&mut self, context: &mut Context, new_state: GameState) {
        if self.state != new_state {
            log::info!("게임 상태 변경: {:?} -> {:?}", self.state, new_state);
            self.state = new_state;

            // 일시정지 상태(GameState::Stop)로 전환 시, 이전에 유지되던 연속 키 입력 상태를 초기화하여 오작동을 차단합니다.
            if self.state == GameState::Stop {
                self.active_key = None;
                self.key_hold_frames = 0;
            }

            context
                .audio
                .speak_text(&new_state.gamestate_text(context.language));
        }
    }

    /// 선형 합동 발생기(LCG)를 이용한 간단한 난수 생성
    fn next_rand(&mut self) -> u32 {
        self.rng_seed = self.rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.rng_seed & 0x7fffffff
    }

    /// 레벨에 따른 중력(떨어지는 속도) 프레임 지연 시간을 계산하여 상태에 저장합니다.
    pub fn update_gravity_frames(&mut self) {
        // (1.0 - (level * 0.05))초 마다 한 칸씩, 최소 0.05초
        let gravity_delay_secs = (1.0 - (self.level as f64 * 0.05)).max(0.05);
        self.gravity_frames = (gravity_delay_secs * self.target_fps() as f64) as u64;
    }

    /// 게임을 초기 상태로 되돌리고 다시 시작합니다.
    pub fn reset_game(&mut self) {
        self.board = vec![vec![None; self.logical_width]; self.logical_height];
        self.next_pieces.clear();
        self.fill_piece_queue();
        let next_block = self.next_pieces.remove(0);
        self.current_piece = Some(Tetromino::new(next_block, self.logical_width));
        self.score = 0;
        self.total_lines = 0;
        self.level = 1;
        self.active_key = None;
        self.key_hold_frames = 0;
        self.lock_timer = 0;
        self.update_gravity_frames();
    }

    /// 7-bag 시스템: 7개의 블록을 한 세트로 섞어서 대기열에 추가합니다.
    pub fn fill_piece_queue(&mut self) {
        let mut bag = vec![
            BlockType::I,
            BlockType::J,
            BlockType::L,
            BlockType::O,
            BlockType::S,
            BlockType::T,
            BlockType::Z,
        ];

        // Fisher-Yates 셔플 알고리즘
        for i in (1..bag.len()).rev() {
            let j = (self.next_rand() as usize) % (i + 1);
            bag.swap(i, j);
        }
        self.next_pieces.extend(bag);
    }

    /// 오른쪽(바닥)부터 꽉 찬 세로줄을 찾아서 삭제하고, 왼쪽에서 빈 공간을 밀어넣는 논리적 처리
    pub fn clear_lines(&mut self) -> usize {
        let old_len = self.board.len();
        self.board
            .retain(|col| col.iter().any(|cell| cell.is_none()));
        let lines_cleared = old_len - self.board.len();
        for _ in 0..lines_cleared {
            self.board.insert(0, vec![None; self.logical_width]);
        }
        lines_cleared
    }

    /// 블록이 보드 내에 위치하며 다른 블록과 겹치지 않는지 확인합니다.
    pub fn is_valid_position(&self, piece: &Tetromino) -> bool {
        for (cx, cy) in piece.get_cells() {
            let px = piece.x + cx;
            let py = piece.y + cy;

            // 상하 경계 및 우측 바닥 충돌 검사 (좌측은 블록 회전 및 생성을 위해 열어둠)
            if py < 0 || py >= self.logical_width as i32 || px >= self.logical_height as i32 {
                return false;
            }

            // 화면 내부(px >= 0)일 때만 이미 쌓인 블록과 충돌 검사
            if px >= 0 && self.board[px as usize][py as usize].is_some() {
                return false;
            }
        }
        true
    }

    /// 현재 블록이 바닥이나 다른 블록에 닿기 전까지 최대로 낙하할 수 있는 거리를 한 번에 계산합니다.
    pub fn calculate_drop_distance(&self, piece: &Tetromino) -> i32 {
        let mut min_distance = 100; // 보드 길이보다 충분히 큰 값으로 설정

        for (cx, cy) in piece.get_cells() {
            let px = piece.x + cx;
            let py = piece.y + cy;

            // 보드 바깥 영역이면 무시 (정상적인 위치의 블록이라면 발생하지 않음)
            if py < 0 || py >= self.logical_width as i32 {
                continue;
            }

            // 현재 칸에서 우측으로 탐색
            let mut distance = 0;
            let mut x_check = px + 1;
            while x_check < self.logical_height as i32 {
                if x_check >= 0 && self.board[x_check as usize][py as usize].is_some() {
                    break; // 다른 블록에 닿음
                }
                distance += 1;
                x_check += 1;
            }

            if distance < min_distance {
                min_distance = distance;
            }
        }

        min_distance
    }

    /// 현재 블록을 보드에 고정하고 완성된 줄을 삭제한 뒤, 새 블록을 생성합니다.
    pub fn lock_current_piece(&mut self, context: &mut Context) {
        let mut is_game_over = false;

        if let Some(piece) = &self.current_piece {
            self.rng_seed = self.rng_seed.wrapping_add(self.last_tick_ms as u32); // 시드에 약간의 변화 부여
            for (cx, cy) in piece.get_cells() {
                let px = piece.x + cx;
                let py = piece.y + cy;

                if px < 0 {
                    is_game_over = true;
                } else if py >= 0
                    && py < self.logical_width as i32
                    && px < self.logical_height as i32
                {
                    self.board[px as usize][py as usize] = Some(piece.block_type.clone());
                }
            }
        }

        if is_game_over {
            self.current_piece = None;
            context.audio.play(GAME_OVER_SOUND);
            self.set_state(context, GameState::GameOver);
            return;
        }

        let cleared = self.clear_lines();
        if cleared > 0 {
            // 1. 점수 계산 (레벨 비례)
            let base_score = match cleared {
                1 => 40,
                2 => 100,
                3 => 300,
                4 => 1200,
                _ => 1500, // 4줄 초과는 비정상적이지만, 보너스 점수 부여
            };
            self.score += base_score as u64 * (self.level + 1);

            // 2. 누적 줄 수 및 레벨업 체크 (10줄당 1레벨)
            self.total_lines += cleared as u64;
            let new_level = (self.total_lines / 10) + 1;
            if new_level > self.level {
                self.level = new_level;
                self.update_gravity_frames();
            }

            if cleared >= 4 {
                context.audio.play(TETRIS_CLEAR_SOUND);
            } else {
                context.audio.play(LINE_CLEAR_SOUND);
            }
        } else {
            context.audio.play(FIXED_SOUND);
        }

        // 대기열에 남은 블록이 적으면(새 가방 필요 시) 추가 생성
        if self.next_pieces.len() < 7 {
            self.fill_piece_queue();
        }

        // 새 블록 생성 후 배치
        let next_block = self.next_pieces.remove(0);
        let new_piece = Tetromino::new(next_block, self.logical_width);
        if self.is_valid_position(&new_piece) {
            self.current_piece = Some(new_piece);
        } else {
            // 블록을 생성할 공간이 없으면 게임 오버
            self.current_piece = Some(new_piece); // 게임 오버여도 마지막 블록 표시 유지
            context.audio.play(GAME_OVER_SOUND);
            self.set_state(context, GameState::GameOver);
        }
    }

    /// 키 입력에 따른 블록 이동을 처리합니다.
    /// Home 키가 눌려 앱을 종료해야 하면 true를 반환합니다.
    pub fn process_move(&mut self, context: &mut Context, code: KeyCode) -> bool {
        if let Some(mut piece) = self.current_piece.clone() {
            match code {
                KeyCode::Left => {
                    // 왼쪽은 중력 반대 방향이므로, 대신 회전 버튼으로 사용
                    let next_rotation = piece.rotation.wrapping_add(1);

                    // 테스트해 볼 기준 좌표(x, y)의 오프셋(보정값) 리스트
                    // 현재 좌표계 기준: x가 하강(중력), y가 좌우 이동
                    let kick_offsets = [(0, 0), (0, 1), (0, -1), (-1, 0)];

                    for (offset_x, offset_y) in kick_offsets {
                        let mut test_piece = piece.clone();
                        test_piece.rotation = next_rotation;
                        test_piece.x += offset_x;
                        test_piece.y += offset_y;

                        if self.is_valid_position(&test_piece) {
                            self.current_piece = Some(test_piece);
                            context.audio.play(ROTATE_SOUND);
                            self.lock_timer = 0; // 조작 성공 시 유예 시간 초기화
                            break; // 성공했으니 루프 탈출!
                        }
                    }
                }
                KeyCode::Right => {
                    piece.x += 1; // 우측(바닥) 방향으로 하강
                    if self.is_valid_position(&piece) {
                        self.current_piece = Some(piece);
                        context.audio.play(MOVE_SOUND);
                        self.lock_timer = 0;
                    }
                }
                KeyCode::Up => {
                    piece.y -= 1; // 위쪽으로 이동
                    if self.is_valid_position(&piece) {
                        self.current_piece = Some(piece);
                        context.audio.play(MOVE_SOUND);
                        self.lock_timer = 0;
                    }
                }
                KeyCode::Down => {
                    piece.y += 1; // 아래쪽으로 이동
                    if self.is_valid_position(&piece) {
                        self.current_piece = Some(piece);
                        context.audio.play(MOVE_SOUND);
                        self.lock_timer = 0;
                    }
                }
                KeyCode::Center => {
                    // 하드 드롭: 블록을 우측 끝까지 단번에 이동
                    let drop_distance = self.calculate_drop_distance(&piece);
                    piece.x += drop_distance; // 루프 없이 구한 거리만큼 단번에 점프
                    self.current_piece = Some(piece);
                    context.audio.play(DONT_SOUND);
                    self.lock_timer = 0; // 하드 드롭은 즉시 고정을 위해 타이머 무관하게 작동
                    self.lock_current_piece(context);
                }
                _ => {} // Menu, Unknown, Function 등 사용하지 않는 키는 무시
            }
        }
        false
    }
}

// 6. 런타임에서 실행하기 위해 Applet 트레이트 구현
impl Applet for Tetris {
    fn target_fps(&self) -> u16 {
        30
    }

    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let size = context.window.get_size();
        let width = size.width as usize;
        let height = size.height as usize;

        // 상하좌우 최소 1픽셀씩 테두리를 그리기 위해 너비와 높이에서 2픽셀 여백 확보
        let play_width = width.saturating_sub(2);
        let play_height = height.saturating_sub(2);

        // 좌측 UI 패널 및 구분선(1픽셀)을 제외한 메인 게임 보드 너비 설정
        let ui_pixel_w = UI_SIDE_WIDTH_CELLS * CELL_SIZE;
        let separator_w = 1;
        let board_pixel_w = play_width.saturating_sub(ui_pixel_w + separator_w);

        let new_lw = board_pixel_w / CELL_SIZE;
        let new_lh = play_height / CELL_SIZE;

        // 실제 디스플레이(기기 화면) 크기에 맞춰 논리적 보드 크기 재조정하여 화면 밖으로 이탈 방지
        if new_lw > 0
            && new_lh > 0
            && (self.logical_height != new_lw || self.logical_width != new_lh)
        {
            self.logical_height = new_lw;
            self.logical_width = new_lh;
            self.reset_game();
        }

        // 게임 시작 음성을 명시적으로 한 번만 출력합니다.
        self.set_state(context, GameState::Playing);
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        // 프레임 갱신 시마다 시드를 계속 변경하여 타이밍 기반 무작위성 추가
        // 처음 떨어지는 블록은 타이핑 전이라 고정됨
        self.rng_seed = self.rng_seed.wrapping_add(1);

        // 입력 처리 로직
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            match event.state {
                KeyState::Pressed => {
                    // 사용자의 키 입력 타이밍 또한 불규칙하므로 시드에 강한 변화를 줌
                    self.rng_seed = self
                        .rng_seed
                        .wrapping_mul(1103515245)
                        .wrapping_add(self.last_tick_ms as u32);

                    // 게임 상태에 따른 키 입력 분기 처리
                    match self.state {
                        GameState::GameOver => {
                            if event.code == KeyCode::Function {
                                self.reset_game();
                                self.set_state(context, GameState::Reset);
                                self.state = GameState::Playing; // 다시시작 안내 후 게임이 멈추지 않도록 상태 복귀
                                context.audio.speak_text(match context.language {
                                    Language::Ko => "게임을 다시 시작합니다.",
                                    Language::En => "Restarting game.",
                                    Language::Ja => "ゲームを再スタートします。",
                                });
                            }
                            continue; // 게임오버 상태에서는 다른 입력 무시
                        }
                        GameState::Playing => {
                            if event.code == KeyCode::Function {
                                self.set_state(context, GameState::Stop);
                                continue; // 일시정지 상태로 전환 시 이번 키 이벤트의 후속 조작 처리를 차단합니다.
                            }
                        }
                        GameState::Stop => {
                            if event.code == KeyCode::Function {
                                self.set_state(context, GameState::Playing);
                                continue; // 일시정지 해제 시 이번 키 이벤트의 후속 조작 처리를 차단합니다.
                            } else {
                                // 일시정지 상태에서 오작동(이동, 회전, 하드드롭 등)을 시도할 경우, 알림 음성을 출력하고 입력을 무시합니다.
                                context.audio.speak_text(match context.language {
                                    Language::Ko => "일시정지 상태입니다. 게임을 계속하려면 기능 버튼을 누르세요.",
                                    Language::En => "Game is paused. Press the function button to resume.",
                                    Language::Ja => "一時停止中です。ゲームを再開するには機能ボタンを押してください。",
                                });
                                continue; // 일시정지 중에는 기능 버튼을 제외한 모든 입력을 차단합니다.
                            }
                        }
                        _ => {}
                    }

                    if self.process_move(context, event.code) {
                        log::info!("테트리스 종료 이벤트가 수신되었습니다.");
                        return Ok(UpdateResult::ExitApp);
                    }

                    // 회전(Left)은 연속 입력에서 제외하고, 방향키만 상태 저장
                    if matches!(event.code, KeyCode::Up | KeyCode::Down | KeyCode::Right) {
                        self.active_key = Some(event.code);
                        self.key_hold_frames = 0;
                    }
                }
                KeyState::Released if self.active_key == Some(event.code) => {
                    self.active_key = None;
                }
                _ => {}
            }
        }

        // 방향키 꾹 누르고 있을 때의 연속 이동 처리 (DAS / ARR)
        if self.state == GameState::Playing
            && let Some(key) = self.active_key
        {
            self.key_hold_frames += 1;

            // DAS (Delayed Auto Shift): 꾹 누르고 대기하는 프레임 (30fps 기준 6프레임 = 0.2초)
            const DAS_FRAMES: u32 = 6;
            // ARR (Auto Repeat Rate): 대기 후 연속으로 움직이는 간격 (30fps 기준 2프레임 = 약 0.06초)
            const ARR_FRAMES: u32 = 2;

            if self.key_hold_frames > DAS_FRAMES
                && (self.key_hold_frames - DAS_FRAMES).is_multiple_of(ARR_FRAMES)
            {
                self.process_move(context, key);
            }
        }

        // 바닥 충돌 및 유예 시간(Lock Delay) 처리
        if self.state == GameState::Playing {
            let mut is_grounded = false;
            if let Some(mut piece) = self.current_piece.clone() {
                piece.x += 1;
                if !self.is_valid_position(&piece) {
                    is_grounded = true;
                }
            }

            if is_grounded {
                self.lock_timer += 1;
                if self.lock_timer >= LOCK_DELAY {
                    self.lock_current_piece(context);
                }
            } else {
                self.lock_timer = 0;
            }
        }

        // 일정 시간마다(Tick) 블록을 우측으로 1칸씩 내리는 로직
        if self.state == GameState::Playing {
            self.last_tick_ms += 1; // 30 FPS 환경에서 프레임 카운터로 활용

            if self.last_tick_ms >= self.gravity_frames {
                // 레벨에 따라 계산된 프레임마다 우측 으로 1칸씩 이동
                self.last_tick_ms = 0;
                if let Some(mut piece) = self.current_piece.clone() {
                    piece.x += 1;
                    if self.is_valid_position(&piece) {
                        self.current_piece = Some(piece);
                    }
                }
            }
        }

        Ok(UpdateResult::NeedsRedraw)
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        let size = canvas.get_size();
        let pb_width = size.width as usize;
        let pb_height = size.height as usize;

        // 1. 도화지 깨끗하게 지우기 (초기화)
        canvas.clear();

        // 오프셋 및 크기 계산
        let board_pixel_w = self.logical_height * CELL_SIZE;
        let board_pixel_h = self.logical_width * CELL_SIZE;
        let ui_pixel_w = UI_SIDE_WIDTH_CELLS * CELL_SIZE;
        let separator_w = 1; // 구분선 두께 추가
        let total_pixel_w = board_pixel_w + ui_pixel_w + separator_w;
        let total_pixel_h = board_pixel_h;

        let offset_x = (pb_width.saturating_sub(total_pixel_w)) / 2;
        let offset_y = (pb_height.saturating_sub(total_pixel_h)) / 2;

        let board_offset_x = offset_x + ui_pixel_w + separator_w; // 왼쪽 구분선 이후부터 보드 그리기 시작
        let board_offset_y = offset_y;

        // 2. 테두리 그리기
        let start_x = offset_x.saturating_sub(1);
        let start_y = offset_y.saturating_sub(1);
        let end_x = (offset_x + total_pixel_w).min(pb_width.saturating_sub(1));
        let end_y = (offset_y + total_pixel_h).min(pb_height.saturating_sub(1));

        // 전체 영역 테두리
        for x in start_x..=end_x {
            canvas.set_pin(Point::new(x as i16, start_y as i16), Intensity::MAX);
            canvas.set_pin(Point::new(x as i16, end_y as i16), Intensity::MAX);
        }
        for y in start_y..=end_y {
            canvas.set_pin(Point::new(start_x as i16, y as i16), Intensity::MAX);
            canvas.set_pin(Point::new(end_x as i16, y as i16), Intensity::MAX);
        }

        // 게임 보드와 좌측 UI 패널 사이의 구분선 (세로선)
        let left_separator_x = offset_x + ui_pixel_w;
        if left_separator_x > start_x && left_separator_x < end_x {
            for y in start_y..=end_y {
                canvas.set_pin(
                    Point::new(left_separator_x as i16, y as i16),
                    Intensity::MAX,
                );
            }
        }

        // 3. 쌓여있는 블록 그리기
        for y in 0..self.logical_width {
            for x in 0..self.logical_height {
                if self.board[x][y].is_some() {
                    for dy in 0..CELL_SIZE {
                        for dx in 0..CELL_SIZE {
                            canvas.set_pin(
                                Point::new(
                                    (board_offset_x + x * CELL_SIZE + dx) as i16,
                                    (board_offset_y + y * CELL_SIZE + dy) as i16,
                                ),
                                Intensity::MAX,
                            );
                        }
                    }
                }
            }
        }

        // 4. 현재 블록 그리기
        if let Some(piece) = &self.current_piece {
            // [고스트 블록 그리기]
            if self.state != GameState::GameOver {
                let drop_dist = self.calculate_drop_distance(piece);
                for (cx, cy) in piece.get_cells() {
                    let px = piece.x + drop_dist + cx;
                    let py = piece.y + cy;

                    if px >= 0
                        && px < self.logical_height as i32
                        && py >= 0
                        && py < self.logical_width as i32
                    {
                        for dy in 0..CELL_SIZE {
                            for dx in 0..CELL_SIZE {
                                let draw_x =
                                    (board_offset_x as i32 + px * (CELL_SIZE as i32) + dx as i32)
                                        as i16;
                                let draw_y =
                                    (board_offset_y as i32 + py * (CELL_SIZE as i32) + dy as i32)
                                        as i16;
                                if draw_x >= board_offset_x as i16
                                    && draw_x < pb_width as i16
                                    && draw_y >= 0
                                    && draw_y < pb_height as i16
                                {
                                    canvas.set_pin(
                                        Point::new(draw_x, draw_y),
                                        Intensity::new(100), // 고스트 블록
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // [실제 현재 블록 그리기]
            for (cx, cy) in piece.get_cells() {
                let px = piece.x + cx;
                let py = piece.y + cy;

                if px >= 0
                    && px < self.logical_height as i32
                    && py >= 0
                    && py < self.logical_width as i32
                {
                    for dy in 0..CELL_SIZE {
                        for dx in 0..CELL_SIZE {
                            let draw_x = (board_offset_x as i32
                                + px * (CELL_SIZE as i32)
                                + dx as i32) as i16;
                            let draw_y = (board_offset_y as i32
                                + py * (CELL_SIZE as i32)
                                + dy as i32) as i16;
                            if draw_x >= board_offset_x as i16
                                && draw_x < pb_width as i16
                                && draw_y >= 0
                                && draw_y < pb_height as i16
                            {
                                canvas.set_pin(Point::new(draw_x, draw_y), Intensity::MAX);
                            }
                        }
                    }
                }
            }
        }

        // 5. 다음 블록 그리기
        if !self.next_pieces.is_empty() {
            let next_block_type = &self.next_pieces[0];
            let next_tetromino = Tetromino {
                block_type: next_block_type.clone(),
                x: 0,
                y: 0,
                rotation: 0,
            };
            let cells = next_tetromino.get_cells();

            // 블록 모양의 바운딩 박스 계산
            let mut min_cx = 5;
            let mut max_cx = -1;
            let mut min_cy = 5;
            let mut max_cy = -1;
            for (cx, cy) in cells {
                min_cx = min_cx.min(cx);
                max_cx = max_cx.max(cx);
                min_cy = min_cy.min(cy);
                max_cy = max_cy.max(cy);
            }
            let shape_w = (max_cx - min_cx + 1) as usize;
            let shape_h = (max_cy - min_cy + 1) as usize;

            // UI 패널 내에서 블록을 픽셀 단위로 완벽하게 중앙 정렬
            let shape_pixel_w = shape_w * CELL_SIZE;
            let shape_pixel_h = shape_h * CELL_SIZE;
            let ui_pixel_w = UI_SIDE_WIDTH_CELLS * CELL_SIZE;
            let ui_pixel_h = self.logical_width * CELL_SIZE;

            let pixel_offset_x = ui_pixel_w.saturating_sub(shape_pixel_w) / 2;
            let pixel_offset_y = ui_pixel_h.saturating_sub(shape_pixel_h) / 2;

            for (cx, cy) in cells {
                let cell_x = (cx - min_cx) as usize * CELL_SIZE;
                let cell_y = (cy - min_cy) as usize * CELL_SIZE;

                for dy in 0..CELL_SIZE {
                    for dx in 0..CELL_SIZE {
                        canvas.set_pin(
                            Point::new(
                                (offset_x + pixel_offset_x + cell_x + dx) as i16,
                                (offset_y + pixel_offset_y + cell_y + dy) as i16,
                            ),
                            Intensity::MAX,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for Tetris {
    fn default() -> Self {
        Self::new()
    }
}

// 7. 앱 시작 진입점 (C ABI 사용)
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(Tetris::new()));
}
