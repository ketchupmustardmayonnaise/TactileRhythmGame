use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Intensity};
use sdk::api::keypad::{
    KeyCode, KeyState, KeypadBrailleChordPopResult, KeypadPopResult, KeypadSide,
};
use sdk::applet::SpeechResult;
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::types::Language;
use widget::{Spacing, Widget};

/// 게임 메시지를 정의하는 열거형
enum ModeTtsMessage {
    Insert,
    Edit,
    View,
}

impl ModeTtsMessage {
    fn text(&self, lang: Language) -> &'static str {
        match self {
            ModeTtsMessage::Insert => match lang {
                Language::Ko => "인서트 모드",
                Language::En => "Insert mode",
                Language::Ja => "インサートモード",
            },
            ModeTtsMessage::Edit => match lang {
                Language::Ko => "편집 모드",
                Language::En => "Edit mode",
                Language::Ja => "編集モード",
            },
            ModeTtsMessage::View => match lang {
                Language::Ko => "뷰 모드",
                Language::En => "View mode",
                Language::Ja => "ビューモード",
            },
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum MemoMode {
    Insert,
    Edit,
    View,
}

impl MemoMode {
    fn next(&self) -> Self {
        match self {
            MemoMode::Insert => MemoMode::Edit,
            MemoMode::Edit => MemoMode::View,
            MemoMode::View => MemoMode::Insert,
        }
    }
}

/// 게임의 상태를 관리하는 구조체.
/// 디스플레이, 키패드 및 커서의 현재 위치를 포함합니다.
struct MemoApplet {
    mode: MemoMode,
    center_pressed: bool,
    center_modifier_used: bool,
    text_area: braille::TextArea,
    container: widget::Container,
}

impl Default for MemoApplet {
    fn default() -> Self {
        Self {
            mode: MemoMode::Insert, // 앱이 실행될 때 기본적으로 점자 입력 모드로 시작합니다.
            center_pressed: false,
            center_modifier_used: false,
            text_area: braille::TextArea::new(BoundsRect::default()),
            container: widget::Container::default(),
        }
    }
}

/// `Applet` 트레이트를 구현하여 게임을 애플릿으로 실행할 수 있도록 합니다.
impl Applet for MemoApplet {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();

        log::info!("on_start: {:?}", context.info);

        context
            .keypad
            .set_perkins_mode(self.mode == MemoMode::Insert);

        self.container.set_bounds(context.window.bounds_rect);
        self.container.set_border(1, Intensity::MAX);
        self.container.set_padding(Spacing::all(1));

        self.text_area
            .set_bounds(self.container.content_bounds());

        // 저장된 기존 점자 텍스트가 있다면 불러오기
        if let Ok(bytes) = context.preferences.get_bytes("memo_text")
            && let Some(lines) = deserialize_lines(&bytes)
            && !lines.is_empty()
        {
            self.text_area.lines = lines;
            // 커서를 마지막 텍스트의 끝 지점으로 이동
            self.text_area.cursor_line = self.text_area.lines.len().saturating_sub(1);
            if let Some(last_line) = self.text_area.lines.last() {
                self.text_area.cursor_word = last_line.len().saturating_sub(1);
                if let Some(last_word) = last_line.last() {
                    self.text_area.cursor_char = last_word.len();
                }
            }
        }

        Ok(())
    }

    fn on_stop(&mut self, context: &mut Context) -> Result<()> {
        log::info!("on_stop");

        // 애플릿 종료 시 현재 작성된 텍스트 상태를 preferences에 영구 저장
        let bytes = serialize_lines(&self.text_area.lines);
        let _ = context.preferences.set_bytes("memo_text", &bytes);

        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;
        // 1. 위젯 업데이트 (텍스트 입력, 지우기, 줄바꿈 등을 내부적으로 처리합니다)
        if self.text_area.on_update(context, true)? == widget::WidgetUpdateResult::NeedsRedraw
        {
            needs_redraw = true;
        }

        // 2. 점자 조합 이벤트 처리 (입력 모드일 때만 적용)
        while let KeypadBrailleChordPopResult::BrailleChord(chord) =
            context.keypad.pop_braille_chord()
        {
            if self.mode == MemoMode::Insert {
                self.text_area.push_chord(chord);
                self.text_area.reset_cursor_blink();
                needs_redraw = true;
            }
        }

        // 3. 키패드 이벤트 처리 (모드 전환 및 특수 키 조작)
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.code == KeyCode::Center {
                self.center_pressed = event.state == KeyState::Pressed;
                if event.state == KeyState::Pressed {
                    self.center_modifier_used = false; // 새로 눌리면 초기화
                }
            }

            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Function => {
                        self.mode = self.mode.next();
                        context
                            .keypad
                            .set_perkins_mode(self.mode == MemoMode::Insert);

                        self.text_area.set_view_mode(self.mode == MemoMode::View);
                        needs_redraw = true;
                    }
                    KeyCode::Center if self.mode == MemoMode::Insert => {
                        self.text_area.push_space();
                        self.text_area.reset_cursor_blink();
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.mode == MemoMode::Insert => {
                        if event.side == KeypadSide::Left {
                            self.text_area.backspace();
                        } else {
                            self.text_area.push_newline();
                        }
                        self.text_area.reset_cursor_blink();
                        needs_redraw = true;
                    }

                    KeyCode::Left if self.mode == MemoMode::Edit => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.move_cursor_prev_word();
                        } else {
                            self.text_area.move_cursor_left();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Right if self.mode == MemoMode::Edit => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.move_cursor_next_word();
                        } else {
                            self.text_area.move_cursor_right();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Up if self.mode == MemoMode::Edit => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.move_cursor_line_start();
                        } else {
                            self.text_area.move_cursor_up();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.mode == MemoMode::Edit => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.move_cursor_line_end();
                        } else {
                            self.text_area.move_cursor_down();
                        }
                        needs_redraw = true;
                    }

                    KeyCode::Up if self.mode == MemoMode::View => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.scroll_page_up();
                        } else {
                            self.text_area.scroll_up();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.mode == MemoMode::View => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.scroll_page_down();
                        } else {
                            self.text_area.scroll_down();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Left if self.mode == MemoMode::View => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.scroll_to_start();
                        } else {
                            self.text_area.scroll_left();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Right if self.mode == MemoMode::View => {
                        if self.center_pressed {
                            self.center_modifier_used = true;
                            self.text_area.scroll_to_end();
                        } else {
                            self.text_area.scroll_right();
                        }
                        needs_redraw = true;
                    }
                    _ => {}
                }
            } else if event.state == KeyState::Released {
                // Center 키 단독 사용 후 떼었을 때 삭제 동작 수행 (편집 모드)
                if event.code == KeyCode::Center
                    && self.mode == MemoMode::Edit
                    && !self.center_modifier_used
                {
                    self.text_area.delete();
                    needs_redraw = true;
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
        // 배경, 테두리 등 컨테이너 렌더링을 직접 수행
        self.container.on_draw(canvas)?;
        self.text_area.on_draw(canvas)?;

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        let msg = match self.mode {
            MemoMode::Insert => ModeTtsMessage::Insert.text(context.language),
            MemoMode::Edit => ModeTtsMessage::Edit.text(context.language),
            MemoMode::View => ModeTtsMessage::View.text(context.language),
        };
        SpeechResult::text(msg)
    }
}

