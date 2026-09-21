// applets/tactile-game/src/game.rs

use crate::config::{GameState, VIBRATION_LEVELS};
use crate::rng::Rng;
use widget::item_selector::{
    Horizontal, ItemBar, LayoutStrategy, SelectorBehavior, SelectorStyle,
};
use widget::core::{Widget, WidgetUpdateResult};
use sdk::Language;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, Point, Size};
use sdk::api::keypad::KeyCode;
use sdk::tts_segment; // [추가] 다중 텍스트 및 효과음 처리를 위한 세그먼트 매크로 임포트 // [추가] 오디오 채널 세그먼트 열거형 임포트
const CORRECT_SOUND: &[u8] = include_bytes!("../assets/correct.mp3");
const INCORRECT_SOUND: &[u8] = include_bytes!("../assets/incorrect_flat.mp3");

/// 매 라운드마다 출제되는 퀴즈 정보 구조체입니다.
pub struct Quiz {
    /// 5지선다형에서 제공되는 고정된 물리 진동 강도 후보군 (0~15 범위)
    pub vibration_levels: Vec<u8>,
    /// 선정된 정답 후보군 리스트의 인덱스 (0..5 범위)
    pub quiz_index: usize,
    /// 정답에 해당하는 실제 물리 진동 강도 값 (vibration_levels[quiz_index])
    pub answer: u8,
}

impl Quiz {
    /// 특정 퀴즈 인덱스를 활용하여 일관된 퀴즈 정보를 신규 생성합니다.
    pub fn new_with_index(quiz_index: usize) -> Self {
        let vibration_levels = VIBRATION_LEVELS.to_vec();
        let answer = vibration_levels[quiz_index];
        Self {
            vibration_levels,
            quiz_index,
            answer,
        }
    }
}

/// 촉각 게임의 전체 데이터 모델, 게임 컴포넌트, 상태 정보 등을 원격 제어하는 총괄 클래스입니다.
pub struct TactileGame {
    /// 현재 동작 중인 게임 상태
    pub state: GameState,
    /// 가이드 화면 등 일시적인 상태 전환에서 이전 상태로 복구하기 위해 백업해두는 변수
    pub previous_state: Option<GameState>,
    /// 진행 중인 현재 퀴즈의 문항 번호 (1부터 차례로 증가)
    pub quiz_count: usize,
    /// 아케이드 누적 획득 점수 (문제를 맞출 때마다 10점씩 증가)
    pub score: usize,
    /// 문제당 제공되는 한도 제한시간 프레임 총량 (초기값 300.0 = 약 5초)
    pub timer_max: f32,
    /// 실시간으로 줄어드는 남은 프레임 양
    pub timer_current: f32,
    /// 난수 추출을 위한 고유의 Xorshift 시드 생성기
    pub rng: Rng,
    /// 테트리스 백(Bag) 시스템처럼 중복을 방지하기 위해 셔플하여 순서대로 출제하는 인덱스 가방
    pub quiz_bag: Vec<usize>,
    /// 현재 활성화된 출제 문제 정보
    pub quiz: Option<Quiz>,
    /// 게임 진행 중 아래쪽에서 가로 방향으로 정답을 골라 제출하는 객관식 바
    pub answer_bar: Option<ItemBar<'static>>,
    /// 회전 점선 테두리 그리기 및 애니메이션 연계 틱 카운터
    pub tick: i16,
    /// 정답 또는 오답 제출 시, 화려한 촉각적 연출 피드백을 가하기 위한 타이머 프레임 수
    pub feedback_timer: i16,
    /// 최근 채점한 정답의 오답 판정 상태 (Some(true): 정답, Some(false): 오답, None: 연출 없음)
    pub feedback_correct: Option<bool>,
}

impl Default for TactileGame {
    /// 촉각 게임의 기본 시작 프리셋 정보를 반환합니다.
    fn default() -> Self {
        Self {
            state: GameState::Ready,
            previous_state: None,
            quiz_count: 1,
            score: 0,
            timer_max: 300.0,
            timer_current: 300.0,
            rng: Rng::new(),
            quiz_bag: Vec::new(),
            quiz: None,
            answer_bar: None,
            tick: 0,
            feedback_timer: 0,
            feedback_correct: None,
        }
    }
}

