// applets/tactile-game/src/game.rs

use crate::config::GameState;
use crate::rng::Rng;
use widget::item_selector::{
    Horizontal, ItemBar, LayoutStrategy, SelectorBehavior, SelectorStyle,
};
use widget::core::{Widget, WidgetUpdateResult};
use sdk::Language;
use sdk::api::audio::AudioSegment; // [추가] 오디오 채널 세그먼트 열거형 임포트
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, Point, Size};
use sdk::api::keypad::KeyCode;
use sdk::tts_segment; // [추가] 다중 텍스트 및 효과음 처리를 위한 세그먼트 매크로 임포트
use std::time::Duration; // 모노토닉 시간 경과 측정을 위해 표준 Duration 라이브러리를 사용합니다.

/// 매 라운드마다 출제되는 퀴즈 정보 구조체입니다.
pub struct Quiz {
    /// 3지선다형에서 제공되는 고정된 물리 진동 강도 후보군 (0~15 범위)
    pub vibration_levels: Vec<u8>,
    /// 선정된 정답 후보군 리스트의 인덱스 (0..3 범위)
    pub quiz_index: usize,
    /// 정답에 해당하는 실제 물리 진동 강도 값 (vibration_levels[quiz_index])
    pub answer: u8,
}

/// 촉각 게임의 전체 데이터 모델, 게임 컴포넌트, 상태 정보 등을 원격 제어하는 총괄 클래스입니다.
pub struct TactileGame {
    /// 현재 동작 중인 게임 상태
    pub state: GameState,
    /// 가이드 화면 등 일시적인 상태 전환에서 이전 상태로 복구하기 위해 백업해두는 변수
    pub previous_state: Option<GameState>,
    /// 진행 중인 현재 퀴즈의 문항 번호 (1부터 차례로 증가)
    pub quiz_count: usize,
    /// 문제당 제공되는 한도 제한시간 총 초수 (60.0초 = 1분)
    pub timer_max: f32,
    /// 실시간으로 줄어드는 남은 시간(초)
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
    /// 퀴즈 출제 시작 시점의 모노토닉 시간 기록용 변수
    pub quiz_start_time: Option<Duration>,
    /// 가이드 화면으로 넘어가기 위해 애플리케이션을 안전하게 종료해야 하는지 여부 플래그
    pub should_exit: bool,
    /// [추가] 현재 가이드 화면의 진행 단계 (1~8단계)
    pub guide_step: u8,
}

impl Default for TactileGame {
    /// 촉각 게임의 기본 시작 프리셋 정보를 반환합니다.
    fn default() -> Self {
        Self {
            state: GameState::Ready,
            previous_state: None,
            quiz_count: 1,
            timer_max: 60.0, // 문제당 제한시간을 60초(1분)로 정확히 고정합니다.
            timer_current: 60.0,
            rng: Rng::new(),
            quiz_bag: Vec::new(),
            quiz: None,
            answer_bar: None,
            tick: 0,
            quiz_start_time: None,
            should_exit: false, // 기본적으로 종료 플래그는 비활성화 상태입니다.
            guide_step: 1,      // [추가] 가이드 단계를 1단계로 안전하게 초기화합니다.
        }
    }
}

