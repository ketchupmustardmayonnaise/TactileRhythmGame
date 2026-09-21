use braille::TextArea;
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use widget::{Spacing, Widget};

const MANUAL_TEXT: &str = r#"장치 사용자 매뉴얼

1. 개요
본 장치는 멀티라인 점자 디스플레이 단말기로, 텍스트 및 점자 그래픽을 실시간으로 표시하고 음성 안내(TTS)를 제공합니다.

2. 전원 켜기 및 끄기
- 전원 켜기: 측면 전원 버튼을 짧게 누르면 장치가 켜집니다.
- 전원 끄기: 전원 버튼을 3초 이상 길게 누르면 시스템이 종료됩니다.

3. 주요 키 위치 및 조작법
- 기능 키: 사용자와 가장 가까운 두 개의 키 중 위쪽 키
- 메뉴 키: 사용자와 가장 가까운 두 개의 키 중 아래쪽 키
- 방향키 (상/하/좌/우): 항목 선택 및 문서/점자 스크롤
- 중앙 (센터) 키: 선택 항목 실행 및 퍼킨스 점자 공백 입력

4. 런처(메인 화면) 이동 및 도움말
- 런처 이동: 메뉴 키와 기능 키(사용자와 가장 가까운 2개 키)를 동시에 누르면 메인 런처 화면으로 이동합니다.
- 도움말 듣기: 앱 실행 중 메뉴 키와 기능 키를 동시에 누르면 도움말 음성이 출력됩니다.

5. 퍼킨스 점자 입력
메모 등 점자 입력 앱에서는 6점 점자 키패드를 사용하여 점자 코드를 직접 입력할 수 있습니다.

6. 시스템 카테고리
설정, 장치 사용자 매뉴얼 등 단말기의 기본 관리 도구가 포함되어 있습니다."#;

/// 일반 한글/영문 텍스트를 braille::text_to_braille로 변환 후 TextArea lines 구조(Vec<Vec<Vec<u8>>>)로 생성
fn text_to_braille_lines(text: &str) -> Vec<Vec<Vec<u8>>> {
    let mut result_lines = Vec::new();
    for line_str in text.lines() {
        let cells = braille::text_to_braille(line_str);
        let mut words = Vec::new();
        let mut current_word = Vec::new();

        for cell in cells {
            if cell == braille::SPACE {
                if !current_word.is_empty() {
                    words.push(current_word);
                    current_word = Vec::new();
                }
            } else {
                current_word.push(cell);
            }
        }
        if !current_word.is_empty() {
            words.push(current_word);
        }
        if words.is_empty() {
            words.push(Vec::new());
        }
        result_lines.push(words);
    }
    if result_lines.is_empty() {
        result_lines.push(vec![Vec::new()]);
    }
    result_lines
}

use sdk::applet::SpeechResult;

struct ManualApplet {
    text_area: TextArea,
    container: widget::Container,
    lines: Vec<String>,
    current_line_idx: usize,
}

impl Default for ManualApplet {
    fn default() -> Self {
        Self {
            text_area: TextArea::new(sdk::api::display::BoundsRect::default()),
            container: widget::Container::default(),
            lines: Vec::new(),
            current_line_idx: 0,
        }
    }
}

impl Applet for ManualApplet {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        log::info!("장치 사용자 매뉴얼 애플릿을 시작합니다.");

        self.container.set_bounds(context.window.bounds_rect);
        self.container.set_border(1, Intensity::MAX);
        self.container.set_padding(Spacing::all(1));

        self.text_area
            .set_bounds(self.container.content_bounds());
        self.text_area.set_word_wrap(true);
        self.text_area.set_view_mode(true); // 커서 숨김 및 탐색 뷰 모드

        self.lines = MANUAL_TEXT.lines().map(|s| s.trim().to_string()).collect();
        self.current_line_idx = 0;
        self.text_area.lines = text_to_braille_lines(MANUAL_TEXT);

        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;

        if self.text_area.on_update(context, true)? == widget::WidgetUpdateResult::NeedsRedraw {
            needs_redraw = true;
        }

        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Up => {
                        if self.current_line_idx > 0 {
                            self.current_line_idx -= 1;
                        }
                        self.text_area.scroll_up();
                        needs_redraw = true;
                    }
                    KeyCode::Down => {
                        if !self.lines.is_empty() && self.current_line_idx + 1 < self.lines.len() {
                            self.current_line_idx += 1;
                        }
                        self.text_area.scroll_down();
                        needs_redraw = true;
                    }
                    KeyCode::Left => {
                        self.text_area.scroll_left();
                        needs_redraw = true;
                    }
                    KeyCode::Right => {
                        self.text_area.scroll_right();
                        needs_redraw = true;
                    }
                    _ => {}
                }
            }
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        self.container.on_draw(canvas)?;
        self.text_area.on_draw(canvas)?;
        Ok(())
    }

    fn on_speech(&self, _context: &Context) -> SpeechResult {
        if self.current_line_idx < self.lines.len() {
            let line = &self.lines[self.current_line_idx];
            if line.is_empty() {
                SpeechResult::text("빈 줄")
            } else {
                SpeechResult::text(line.as_str())
            }
        } else {
            SpeechResult::None
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(ManualApplet::default()));
}