impl TactileGame {
    /// 윈도우 해상도가 바뀔 때마다 객관식 가로 바의 레이아웃을 안전하게 실시간 자동 조율합니다.
    pub fn init_layouts(&mut self, width: i16, _height: i16) {
        // 도전 모드 화면용 객관식 1차원 바(ItemBar) 구축 (5지선다 고정)
        let total = 5;

        let items = vec![""; total];

        // 스크롤이 불필요하며 화면 하단을 가로로 꽉 채우도록 설정합니다.
        // LayoutStrategy::FixedSpacing { spacing: 0 }을 적용하면 여백 없이 화면 너비를 5등분하여 아이템 크기를 자동 계산합니다.
        let bar_behavior = SelectorBehavior {
            scroll_canvas_size: None, // 스크롤을 비활성화하여 오프스크린 캔버스 공간 낭비를 막고 화면 해상도에 정확히 피팅합니다.
            layout_strategy: LayoutStrategy::FixedSpacing { spacing: 0 },
            selection_style: Some(SelectorStyle::border(1)), // 사용자가 선택한 답안에 명확하게 외곽 테두리를 두르도록 설정합니다.
        };

        let mut bar = ItemBar::new(items, Box::new(Horizontal), bar_behavior);
        // 하단 메뉴 영역의 높이(MENU_PANEL_HEIGHT = 8) 기준으로 레이아웃과 뷰포트를 일치시켜 생성합니다.
        bar.set_bounds(BoundsRect::new(Point::new(0, _height - 8), Size::new(width, 8)));
        self.answer_bar = Some(bar);
    }

    /// 테트리스 가방 시스템(Bag Randomizer)을 활용하여 중복되지 않게 문제를 신규 출제하고 객관식 인덱스를 복구합니다.
    pub fn next_quiz(&mut self) {
        let total = 5; // 5지선다 고정

        // 퀴즈 가방이 비어 있다면 전체 정답 인덱스 목록을 새로 채우고 무작위 셔플합니다.
        if self.quiz_bag.is_empty() {
            self.quiz_bag = (0..total).collect();
            // 피셔-예이츠 셔플(Fisher-Yates Shuffle) 알고리즘을 사용한 무작위 카드 섞기
            for i in (1..total).rev() {
                let j = (self.rng.next_u32() as usize) % (i + 1);
                self.quiz_bag.swap(i, j);
            }
            log::info!(
                "퀴즈 가방 풀이 비어 새로이 섞어 채워 넣었습니다. 신규 가방 상태: {:?}",
                self.quiz_bag
            );
        }

        // 가방 맨 뒤에서 인덱스를 하나씩 정직하게 pop하여 출제합니다.
        let quiz_index = self.quiz_bag.pop().unwrap_or(0);
        let quiz = Quiz::new_with_index(quiz_index);

        // 디버깅을 위해 선정된 퀴즈 정보를 로그로 출력합니다.
        log::info!(
            "새 퀴즈 정답: {}단계 (실제 값: {}), 현재 가방 잔여 문항 개수: {}",
            quiz.quiz_index + 1,
            quiz.answer,
            self.quiz_bag.len()
        );
        self.quiz = Some(quiz);

        // 새 퀴즈가 제시되었으므로 제한시간 타이머 바를 가득 리필 충전시킵니다.
        self.timer_current = self.timer_max;

        if let Some(ref mut bar) = self.answer_bar {
            bar.selected_index = 0;
            // 스크롤 물리값도 초기 위치로 원상 복구
            bar.current_scroll = 0.0;
            bar.target_scroll = 0.0;
        }
    }

    /// 진행 상황 및 퀴즈 인덱스 가방, 타이머, 점수판을 완전히 초기화하고 대기 상태로 되돌립니다.
    pub fn reset_game(&mut self, width: i16, height: i16) {
        self.state = GameState::Ready;
        self.previous_state = None;
        self.quiz_count = 1;
        self.score = 0;
        self.timer_max = 300.0;
        self.timer_current = 300.0;
        self.quiz = None;
        self.quiz_bag.clear(); // 게임 초기화 시 퀴즈 가방도 완전히 비웁니다.
        self.feedback_correct = None;
        self.feedback_timer = 0;
        self.init_layouts(width, height);
    }

