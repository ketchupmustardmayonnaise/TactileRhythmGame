use i18n::num; // 다국어 공통 일반 숫자 변환 함수 추가
use sdk::Language;
use sdk::Result;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::api::log as sdk_log;
use sdk::api::settings::{AUDIO_VOLUME, LANGUAGE, SPEECH_RATE, Settings};
use sdk::applet::Applet;
use sdk::event::UpdateResult;
use sdk::tts_segment;

const UNSELECTED_BOX_INTENSITY: u8 = 255;
const UNSELECTED_VAL_INTENSITY: u8 = 255;
const ITEM_HEIGHT: i16 = 10;

fn translate_menu_name(name: &str, lang: Language) -> &str {
    match name {
        LANGUAGE => match lang {
            Language::Ko => "언어",
            _ => "Language",
        },
        AUDIO_VOLUME => match lang {
            Language::Ko => "볼륨",
            _ => "Volume",
        },
        SPEECH_RATE => match lang {
            Language::Ko => "속도",
            _ => "Speed",
        },
        _ => name,
    }
}

fn translate_value_name(menu_name: &str, val: &str, display_name: &str, lang: Language) -> String {
    match menu_name {
        LANGUAGE => match val {
            "0" => match lang {
                Language::Ko => "한국어",
                _ => "Korean",
            },
            "1" => match lang {
                Language::Ko => "영어",
                _ => "English",
            },
            _ => display_name,
        }
        .to_string(),
        AUDIO_VOLUME => {
            // 오디오 볼륨값(0~10 등)을 숫자로 파싱하여 공통 사전 모듈 num(lang, n) 함수를 사용하여 매끄럽게 번역합니다.
            if let Ok(n) = val.parse::<u32>() {
                num(lang, n)
            } else {
                display_name.to_string()
            }
        }
        SPEECH_RATE => match val {
            "0" => match lang {
                Language::Ko => "느리게".to_string(),
                _ => "Slow".to_string(),
            },
            "1" => match lang {
                Language::Ko => "보통".to_string(),
                _ => "Normal".to_string(),
            },
            "2" => match lang {
                Language::Ko => "빠르게".to_string(),
                _ => "Fast".to_string(),
            },
            _ => display_name.to_string(),
        },
        _ => display_name.to_string(),
    }
}

