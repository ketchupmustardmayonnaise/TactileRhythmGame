use braille::{Alignment, Label, render_text};
use sdk::Applet;
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadEvent, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use widget::Widget;

const SAMPLES: &[&str] = &[
    "2024 개정 한국 점자 규정 (braillify 수동 스크롤 테스트)",
    "안녕하세요! 다자 점자 디스플레이 수동 스크롤 기능 검증 문구입니다.",
    "Hello World! This is a long English text to test manual scrolling on Label.",
    "1234567890 !@#$%^&*()",
];

pub struct BrailleTestApplet {
    sample_index: usize,
    label: Label,
    need_draw: bool,
}

impl Default for BrailleTestApplet {
    fn default() -> Self {
        let initial_text = SAMPLES[0];
        let label = Label::new(initial_text).align(Alignment::Start);

        Self {
            sample_index: 0,
            label,
            need_draw: true,
        }
    }
}

impl BrailleTestApplet {
    fn set_sample(&mut self, index: usize, size: sdk::api::display::Size) {
        self.sample_index = index;
        let text = SAMPLES[index];

        self.label.set_text(text);
        self.label
            .set_bounds(BoundsRect::new(Point::new(0, 8), size));

        self.need_draw = true;
    }
}

impl Applet for BrailleTestApplet {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        self.need_draw = true;
        let size = context.window.get_size();
        self.set_sample(0, size);

        context.audio.speak_text(
            "점자 라벨 수동 스크롤 테스트 애플릿입니다. 좌우 키로 수동 스크롤하고 위아래 키로 문구를 변경하세요.",
        );
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let size = context.window.get_size();
        let mut updated = false;

        loop {
            match context.keypad.pop_event() {
                KeypadPopResult::NoEvent => break,
                KeypadPopResult::Event(KeypadEvent { code, state, .. }) => {
                    if state == KeyState::Pressed {
                        match code {
                            KeyCode::Up => {
                                let new_idx = if self.sample_index == 0 {
                                    SAMPLES.len() - 1
                                } else {
                                    self.sample_index - 1
                                };
                                self.set_sample(new_idx, size);
                                context.audio.speak_text(SAMPLES[new_idx]);
                                updated = true;
                            }
                            KeyCode::Down => {
                                let new_idx = (self.sample_index + 1) % SAMPLES.len();
                                self.set_sample(new_idx, size);
                                context.audio.speak_text(SAMPLES[new_idx]);
                                updated = true;
                            }
                            KeyCode::Left => {
                                // 수동 스크롤: 왼쪽으로 스크롤 (이전 텍스트 위치 보기)
                                self.label.scroll_left(size.width);
                                updated = true;
                            }
                            KeyCode::Right => {
                                // 수동 스크롤: 오른쪽으로 스크롤 (다음 텍스트 위치 보기)
                                self.label.scroll_right(size.width);
                                updated = true;
                            }
                            KeyCode::Center => {
                                context.audio.speak_text(SAMPLES[self.sample_index]);
                                updated = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let widget_update = self.label.on_update(context, false)?;
        if updated || self.need_draw || widget_update == widget::WidgetUpdateResult::NeedsRedraw {
            self.need_draw = false;
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        // 1. 상단 고정 render_text 테스트 (y = 0)
        render_text(canvas, SAMPLES[self.sample_index], Point::new(0, 0));

        // 2. 수동 스크롤 Label 위젯 렌더링 테스트 (y = 8)
        self.label.on_draw(canvas)?;

        Ok(())
    }

    fn on_help(&self, _context: &Context) -> sdk::applet::LocalizedString {
        sdk::applet::LocalizedString {
            ko: "점자 테스트 애플릿 도움말입니다. 위아래 키로 샘플을 변경하고, 좌우 키로 점자 라벨을 수동 스크롤합니다.".to_string(),
            en: "Braille test applet help. Use Up and Down keys to change samples, and Left and Right keys to manually scroll.".to_string(),
            ja: "点字テストアプリのヘルプです。上下キーでサンプルを変更し、左右キーで手동スクロールします。".to_string(),
        }
    }
}


#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(BrailleTestApplet::default()));
}