    /// 활성화된 게임 내부 모드(GameState)의 전이 규칙에 입각하여, 키패드 입력을 섬세하게 분기 제어합니다.
    pub fn handle_key_event(&mut self, code: KeyCode, context: &mut Context) {
        let width = context.window.width();
        let height = context.window.height();

        match self.state {
            GameState::Ready => {
                if code == KeyCode::Center {
                    // 게임 아케이드 도전의 막을 올립니다!
                    self.state = GameState::Playing;
                    self.quiz_count = 1;
                    self.score = 0;
                    self.timer_current = self.timer_max;
                    self.next_quiz();

                    let tts = match context.language {
                        Language::Ko => tts_segment! {
                            "게임을 시작합니다. ","위의 퀴즈 상자를 손가락으로 만져보고, ","아래 선택 바에서 같은 강도의 진동을 찾아 선택해 보세요."
                        },
                        Language::En => tts_segment! {
                            "Start game. "," Feel the quiz box at the top, ","and choose the same vibration intensity in the answer bar below."
                        },
                        Language::Ja => tts_segment! {
                            "ゲームを開始します。","上のクイズボックスを指で触って、","下の選択バーから同じ強さの振動を探して選択してください。"
                        },
                    }
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect::<Vec<_>>();

                    // [추가] 생성된 음성 세그먼트들을 즉시 강제 재생(forced)하여 게임 시작을 알립니다.
                    context
                        .audio
                        .play_audio_sequence(&tts, sdk::applet::SpeechOption::forced());
                }
            }
            GameState::Playing => {
                // 피드백 연출 타이머 동작 중에는 고장 방지를 위해 키 이벤트 입력 차단
                if self.feedback_timer > 0 {
                    return;
                }

                match code {
                    KeyCode::Menu => {
                        // 게임 진행 중 전체 초기화 단축키를 눌렀을 때, 안전 조치로 Reset 경고 분기로 즉각 이행합니다.
                        self.previous_state = Some(self.state);
                        self.state = GameState::Reset;

                        let tts = match context.language {
                            Language::Ko => "정말로 게임 점수와 진행률을 초기화하시겠습니까? 가운데 키를 누르면 완전히 초기화하고, 펑션 키를 누르면 취소합니다.".to_string(),
                            Language::En => "Are you sure you want to reset your score and progress? Press Center to confirm, Function to cancel.".to_string(),
                            Language::Ja => "本当にゲームスコアと進行状況を初期化しますか？中央キーを押すと初期化し、ファンクションキーを押すとキャンセルします。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    KeyCode::Function => {
                        // 일시정지로 도피합니다
                        self.state = GameState::Stop;
                        context
                            .audio
                            .speak_text(&GameState::Stop.gamestate_text(context.language));
                    }
                    KeyCode::Center => {
                        // 플레이어의 응답 값을 퀴즈의 정답(answer)과 엄밀하게 교차 매칭 대조합니다.
                        if let (Some(quiz), Some(bar)) = (&self.quiz, &self.answer_bar) {
                            let selected_idx = bar.selected_index;
                            let user_val = quiz.vibration_levels[selected_idx];

                            if user_val == quiz.answer {
                                // 정답인 경우! 맞출 때마다  점수를 더해줍니다.
                                self.score += 1;
                                self.feedback_correct = Some(true);
                                self.feedback_timer = 20; // 20프레임 동안 정답 깜빡임 연출 수행

                                context.audio.play(CORRECT_SOUND);

                                let tts = match context.language {
                                    Language::Ko => "정답입니다!".to_string(),
                                    Language::En => "Correct!".to_string(),
                                    Language::Ja => "正解です！".to_string(),
                                };
                                context.audio.speak_text_with_option(
                                    &tts,
                                    sdk::applet::SpeechOption::forced(),
                                );
                            } else {
                                // 오답인 경우! 1점을 소폭 감점(음수 하한 보호 saturating_sub)하고 타이머 1초(60프레임) 즉각 단축 패널티를 적용하며 제자리 유지
                                self.score = self.score.saturating_sub(1);
                                self.timer_current = (self.timer_current - 60.0).max(0.0);
                                self.feedback_correct = Some(false);
                                self.feedback_timer = 20; // 20프레임 동안 오답 연출 수행

                                context.audio.play(INCORRECT_SOUND);

                                let tts = match context.language {
                                    Language::Ko => tts_segment!(
                                        "틀렸습니다. ",
                                        "시간 패널티가 주어집니다. ",
                                        "다시 선택해 보세요."
                                    ),
                                    Language::En => tts_segment!(
                                        "Incorrect. ",
                                        "Time penalty applied. ",
                                        "Please try choosing again."
                                    ),
                                    Language::Ja => tts_segment!(
                                        "違います。 ",
                                        "時間ペナルティが与えられます。 ",
                                        "もう一度選んでみてください。"
                                    ),
                                }
                                .into_iter()
                                .map(AudioSegment::Text)
                                .collect::<Vec<_>>();
                                context
                                    .audio
                                    .play_audio_sequence(&tts, sdk::applet::SpeechOption::new());
                            }
                        }
                    }
                    _ => {
                    }
                }
            }
            GameState::GameOver => {
                match code {
                    KeyCode::Center | KeyCode::Function => {
                        // 아케이드 신속한 재도전 진행! 모든 변수 초기화 후 전열 가다듬기
                        self.state = GameState::Playing;
                        self.score = 0;
                        self.quiz_count = 1;
                        self.timer_current = self.timer_max;
                        self.next_quiz(); // 퀴즈 신규 생성 및 타이머 리필

                        let tts = match context.language {
                            Language::Ko => "게임을 다시 시작합니다.".to_string(),
                            Language::En => "Restarting game.".to_string(),
                            Language::Ja => "ゲームを再スタートします。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    KeyCode::Menu => {
                        // 메인 대기 화면으로 안전 리턴
                        self.reset_game(width, height);

                        let tts = match context.language {
                            Language::Ko => "대기 화면으로 돌아왔습니다.".to_string(),
                            Language::En => "Returned to start screen.".to_string(),
                            Language::Ja => "待機画面に戻りました。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    _ => {}
                }
            }
            GameState::Stop => {
                match code {
                    KeyCode::Function => {
                        // 일시 정지 해제
                        self.state = GameState::Playing;
                        let tts = match context.language {
                            Language::Ko => "게임을 재개합니다.".to_string(),
                            Language::En => "Game resumed.".to_string(),
                            Language::Ja => "ゲームを再開します。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    KeyCode::Menu => {
                        // 전체 초기화를 위한 경고 확인 모드로 이행
                        self.previous_state = Some(self.state);
                        self.state = GameState::Reset;

                        let tts = match context.language {
                            Language::Ko => "정말로 게임을 초기화하시겠습니까? 가운데 키를 누르면 초기화하고, 펑션 키를 누르면 취소합니다.".to_string(),
                            Language::En => "Are you sure you want to reset? Press Center to confirm, Function to cancel.".to_string(),
                            Language::Ja => "本当にゲームを初期化しますか？中央キーを押すと初期化し、ファンクションキーを押すとキャンセルします。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    _ => {}
                }
            }
            GameState::Reset => {
                match code {
                    KeyCode::Center => {
                        // 흔적 없이 모두 리셋하고 대기 화면 복귀
                        self.reset_game(width, height);
                        let tts = match context.language {
                            Language::Ko => "게임을 초기화했습니다. 메인 화면입니다.".to_string(),
                            Language::En => "Game has been reset. Back to main screen.".to_string(),
                            Language::Ja => "ゲームを初期化しました。メイン画面です。".to_string(),
                        };
                        context.audio.speak_text(&tts);
                    }
                    KeyCode::Function => {
                        // 리셋을 철회하고 일시정지 상태로 복귀
                        self.state = GameState::Stop;
                        self.previous_state = None;

                        let tts = match context.language {
                            Language::Ko => {
                                "초기화를 취소하고 일시정지 화면으로 돌아왔습니다.".to_string()
                            }
                            Language::En => "Reset cancelled. Back to pause screen.".to_string(),
                            Language::Ja => {
                                "初期化をキャンセルし、一時停止画面に戻りました。".to_string()
                            }
                        };
                        context.audio.speak_text(&tts);
                    }
                    _ => {}
                }
            }
        }
    }

    /// 매 프레임 업데이트 틱을 누적하며, 채점 연출 지속시간 차감 및 타이머 시간 연산을 정밀 처리합니다.
    pub fn update_ticks(&mut self, _width: i16, _height: i16, context: &mut Context) -> bool {
        self.tick = self.tick.wrapping_add(1);

        let mut needs_redraw = false;

        // 1. 피드백 연출 타이머 차감 및 다음 라운드 전이 처리
        if self.feedback_timer > 0 {
            self.feedback_timer -= 1;
            needs_redraw = true;

            // 타이머가 끝난 시점에 정오답 연출이 마무리되고 다음 처리 진행
            if self.feedback_timer == 0 {
                let was_correct = self.feedback_correct == Some(true);
                self.feedback_correct = None;

                if was_correct {
                    // 정답을 맞춘 경우에만 문항 번호를 1 증가시키고 새로운 문제를 신규 출제합니다.
                    self.quiz_count += 1;
                    self.next_quiz(); // 새 퀴즈 출제 및 제한시간 타이머 100% 충전 자동 수행

                    // 새로운 문제 번호 음성 안내 (정적 텍스트와 동적 변수를 사전에 완벽히 세그먼트화함)
                    let tts = match context.language {
                        Language::Ko => tts_segment!(self.quiz_count, "번 문제."),
                        Language::En => tts_segment!("Question ", self.quiz_count, "."),
                        Language::Ja => tts_segment!(self.quiz_count, "番の問題"),
                    }
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect::<Vec<_>>();

                    // 변환된 음성 세그먼트들을 오디오 서비스의 시퀀스 재생 인터페이스를 통해 안전하게 재생합니다.
                    context
                        .audio
                        .play_audio_sequence(&tts, sdk::applet::SpeechOption::new());
                } else {
                    // 오답인 경우는 새 문제를 제시하지 않고, 패널티로 줄어든 타이머를 즉각 적용
                    // 만약 오답 연출 완료 후 남은 시간이 이미 0 이하라면 즉시 게임오버 검진에 걸림
                }
            }
        }

        // 2. 실시간 시간 제한 타이머 차감 (플레이 진행 중이며 피드백 연출 정체 타이머가 돌고 있지 않을 때만)
        if self.state == GameState::Playing && self.feedback_timer == 0 {
            // 점수가 오를수록 빨리줄어듬
            let speed_multiplier = 0.2 + (self.score as f32 * 0.05);
            self.timer_current -= 1.0 * speed_multiplier;
            needs_redraw = true; // 타이머 막대가 시시각각 수축하므로 재그리기 적극 인가

            // 제한 시간 초과로 인한 타임 오버 처리 (게임 종료 없이 1점 감점 후 강제 다음 문제로 전이)
            if self.timer_current <= 0.0 {
                self.timer_current = 0.0;

                // 시간 초과 시 1점 감점 처리 (점수는 saturating_sub를 활용해 음수 하한 보호선 확보)
                self.score = self.score.saturating_sub(1);

                // 새로운 퀴즈로 세대를 강제 이행시킴
                self.quiz_count += 1;
                self.next_quiz(); // 퀴즈 신규 출제 및 타이머 바 100% 충전 자동 수행

                // 시간 초과 감점 및 다음 문항 음성 알림 출력 (모든 동적 수치 변수와 다국어 템플릿을 완벽히 세그먼트화함)
                let tts = match context.language {
                    Language::Ko => tts_segment!(
                        "시간이 초과되어 1점이 감점되었습니다. 현재 점수는 ",
                        self.score,
                        "점입니다. 다음 ",
                        self.quiz_count,
                        "번 문제입니다."
                    ),
                    Language::En => tts_segment!(
                        "Time out! 1 point deducted. Current score is ",
                        self.score,
                        " points. Next, question ",
                        self.quiz_count,
                        "."
                    ),
                    Language::Ja => tts_segment!(
                        "時間切れです！1点減点されました。現在のスコアは",
                        self.score,
                        "点です。次の問",
                        self.quiz_count,
                        "です。"
                    ),
                }
                .into_iter()
                .map(AudioSegment::Text)
                .collect::<Vec<_>>();

                // 강제 재생(forced) 옵션을 부여하여 오디오 시퀀스를 지체 없이 즉각 출력합니다.
                context
                    .audio
                    .play_audio_sequence(&tts, sdk::applet::SpeechOption::forced());
            }
        }

        // 하단 바 컴포넌트의 부드러운 스크롤 애니메이션 여부 체크 및 이벤트 처리
        if self.state == GameState::Playing
            && let Some(ref mut bar) = self.answer_bar
        {
            let prev_idx = bar.selected_index;
            let res = bar.on_update(context, true).unwrap_or(WidgetUpdateResult::Unchanged);
            if res == WidgetUpdateResult::NeedsRedraw {
                needs_redraw = true;
                if prev_idx != bar.selected_index {
                    let cur = (bar.selected_index + 1).to_string();
                    let tts = match context.language {
                        Language::Ko => tts_segment!(&cur, "단계 진동"),
                        Language::En => tts_segment!("Level ", &cur, " vibration"),
                        Language::Ja => tts_segment!(&cur, "段階の振動"),
                    }
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect::<Vec<_>>();
                    context
                        .audio
                        .play_audio_sequence(&tts, sdk::applet::SpeechOption::new());
                }
            }
        }

        needs_redraw
    }
}
