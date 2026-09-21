use braille::TextArea;
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use widget::{Spacing, Widget};

const CHARTER_TEXT: &str = r#"국민 교육 헌장

우리는 민족 중흥의 역사적 사명을 띠고 이 땅에 태어났다.
조상의 빛난 헐을 오늘에 되살려, 안으로 자주독립의 자세를 확립하고, 밖으로 인류 공영에 이바지할 때다.
이에, 우리의 나아갈 바를 밝혀 교육의 지표로 삼는다.

성실한 마음과 튼튼한 몸으로, 학문과 기술을 배우고 익히며, 타고난 저마다의 소질을 개발하여, 우리의 삶을 개척하고 공공의 안녕과 질서를 지키며, 창조의 힘과 배려의 정신을 가꾸어, 나라의 발전에 이바지한다.

아무런 사심 없는 협동심과 애국심을 바탕으로, 책임과 의무를 다하며, 솔선수범하는 민주 시민의 자질을 함양하여, 복지 국가 건설에 기여한다.

반공 민주 정신에 투철한 애국 애족이 우리의 삶의 길이며, 자유 세계의 인류 평화를 이루는 터전이다.
길게 흥할 우리의 미래를 약속하는 신념과 용기를 가지고, 힘차게 나아가자.

1968년 12월 5일"#;

/// 6점 점자 셀 비트마스크(u8)를 BRF (ASCII Braille) 문자로 변환
fn cell_to_brf_char(cell: u8) -> char {
    match cell & 0b111111 {
        0b000000 => ' ',
        0b000001 => 'A',
        0b000010 => ',',
        0b000011 => 'B',
        0b000100 => '\'',
        0b000101 => 'K',
        0b000110 => ';',
        0b000111 => 'L',
        0b001000 => '@',
        0b001001 => 'C',
        0b001010 => 'I',
        0b001011 => 'F',
        0b001100 => '/',
        0b001101 => 'M',
        0b001110 => 'S',
        0b001111 => 'P',
        0b010000 => '"',
        0b010001 => 'E',
        0b010010 => ':',
        0b010011 => 'H',
        0b010100 => '*',
        0b010101 => 'O',
        0b010110 => '!',
        0b010111 => 'R',
        0b011000 => '^',
        0b011001 => 'D',
        0b011010 => 'J',
        0b011011 => 'G',
        0b011100 => '>',
        0b011101 => 'N',
        0b011110 => 'T',
        0b011111 => 'Q',
        0b100000 => '`',
        0b100001 => 'a',
        0b100010 => 'b',
        0b100011 => 'c',
        0b100100 => 'd',
        0b100101 => 'U',
        0b100110 => 'f',
        0b100111 => 'V',
        0b101000 => 'h',
        0b101001 => '%',
        0b101010 => '[',
        0b101011 => '$',
        0b101100 => '+',
        0b101101 => 'X',
        0b101110 => '!',
        0b101111 => '&',
        0b110000 => '0',
        0b110001 => '1',
        0b110010 => '.',
        0b110011 => '\\',
        0b110100 => '4',
        0b110101 => 'Z',
        0b110110 => '6',
        0b110111 => '(',
        0b111000 => '_',
        0b111001 => '?',
        0b111010 => 'W',
        0b111011 => ']',
        0b111100 => '#',
        0b111101 => 'Y',
        0b111110 => ')',
        0b111111 => '=',
        _ => ' ',
    }
}