/// 문자열(str)을 인자로 받아 그에 해당하는 메뉴 항목을 상자로 그리고,
/// 안쪽에 상세 설정 값(values)을 작은 상자로 그리는 함수입니다.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu_box(
    canvas: &mut dyn DisplayInterface,
    _text: &str,
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    is_selected: bool,
    num_values: usize,
    current_value_idx: usize,
) {
    let intensity = if is_selected {
        Intensity::MAX
    } else {
        Intensity::new(UNSELECTED_BOX_INTENSITY)
    };

    // 메뉴 아이템 배경 그리기
    if is_selected {
        for j in y..y + height {
            for i in x..x + width {
                canvas.set_pin(Point::new(i, j), intensity);
            }
        }
    } else {
        for i in x..x + width {
            canvas.set_pin(Point::new(i, y), intensity);
            canvas.set_pin(Point::new(i, y + height - 1), intensity);
        }
        for j in y..y + height {
            canvas.set_pin(Point::new(x, j), intensity);
            canvas.set_pin(Point::new(x + width - 1, j), intensity);
        }
    }

    // 안쪽 values를 나타내는 작은 상자들 그리기
    if num_values > 0 {
        let inner_x = x + 2;
        let inner_y = y + 2;
        let inner_w = width - 4;
        let inner_h = height - 4;

        if inner_w > 0 && inner_h > 0 {
            let box_w = (inner_w / num_values as i16).max(2);

            for v in 0..num_values {
                let vx = inner_x + (v as i16) * box_w;
                let vw = box_w - 1;

                let is_val_selected = v == current_value_idx;

                let val_intensity = if is_selected {
                    Intensity::OFF
                } else {
                    Intensity::new(UNSELECTED_VAL_INTENSITY)
                };

                for j in inner_y..inner_y + inner_h {
                    for i in vx..vx + vw {
                        if i < inner_x + inner_w {
                            if is_val_selected
                                || j == inner_y
                                || j == inner_y + inner_h - 1
                                || i == vx
                                || i == vx + vw - 1
                            {
                                canvas.set_pin(Point::new(i, j), val_intensity);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct SettingsApp {
    setting: Settings,
    selected_index: usize,
    menu_items: Vec<&'static str>,
    setting_display_names: Vec<Vec<String>>,
    setting_values: Vec<Vec<String>>,
    current_value_indices: Vec<usize>,
    menu_name: String,
    val_name: String,
}

impl Default for SettingsApp {
    fn default() -> Self {
        Self {
            setting: Settings::default(),
            selected_index: 0,
            menu_items: vec![AUDIO_VOLUME, LANGUAGE, SPEECH_RATE],
            setting_display_names: vec![
                vec!["10".to_string(), "20".to_string(), "30".to_string(), "40".to_string(), "50".to_string(), "60".to_string(), "70".to_string(), "80".to_string(), "90".to_string(), "100".to_string()],
                vec!["한국어".to_string(), "영어".to_string()],
                vec!["느리게".to_string(), "보통".to_string(), "빠르게".to_string()],
            ],
            setting_values: vec![
                vec!["10".to_string(), "20".to_string(), "30".to_string(), "40".to_string(), "50".to_string(), "60".to_string(), "70".to_string(), "80".to_string(), "90".to_string(), "100".to_string()],
                vec!["0".to_string(), "1".to_string()],
                vec!["0".to_string(), "1".to_string(), "2".to_string()],
            ],

            current_value_indices: vec![0, 0, 0],
            menu_name: "".to_string(),
            val_name: "".to_string(),
        }
    }
}

impl SettingsApp {
    pub fn new() -> Self {
        Self::default()
    }

    fn update_names(&mut self, language: Language) {
        let idx = self.selected_index;
        let val_idx = self.current_value_indices[idx];
        let menu_key = self.menu_items[idx];

        let new_val_name = translate_value_name(
            menu_key,
            &self.setting_values[idx][val_idx],
            &self.setting_display_names[idx][val_idx],
            language,
        );

        let menu = translate_menu_name(menu_key, language);
        let new_menu_name = format!("{} 설정", menu);

        self.val_name = new_val_name;
        self.menu_name = new_menu_name;
    }

    /// 현재 선택된 값을 호스트 설정에 즉시 적용
    fn apply_current_setting(&mut self, context: &mut Context) {
        let idx = self.selected_index;
        let val_idx = self.current_value_indices[idx];
        let menu_key = self.menu_items[idx];
        let val_str = self.setting_values[idx][val_idx].clone();

        match self.setting.set_value(menu_key, &val_str) {
            Ok(_) => {
                if menu_key == LANGUAGE {
                    context.language = match val_str.as_str() {
                        "0" => Language::Ko,
                        "1" => Language::En,
                        _ => Language::Ko,
                    };
                }
                self.update_names(context.language);
                log::info!("'{}' 설정이 '{}'(으)로 즉시 적용되었습니다.", menu_key, val_str);
            }
            Err(e) => {
                log::error!("'{}' 설정 적용 실패: {:?}", menu_key, e);
            }
        }
    }
}

impl Applet for SettingsApp {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk_log::init();
        log::info!("설정 앱을 시작합니다.");

        // 1. 호스트에서 지원하는 설정 옵션 스키마 리스트 불러오기 (일본어 값 "2" 제외)
        match self.setting.get_options() {
            Ok(options) => {
                for (i, menu_item) in self.menu_items.iter().enumerate() {
                    if let Some(option) = options.items.get(*menu_item)
                        && !option.value_list.is_empty()
                    {
                        let mut display_names = Vec::new();
                        let mut values = Vec::new();
                        for (k, v) in &option.value_list {
                            if *menu_item == LANGUAGE && (v == "2" || k.contains("Japanese") || k.contains("日本語")) {
                                continue;
                            }
                            display_names.push(k.clone());
                            values.push(v.clone());
                        }
                        if !values.is_empty() {
                            self.setting_display_names[i] = display_names;
                            self.setting_values[i] = values;
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("설정 목록 옵션을 불러오지 못했습니다: {:?}", e);
            }
        }

        // 2. 기존에 저장된 값들을 읽어와서 current_value_indices를 설정
        for (i, key) in self.menu_items.iter().enumerate() {
            if let Ok(val) = self.setting.get_value(*key)
                && let Some(pos) = self.setting_values[i].iter().position(|x| x == &val)
            {
                self.current_value_indices[i] = pos;
            }
        }

        let menu_key = self.menu_items[self.selected_index];
        let val_key = &self.setting_values[self.selected_index]
            [self.current_value_indices[self.selected_index]];
        let display_name = &self.setting_display_names[self.selected_index]
            [self.current_value_indices[self.selected_index]];

        let menu = translate_menu_name(menu_key, context.language);
        self.val_name = translate_value_name(menu_key, val_key, display_name, context.language);
        self.menu_name = format!("{} 설정", menu);

        let segments = match context.language {
            Language::Ko => tts_segment!(
                "설정 앱을 시작합니다. 위 아래 방향키로 설정 항목을 고르고 좌우 방향키로 값을 변경하세요. 첫 번째 항목, ",
                self.menu_name,
                ", 현재 값 ",
                self.val_name,
                "."
            ),
            _ => tts_segment!(
                "Starting Settings app. Use Up/Down to select items, Left/Right to change values. First item: ",
                self.menu_name,
                ", Current value: ",
                self.val_name,
                "."
            ),
        }
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();

        context
            .audio
            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Up if self.selected_index > 0 => {
                        self.selected_index -= 1;
                        self.update_names(context.language);

                        log::info!("{}", self.menu_name);
                        let segments = tts_segment!(self.menu_name)
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect::<Vec<_>>();
                        context
                            .audio
                            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.selected_index + 1 < self.menu_items.len() => {
                        self.selected_index += 1;
                        self.update_names(context.language);

                        log::info!("{}", self.menu_name);
                        let segments = tts_segment!(self.menu_name)
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect::<Vec<_>>();
                        context
                            .audio
                            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                        needs_redraw = true;
                    }
                    KeyCode::Left => {
                        if self.current_value_indices[self.selected_index] > 0 {
                            self.current_value_indices[self.selected_index] -= 1;
                        } else {
                            self.current_value_indices[self.selected_index] = self.setting_values
                                [self.selected_index]
                                .len()
                                .saturating_sub(1);
                        }
                        // 좌우 방향키 이동 시 즉시 설정 적용
                        self.apply_current_setting(context);

                        log::info!("{}으로 값 변경 및 적용됨", self.val_name);
                        let segments = tts_segment!(self.val_name)
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect::<Vec<_>>();
                        context
                            .audio
                            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                        needs_redraw = true;
                    }
                    KeyCode::Right => {
                        if self.current_value_indices[self.selected_index]
                            < self.setting_values[self.selected_index]
                                .len()
                                .saturating_sub(1)
                        {
                            self.current_value_indices[self.selected_index] += 1;
                        } else {
                            self.current_value_indices[self.selected_index] = 0;
                        }
                        // 좌우 방향키 이동 시 즉시 설정 적용
                        self.apply_current_setting(context);

                        log::info!("{}으로 값 변경 및 적용됨", self.val_name);
                        let segments = tts_segment!(self.val_name)
                            .into_iter()
                            .map(AudioSegment::Text)
                            .collect::<Vec<_>>();
                        context
                            .audio
                            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());
                        needs_redraw = true;
                    }
                    KeyCode::Center => {
                        self.apply_current_setting(context);

                        let segments = match context.language {
                            Language::Ko => tts_segment!(
                                self.menu_name,
                                " 이 ",
                                self.val_name,
                                " 값으로 설정되었습니다"
                            ),
                            _ => {
                                tts_segment!(self.menu_name, " set to ", self.val_name)
                            }
                        }
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                        context.audio.play_audio_sequence(
                            &segments,
                            sdk::applet::SpeechOption::new(),
                        );
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
        canvas.clear();
        let display_size = canvas.get_size();

        let item_height = ITEM_HEIGHT;
        let spacing = 2_i16;
        let item_step = item_height + spacing;

        let visible_items = (display_size.height / item_step).max(1);
        let scroll_offset = if (self.selected_index as i16) >= visible_items {
            ((self.selected_index as i16) - visible_items + 1) * item_step
        } else {
            0
        };

        let total_items = self.menu_items.len();

        for i in 0..total_items {
            let start_y = (i as i16) * item_step - scroll_offset;
            let end_y = start_y + item_height;

            if end_y <= 0 {
                continue;
            }
            if start_y >= display_size.height {
                break;
            }

            let is_selected = i == self.selected_index;
            let x = 1;
            let width = display_size.width - 2;
            let height = item_height;

            let item_text = self.menu_items[i];
            let num_values = self.setting_values[i].len();
            let current_val_idx = self.current_value_indices[i];

            draw_menu_box(
                canvas,
                item_text,
                x,
                start_y,
                width,
                height,
                is_selected,
                num_values,
                current_val_idx,
            );
        }

        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let app = Box::new(SettingsApp::default());
    sdk::applet::run(app);
}