impl TactileGame {
    /// 윈도우 해상도가 바뀔 때마다 객관식 가로 바의 레이아웃을 안전하게 실시간 자동 조율합니다.
    pub fn init_layouts(&mut self, width: i16, _height: i16) {
        // 도전 모드 화면용 객관식 1차원 바(ItemBar) 구축 (3지선다 고정)
        let total = 3;

        let items = vec![""; total];

        // 스크롤이 불필요하며 화면 하단을 가로로 꽉 채우도록 설정합니다.
        // LayoutStrategy::FixedSpacing { spacing: 0 }을 적용하면 여백 없이 화면 너비를 3등분하여 아이템 크기를 자동 계산합니다.
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

    /// pattern.rs에 정의된 10개의 고정 패턴을 문제 번호(quiz_count)에 따라 순차적으로 출제하고,
    /// 3지선다(3개 항목)의 객관식 보기 중 정답 위치 외의 나머지 오답 항목은 1~5 범위의 진동 세기로 랜덤 배치합니다.
    /// 수정 이유: 보기가 3개로 축소됨에 따라 3지선다 고정 사양 및 1~3 정답 번호를 정확하게 매핑하기 위함입니다.
    pub fn next_quiz(&mut self) {
        // 현재 문제 번호(quiz_count)에 해당하는 인덱스를 계산하여 고정 패턴 정보를 로드합니다.
        // 10문제를 넘어서는 경우(예: 11번 문제), 다시 1번 문제 패턴(인덱스 0)부터 차례대로 순환 출제합니다.
        let pattern_idx = (self.quiz_count - 1) % 10;
        let pattern = &crate::pattern::PATTERNS[pattern_idx];

        let quiz_index = pattern.answer_pos - 1; // 0-based index로 변환 (1..=3 -> 0..=2)
        let answer = pattern.vibration; // 정답 진동 세기 (1..=5)

        // 정답 진동세기를 제외한 나머지 진동세기 후보군 [1, 2, 3, 4, 5]를 준비합니다.
        let mut candidates: Vec<u8> = (1..=5).filter(|&v| v != answer).collect();

        // 후보군 목록을 무작위로 섞어 매번 다른 오답 배치가 나오도록 설정합니다. (피셔-예이츠 셔플 알고리즘)
        let candidates_len = candidates.len();
        for i in (1..candidates_len).rev() {
            let j = (self.rng.next_u32() as usize) % (i + 1);
            candidates.swap(i, j);
        }

        // 객관식 답안 항목은 3개로 항상 완벽하게 고정합니다. (3지선다 고정 보장)
        let mut vibration_levels = vec![0; 3];

        // 지정된 고정 정답 위치에 정답 진동 세기를 대입합니다.
        vibration_levels[quiz_index] = answer;

        // 정답 위치를 제외한 나머지 2개의 오답 칸에 셔플된 랜덤 오답 진동 세기를 순서대로 대입합니다.
        let mut candidate_idx = 0;
        for (idx, level) in vibration_levels.iter_mut().enumerate() {
            if idx != quiz_index {
                *level = candidates[candidate_idx];
                candidate_idx += 1;
            }
        }

        // 고정 정답 및 랜덤 오답이 배정된 퀴즈 구조체를 최종 생성합니다.
        let quiz = Quiz {
            vibration_levels,
            quiz_index,
            answer,
        };

        // 검증 및 디버깅을 원활히 지원하기 위해 새로 출제된 문제 정보를 한글 로그로 상세하게 남깁니다.
        log::info!(
            "새로운 퀴즈 출제 - 문제 번호: {}번, 정답 위치: {}번(인덱스: {}), 실제 진동세기: {}단계",
            self.quiz_count,
            quiz.quiz_index + 1,
            quiz.quiz_index,
            quiz.answer
        );

        self.quiz = Some(quiz);

        // 새 퀴즈가 제시되었으므로 제한시간 타이머 바를 가득 리필 충전시킵니다.
        self.timer_current = self.timer_max;

        // 객관식 가로 바의 선택 상태와 스크롤 물리값을 초기 시작 위치로 복원합니다.
        if let Some(ref mut bar) = self.answer_bar {
            bar.selected_index = 0;
            bar.current_scroll = 0.0;
            bar.target_scroll = 0.0;
        }
    }

    /// 진행 상황 및 퀴즈 인덱스 가방, 타이머를 완전히 초기화하고 대기 상태로 되돌립니다.
    pub fn reset_game(&mut self, width: i16, height: i16) {
        self.state = GameState::Ready;
        self.previous_state = None;
        self.quiz_count = 1;
        self.timer_max = 60.0; // 문제당 제한시간을 60초(1분)로 정확히 초기화합니다.
        self.timer_current = 60.0;
        self.quiz = None;
        self.quiz_bag.clear(); // 게임 초기화 시 퀴즈 가방도 완전히 비웁니다.
        self.guide_step = 1; // [추가] 가이드 단위를 초기 기본값 1단계로 되돌립니다.
        self.init_layouts(width, height);
    }

    /// 가이드 화면을 중단하거나 건너뛰고 곧바로 실전 퀴즈 게임을 시작하는 공통 구동 메서드입니다.
    pub fn start_real_game(&mut self, context: &mut Context) {
        self.state = GameState::Playing;
        self.quiz_count = 1;
        self.next_quiz();

        // 시작 환영 메시지 및 1번 퀴즈 정보 나레이션 병합 구성
        let mut start_tts = match context.language {
            Language::Ko => {
                tts_segment!(
                    "지금부터 점자 디스플레이로 촉지게임 테스트를 시작하겠습니다. 게임시작"
                )
            }
            Language::En => tts_segment!(
                "We will now begin the tactile game test using the braille display. Game start."
            ),
            Language::Ja => tts_segment!(
                "ただいまから点字ディスプレイによる触覚ゲームテストを開始します。ゲーム開始"
            ),
        }
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();

        // 첫 문제의 출제 나레이션도 순차적으로 병합
        let quiz_announcement = self.build_quiz_announcement(context.language);
        start_tts.extend(quiz_announcement);

        // 오디오 장치를 통해 전체 음성 시퀀스를 순차적으로 재생합니다.
        context
            .audio
            .play_audio_sequence(&start_tts, sdk::applet::SpeechOption::new());

        // 재생 대기 중 손가락 햅틱 촉지를 위해 모노토닉 타이머의 타임스탬프를 넉넉하게 정렬 오프셋 보정
        let audio_delay_ms = match context.language {
            Language::Ko => 5500,
            Language::En => 6000,
            Language::Ja => 6500,
        };
        self.quiz_start_time =
            Some(context.time.get_monotonic_time() + Duration::from_millis(audio_delay_ms));
        self.timer_current = self.timer_max;
    }

    /// 현재 문제 번호(quiz_count)와 언어 설정을 기반으로 안내 TTS 오디오 시퀀스(AudioSegment 리스트)를 생성해 반환합니다.
    /// 수정 이유: 한국어 명세에 맞게 "번째" 문구를 삽입하고, 다국어 사양을 3지선다에 맞춰 다듬기 위함입니다.
    pub fn build_quiz_announcement(&self, lang: Language) -> Vec<AudioSegment> {
        let ordinal_str = i18n::format_ordinal(lang, self.quiz_count as u32);
        let segments = match lang {
            Language::Ko => tts_segment! {
                    ordinal_str,
                    "문제입니다. 위쪽 사각형부터 만져 보세요."
            },
            Language::En => tts_segment! {
                    "This is the ",
                    ordinal_str,
                    " question. Please feel the upper square first."
            },
            Language::Ja => tts_segment! {
                    ordinal_str,
                    "番目の問題です.上部の四角形から触ってみてください."
            },
        };

        segments
            .into_iter()
            .map(AudioSegment::Text)
            .collect::<Vec<_>>()
    }

    /// 활성화된 게임 내부 모드(GameState)의 전이 규칙에 입각하여, 키패드 입력을 섬세하게 분기 제어합니다.
    pub fn handle_key_event(&mut self, code: KeyCode, context: &mut Context) {
        let width = context.window.width();
        let height = context.window.height();

        match self.state {
            GameState::Ready => {
                if code == KeyCode::Center {
                    // [변경] 중앙키를 누르면 촉지게임 설명 가이드 화면으로 전환하고 1단계 가이드 멘트를 시작합니다.
                    self.state = GameState::Guide;
                    self.guide_step = 1;
                    crate::guide::set_guide_step(self, 1, context);
                } else if code == KeyCode::Menu {
                    // [변경] 메뉴키를 누르면 준비 상태를 즉시 종료하고 새로 구현한 start_real_game 메서드를 호출해 실전 게임을 시작합니다.
                    self.start_real_game(context);
                }
            }
            GameState::Guide => {
                // [추가] 가이드 화면 진행 중에는 가이드 전용 모듈에서 입력을 전담 처리합니다.
                crate::guide::handle_guide_key_event(self, code, context);
            }
            GameState::Playing => {
                if code == KeyCode::Center {
                    // 중앙 키를 누르면 채점 없이 즉시 다음 문항으로 전이하거나 게임을 완전히 종료합니다.
                    if self.quiz_count >= 10 {
                        // 마지막 10번째 문항까지 모두 완료한 경우, 게임을 초기화하고 대기 화면으로 돌아갑니다.
                        self.reset_game(width, height);
                        let tts = match context.language {
                            Language::Ko => {
                                "테스트를 모두 마쳤습니다. 고생했어요. 감사합니다.".to_string()
                            }
                            Language::En => {
                                "The test is complete. Thank you for your hard work.".to_string()
                            }
                            Language::Ja => {
                                "テストをすべて終了しました。お疲れ様でした。ありがとうございました。".to_string()
                            }
                        };
                        context.audio.speak_text(&tts);
                    } else {
                        // 아직 풀어야 할 문제가 남은 경우 다음 문제로 순차 이동합니다.
                        self.quiz_count += 1;
                        self.next_quiz();
                        self.quiz_start_time = Some(context.time.get_monotonic_time()); // 새로운 문제에 대비해 타이머 시작 시점 재지정

                        // 다음 문항 안내 오디오 시퀀스 생성 (헬퍼 함수 활용)
                        let tts = self.build_quiz_announcement(context.language);

                        // 다음 문항 안내 오디오 시퀀스를 출력합니다.
                        context
                            .audio
                            .play_audio_sequence(&tts, sdk::applet::SpeechOption::new());
                    }
                }
            }
        }
    }

    /// 매 프레임 업데이트 틱을 누적하며, 제한시간 타이머 연산을 정밀 처리합니다.
    pub fn update_ticks(&mut self, _width: i16, _height: i16, context: &mut Context) -> bool {
        self.tick = self.tick.wrapping_add(1);

        let mut needs_redraw = false;

        // 1. 실시간 시간 제한 타이머 차감 (플레이 진행 중일 때만 고정 모노토닉 시간 경과에 기반하여 차감)
        if self.state == GameState::Playing
            && let Some(start_time) = self.quiz_start_time
        {
            let current_time = context.time.get_monotonic_time();
            let elapsed = current_time.saturating_sub(start_time);
            let elapsed_secs = elapsed.as_secs_f32();

            // 타이머 최댓값(60.0초)에서 실제 흐른 시간을 정밀 차감합니다.
            self.timer_current = (self.timer_max - elapsed_secs).max(0.0);
            needs_redraw = true; // 타이머 막대가 시시각각 줄어들어야 하므로 화면 재렌더링 수행
        }

        // 하단 바 컴포넌트의 부드러운 스크롤 애니메이션 여부 체크
        if let (GameState::Playing, Some(bar)) = (self.state, &mut self.answer_bar) {
            let res = bar.on_update(context, true).unwrap_or(WidgetUpdateResult::Unchanged);
            if res == WidgetUpdateResult::NeedsRedraw {
                needs_redraw = true;
            }
        }

        // 2. [추가] 가이드 화면에서 깜빡임 애니메이션이 필요한 단계(2, 3, 4, 5단계)일 때만 화면 재렌더링 수행
        if self.state == GameState::Guide && (2..=5).contains(&self.guide_step) {
            needs_redraw = true;
        }

        needs_redraw
    }
}