/// BRF (ASCII Braille) 문자를 6점 점자 셀 비트마스크(u8)로 변환
fn brf_char_to_cell(c: char) -> u8 {
    match c {
        ' ' => 0b000000,
        'A' | 'a' => 0b000001,
        ',' => 0b000010,
        'B' | 'b' => 0b000011,
        '\'' => 0b000100,
        'K' | 'k' => 0b000101,
        ';' => 0b000110,
        'L' | 'l' => 0b000111,
        '@' => 0b001000,
        'C' | 'c' => 0b001001,
        'I' | 'i' => 0b001010,
        'F' | 'f' => 0b001011,
        '/' => 0b001100,
        'M' | 'm' => 0b001101,
        'S' | 's' => 0b001110,
        'P' | 'p' => 0b001111,
        '"' => 0b010000,
        'E' | 'e' => 0b010001,
        ':' => 0b010010,
        'H' | 'h' => 0b010011,
        '*' => 0b010100,
        'O' | 'o' => 0b010101,
        '!' => 0b010110,
        'R' | 'r' => 0b010111,
        '^' => 0b011000,
        'D' | 'd' => 0b011001,
        'J' | 'j' => 0b011010,
        'G' | 'g' => 0b011011,
        '>' => 0b011100,
        'N' | 'n' => 0b011101,
        'T' | 't' => 0b011110,
        'Q' | 'q' => 0b011111,
        '`' => 0b100000,
        'U' | 'u' => 0b100101,
        'V' | 'v' => 0b100111,
        '%' => 0b101001,
        '[' => 0b101010,
        '$' => 0b101011,
        '+' => 0b101100,
        'X' | 'x' => 0b101101,
        '&' => 0b101111,
        '\\' => 0b110011,
        'Z' | 'z' => 0b110101,
        '(' => 0b110111,
        '_' => 0b111000,
        '?' => 0b111001,
        'W' | 'w' => 0b111010,
        ']' => 0b111011,
        '#' => 0b111100,
        'Y' | 'y' => 0b111101,
        ')' => 0b111110,
        '=' => 0b111111,
        _ => 0b000000,
    }
}

/// 텍스트를 BRF 포맷 문자열로 변환
fn text_to_brf(text: &str) -> String {
    let mut brf_lines = Vec::new();
    for line in text.lines() {
        let cells = braille::text_to_braille(line);
        let brf_line: String = cells.into_iter().map(cell_to_brf_char).collect();
        brf_lines.push(brf_line);
    }
    brf_lines.join("\n")
}

/// BRF 포맷 문자열을 TextArea의 lines 구조(Vec<Vec<Vec<u8>>>)로 파싱
fn parse_brf_to_lines(brf: &str) -> Vec<Vec<Vec<u8>>> {
    let mut result_lines = Vec::new();
    for line_str in brf.lines() {
        let mut words = Vec::new();
        let mut current_word = Vec::new();

        for c in line_str.chars() {
            if c == ' ' {
                if !current_word.is_empty() {
                    words.push(current_word);
                    current_word = Vec::new();
                }
            } else {
                let cell = brf_char_to_cell(c);
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

struct CharterApplet {
    text_area: TextArea,
    container: widget::Container,
    lines: Vec<String>,
    current_line_idx: usize,
}

impl Default for CharterApplet {
    fn default() -> Self {
        Self {
            text_area: TextArea::new(sdk::api::display::BoundsRect::default()),
            container: widget::Container::default(),
            lines: Vec::new(),
            current_line_idx: 0,
        }
    }
}

impl Applet for CharterApplet {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        log::info!("국민 교육 헌장 점자책 애플릿을 시작합니다.");

        self.container.set_bounds(context.window.bounds_rect);
        self.container.set_border(1, Intensity::MAX);
        self.container.set_padding(Spacing::all(1));

        self.text_area
            .set_bounds(self.container.content_bounds());
        self.text_area.set_word_wrap(true);
        self.text_area.set_view_mode(true); // 뷰 모드 활성화 (커서 숨김, 수동 스크롤)

        self.lines = CHARTER_TEXT.lines().map(|s| s.trim().to_string()).collect();
        self.current_line_idx = 0;

        // 국민 교육 헌장 텍스트를 BRF 포맷으로 변환 후 TextArea에 설정
        let brf_content = text_to_brf(CHARTER_TEXT);
        self.text_area.lines = parse_brf_to_lines(&brf_content);

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
    sdk::run(Box::new(CharterApplet::default()));
}