/// 게임 애플릿의 진입점입니다.
/// 이 함수는 `multiline-braille-display` 런타임에 의해 호출됩니다.
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(MemoApplet::default())); // 게임 애플릿 실행
}

/// 점자 텍스트 영역(lines)을 바이트 배열로 직렬화합니다.
fn serialize_lines(lines: &[Vec<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(lines.len() as u32).to_le_bytes());
    for line in lines {
        bytes.extend_from_slice(&(line.len() as u32).to_le_bytes());
        for word in line {
            bytes.extend_from_slice(&(word.len() as u32).to_le_bytes());
            bytes.extend_from_slice(word);
        }
    }
    bytes
}

/// 저장된 바이트 배열에서 점자 텍스트 영역(lines)을 역직렬화합니다.
fn deserialize_lines(bytes: &[u8]) -> Option<Vec<Vec<Vec<u8>>>> {
    let mut cursor = 0;
    let read_u32 = |c: &mut usize| -> Option<u32> {
        if *c + 4 <= bytes.len() {
            let val = u32::from_le_bytes(bytes[*c..*c + 4].try_into().unwrap());
            *c += 4;
            Some(val)
        } else {
            None
        }
    };

    let num_lines = read_u32(&mut cursor)?;
    if num_lines > 10000 {
        return None;
    } // 비정상적인 데이터로 인한 메모리 폭주 방지

    let mut lines = Vec::new();
    for _ in 0..num_lines {
        let num_words = read_u32(&mut cursor)?;
        if num_words > 10000 {
            return None;
        }
        let mut line = Vec::new();
        for _ in 0..num_words {
            let num_chords = read_u32(&mut cursor)?;
            if num_chords > 10000 {
                return None;
            }

            if cursor + (num_chords as usize) <= bytes.len() {
                line.push(bytes[cursor..cursor + (num_chords as usize)].to_vec());
                cursor += num_chords as usize;
            } else {
                return None;
            }
        }
        lines.push(line);
    }
    Some(lines)
}
