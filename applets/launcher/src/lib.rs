use braille;
use graphics::Graphics;
use sdk::Language;
use sdk::Result;
use sdk::api::applet_manager::AppletManager;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::api::log as sdk_log;
use sdk::applet::{Applet, AppletInfo, LocalizedString, SpeechOption, SpeechResult};
use sdk::event::UpdateResult;
use sdk::tts_segment;

use views::canvas::PixelBuffer;
use views::viewport::Viewport;

const UNSELECTED_BOX_INTENSITY: u8 = 80;
const GRID_COLUMNS: i16 = 3;
const BOX_SIZE: i16 = 15;
const SPACING: i16 = 1;
const ITEM_STEP_Y: i16 = BOX_SIZE + SPACING;
const BORDER_THICKNESS: i16 = 1;
const ICON_PADDING: i16 = 1;
const HEADER_HEIGHT: i16 = 7;
const HEADER_MARGIN_BOTTOM: i16 = 0;
const SECTION_GAP: i16 = 0;

/// 스크롤 애니메이션 부드러움 조절 값
const LERP_FACTOR: f32 = 0.2;

#[derive(Debug, Clone)]
enum LauncherItem {
    Category {
        category_name: String,
        applet_count: usize,
        pos: Point,
    },
    Applet {
        applet_index: usize,
        pos: Point,
    },
}

impl LauncherItem {
    fn pos(&self) -> Point {
        match self {
            LauncherItem::Category { pos, .. } => *pos,
            LauncherItem::Applet { pos, .. } => *pos,
        }
    }
}

enum LauncherMessage<'a> {
    LoadError,
    LaunchError,
    LaunchingApp(&'a str),
    CategoryFocused { category: &'a str, count: usize },
    AppFocused(&'a str),
}

fn korean_count_number(n: usize) -> String {
    match n {
        1 => "한".to_string(),
        2 => "두".to_string(),
        3 => "세".to_string(),
        4 => "네".to_string(),
        5 => "다섯".to_string(),
        6 => "여섯".to_string(),
        7 => "일곱".to_string(),
        8 => "여덟".to_string(),
        9 => "아홉".to_string(),
        10 => "열".to_string(),
        11 => "열한".to_string(),
        12 => "열두".to_string(),
        13 => "열세".to_string(),
        14 => "열네".to_string(),
        15 => "열다섯".to_string(),
        16 => "열여섯".to_string(),
        17 => "열일곱".to_string(),
        18 => "열여덟".to_string(),
        19 => "열아홉".to_string(),
        20 => "스무".to_string(),
        _ => n.to_string(),
    }
}

impl<'a> LauncherMessage<'a> {
    pub fn to_segments(&self, lang: Language) -> Vec<AudioSegment> {
        match self {
            LauncherMessage::LoadError => {
                let msg = match lang {
                    Language::Ko => {
                        tts_segment! {"런처 앱을 시작합니다.", " 애플릿 목록을 가져오는데 실패했습니다"}
                    }
                    Language::En => {
                        tts_segment! {"Starting Launcher.", "Failed to load applet list."}
                    }
                    Language::Ja => {
                        tts_segment! {"ランチャーを開始します。", "アプレットリストの取得に失敗しました。"}
                    }
                };
                msg.into_iter().map(AudioSegment::Text).collect()
            }
            LauncherMessage::LaunchError => {
                let msg = match lang {
                    Language::Ko => "앱 실행에 실패했습니다",
                    Language::En => "Failed to launch the app.",
                    Language::Ja => "アプリの実行に失敗しました",
                };
                vec![AudioSegment::Text(msg.to_string())]
            }
            LauncherMessage::LaunchingApp(name) => {
                match lang {
                    Language::Ko => {
                        let word_with_particle = tossicat::postfix(name, "을")
                            .unwrap_or_else(|_| format!("{}을", name));
                        let text = format!("{} 실행합니다.", word_with_particle);
                        vec![AudioSegment::Text(text)]
                    }
                    Language::En => {
                        let raw_segments = tts_segment! {"Launching", *name, "app"};
                        raw_segments.into_iter().map(AudioSegment::Text).collect()
                    }
                    Language::Ja => {
                        let raw_segments = tts_segment! {*name, "アプリを起動します"};
                        raw_segments.into_iter().map(AudioSegment::Text).collect()
                    }
                }
            }


            LauncherMessage::CategoryFocused { category, count } => {
                let count_str = count.to_string();
                let ko_count = korean_count_number(*count);
                match lang {
                    Language::Ko => {
                        vec![AudioSegment::Text(format!(
                            " {} 카테고리, {}개의 앱이 있습니다.",
                            category, ko_count
                        ))]
                    }
                    Language::En => {
                        vec![AudioSegment::Text(format!(
                            " {} category, {} apps available.",
                            category, count_str
                        ))]
                    }
                    Language::Ja => {
                        vec![AudioSegment::Text(format!(
                            " {}カテゴリ、{}個のアプリがあります。",
                            category, count_str
                        ))]
                    }
                }
            }

            LauncherMessage::AppFocused(name) => {
                vec![AudioSegment::Text((*name).to_string())]
            }
        }
    }
}

fn draw_box_on_canvas(
    canvas: &mut PixelBuffer,
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    icon: &str,
) {
    let intensity = Intensity::new(UNSELECTED_BOX_INTENSITY);

    if !icon.is_empty() {
        let offset = BORDER_THICKNESS + ICON_PADDING;
        for (icon_y, line) in icon.lines().enumerate() {
            for (icon_x, ch) in line.chars().enumerate() {
                let pin_intensity = if ch == '1' {
                    Intensity::MAX
                } else {
                    Intensity::new(0)
                };
                let px = x + offset + icon_x as i16;
                let py = y + offset + icon_y as i16;

                if px >= 0 && py >= 0 && px < canvas.width as i16 && py < canvas.height as i16 {
                    canvas.set_pixel(px as usize, py as usize, pin_intensity);
                }
            }
        }
    } else {
        for j in y..y + height {
            for i in x..x + width {
                if j == y
                    || j == y + height - 1
                    || i == x
                    || i == x + width - 1
                        && i >= 0
                        && j >= 0
                        && i < canvas.width as i16
                        && j < canvas.height as i16
                {
                    canvas.set_pixel(i as usize, j as usize, intensity);
                }
            }
        }
    }
}

pub struct LauncherApp {
    applet_manager: AppletManager,
    applets: Vec<AppletInfo>,
    items: Vec<LauncherItem>,
    selected_index: usize,
    canvas: PixelBuffer,
    viewport: Viewport,
    current_scroll_y: f32,
    target_scroll_y: f32,
    item_step_x: i16,
    start_offset_x: i16,
    is_started: bool,
    load_error: bool,
    launching_app_name: Option<String>,
    launch_error: bool,
    speech_description: Option<String>,
    last_frame_time: Option<std::time::Duration>,
}

impl Default for LauncherApp {
    fn default() -> Self {
        Self {
            applet_manager: AppletManager::new(),
            applets: Vec::new(),
            items: Vec::new(),
            selected_index: 0,
            canvas: PixelBuffer::new(1, 1),
            viewport: Viewport::new(Point::new(0, 0), 1, 1),
            current_scroll_y: 0.0,
            target_scroll_y: 0.0,
            item_step_x: 0,
            start_offset_x: 0,
            is_started: true,
            load_error: false,
            launching_app_name: None,
            launch_error: false,
            speech_description: None,
            last_frame_time: None,
        }
    }
}


impl Applet for LauncherApp {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk_log::init();
        log::info!("런처 앱을 시작합니다.");

        match self.applet_manager.list_applets() {
            Ok(mut applets) => {
                fn category_order(category: &str) -> usize {
                    let cat_lower = category.trim().to_lowercase();
                    if cat_lower.contains("system") || cat_lower.contains("시스템") {
                        0
                    } else if cat_lower.contains("utility") || cat_lower.contains("유틸리티") || cat_lower.contains("tool") || cat_lower.contains("도구") {
                        1
                    } else if cat_lower.contains("book") || cat_lower.contains("점자책") {
                        2
                    } else if cat_lower.contains("game") || cat_lower.contains("게임") {
                        3
                    } else if cat_lower.contains("demo") || cat_lower.contains("데모") {
                        4
                    } else if cat_lower.contains("test") || cat_lower.contains("테스트") {
                        5
                    } else {
                        6
                    }
                }

                applets.sort_by(|a, b| {
                    let cat_a = a.category.get_string_by_lang(&context.language);
                    let cat_b = b.category.get_string_by_lang(&context.language);

                    let cat_order_a = category_order(cat_a);
                    let cat_order_b = category_order(cat_b);

                    if cat_order_a != cat_order_b {
                        return cat_order_a.cmp(&cat_order_b);
                    }
                    if cat_a != cat_b {
                        return cat_a.cmp(cat_b);
                    }

                    if a.priority != b.priority {
                        return a.priority.cmp(&b.priority);
                    }

                    let name_a = a.name.get_string_by_lang(&context.language);
                    let name_b = b.name.get_string_by_lang(&context.language);
                    name_a.cmp(name_b)
                });

                log::info!("===== 정렬된 애플릿 목록 =====");
                for (i, applet) in applets.iter().enumerate() {
                    let cat_name = applet.category.get_string_by_lang(&context.language);
                    let display_name = applet.name.get_string_by_lang(&context.language);
                    log::info!(
                        "[{}]: [{}] {} (Priority: {:?}, ID: {})",
                        i,
                        cat_name,
                        display_name,
                        applet.priority,
                        applet.id
                    );
                }
                log::info!("==============================");

                self.applets = applets;

                let display_size = context.window.get_size();

                let total_items_width = GRID_COLUMNS * BOX_SIZE;
                let spacing_x = ((display_size.width) - total_items_width) / (GRID_COLUMNS + 1);
                let spacing_x = spacing_x.max(1);
                self.item_step_x = BOX_SIZE + spacing_x;
                let total_width_with_spacing = total_items_width + spacing_x * (GRID_COLUMNS - 1);
                self.start_offset_x = ((display_size.width) - total_width_with_spacing) / 2;
                self.start_offset_x = self.start_offset_x.max(0);

                // 1. 포커스 가능한 아이템 리스트(카테고리 헤더 + 앱) 및 좌표 생성
                let mut items = Vec::new();
                let mut cat_start_index = 0;
                let mut current_y: i16 = 0;

                while cat_start_index < self.applets.len() {
                    let raw_cat = self.applets[cat_start_index]
                        .category
                        .get_string_by_lang(&context.language);
                    let cat_name = if raw_cat.trim().is_empty() || raw_cat.trim().eq_ignore_ascii_case("unknown") {
                        match context.language {
                            Language::Ko => "기타".to_string(),
                            Language::En => "Others".to_string(),
                            Language::Ja => "その他".to_string(),
                        }
                    } else {
                        raw_cat.to_string()
                    };

                    let mut cat_end_index = cat_start_index;
                    while cat_end_index < self.applets.len()
                        && self.applets[cat_end_index]
                            .category
                            .get_string_by_lang(&context.language)
                            == raw_cat
                    {
                        cat_end_index += 1;
                    }

                    let count = cat_end_index - cat_start_index;

                    // 카테고리 헤더 아이템 추가
                    items.push(LauncherItem::Category {
                        category_name: cat_name.clone(),
                        applet_count: count,
                        pos: Point::new(0, current_y),
                    });

                    current_y += HEADER_HEIGHT + HEADER_MARGIN_BOTTOM;

                    // 해당 카테고리 내 애플릿 아이템 추가
                    for i in 0..count {
                        let applet_idx = cat_start_index + i;
                        let row = (i as i16) / GRID_COLUMNS;
                        let col = (i as i16) % GRID_COLUMNS;

                        let start_x = self.start_offset_x + col * self.item_step_x;
                        let start_y = current_y + row * ITEM_STEP_Y;

                        items.push(LauncherItem::Applet {
                            applet_index: applet_idx,
                            pos: Point::new(start_x, start_y),
                        });
                    }

                    let num_rows = (count as i16 + GRID_COLUMNS - 1) / GRID_COLUMNS;
                    current_y += num_rows * ITEM_STEP_Y;

                    if cat_end_index < self.applets.len() {
                        current_y += SECTION_GAP;
                    }

                    cat_start_index = cat_end_index;
                }

                self.items = items;
                self.selected_index = 0;

                // 2. 캔버스 생성 및 점자 카테고리 텍스트 + 가로선 + 앱 아이콘 렌더링
                let canvas_height = current_y.max(display_size.height);
                self.canvas = PixelBuffer::new(display_size.width as usize, canvas_height as usize);

                for item in &self.items {
                    match item {
                        LauncherItem::Category {
                            category_name, pos, ..
                        } => {
                            // 가로선 앞에 카테고리 텍스트를 점자로 변환하여 출력
                            braille::render_text_with_intensity(
                                &mut self.canvas,
                                category_name,
                                Point::new(2, pos.y + 1),
                                Intensity::MAX,
                            );

                            let braille_w = braille::measure_text(category_name);
                            let line_x_start = (2 + braille_w + 3).min(display_size.width);
                            let line_y = pos.y + 2;
                            let line_intensity = Intensity::MAX;

                            // 점자 텍스트 뒤쪽으로 2pixel 두께, 완전 UP(MAX) 상태의 가로 구분선 렌더링
                            for dy in 0..2 {
                                let y = line_y + dy;
                                for x in line_x_start..display_size.width - 2 {
                                    if y >= 0 && (y as usize) < canvas_height as usize {
                                        self.canvas.set_pixel(
                                            x as usize,
                                            y as usize,
                                            line_intensity,
                                        );
                                    }
                                }
                            }
                        }
                        LauncherItem::Applet { applet_index, pos } => {
                            draw_box_on_canvas(
                                &mut self.canvas,
                                pos.x,
                                pos.y,
                                BOX_SIZE,
                                BOX_SIZE,
                                &self.applets[*applet_index].icon,
                            );
                        }
                    }
                }

                self.viewport = Viewport::new(
                    Point::new(display_size.width / 2, display_size.height / 2),
                    display_size.width,
                    display_size.height,
                );
                self.current_scroll_y = 0.0;
                self.target_scroll_y = 0.0;
                self.last_frame_time = Some(context.time.get_monotonic_time());
            }
            Err(e) => {
                log::error!("애플릿 목록을 가져오는데 실패했습니다: {:?}", e);
                self.load_error = true;
            }
        }
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;

        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            self.launching_app_name = None;
            self.launch_error = false;

            if event.state == KeyState::Pressed {
                self.is_started = false;
                self.speech_description = None;

                let current_pos = if !self.items.is_empty() {
                    self.items[self.selected_index].pos()
                } else {
                    Point::new(0, 0)
                };

                match event.code {
                    KeyCode::Function => {
                        if self.selected_index < self.items.len() {
                            if let LauncherItem::Applet { applet_index, .. } =
                                &self.items[self.selected_index]
                            {
                                let applet = &self.applets[*applet_index];
                                let desc = applet.description.get_string_by_lang(&context.language);
                                if !desc.is_empty() {
                                    self.speech_description = Some(desc.to_string());
                                }
                            }
                        }
                    }
                    KeyCode::Up => {

                        let mut best_idx = None;
                        let mut min_dy = i16::MAX;
                        let mut min_dx = i16::MAX;

                        for (idx, item) in self.items.iter().enumerate() {
                            let pos = item.pos();
                            if pos.y < current_pos.y {
                                let dy = current_pos.y - pos.y;
                                let dx = (pos.x - current_pos.x).abs();

                                if dy < min_dy || (dy == min_dy && dx < min_dx) {
                                    min_dy = dy;
                                    min_dx = dx;
                                    best_idx = Some(idx);
                                }
                            }
                        }

                        if let Some(idx) = best_idx {
                            self.selected_index = idx;
                            needs_redraw = true;
                        }
                    }
                    KeyCode::Down => {
                        let mut best_idx = None;
                        let mut min_dy = i16::MAX;
                        let mut min_dx = i16::MAX;

                        for (idx, item) in self.items.iter().enumerate() {
                            let pos = item.pos();
                            if pos.y > current_pos.y {
                                let dy = pos.y - current_pos.y;
                                let dx = (pos.x - current_pos.x).abs();

                                if dy < min_dy || (dy == min_dy && dx < min_dx) {
                                    min_dy = dy;
                                    min_dx = dx;
                                    best_idx = Some(idx);
                                }
                            }
                        }

                        if let Some(idx) = best_idx {
                            self.selected_index = idx;
                            needs_redraw = true;
                        }
                    }
                    KeyCode::Left if self.selected_index > 0 => {
                        self.selected_index -= 1;
                        needs_redraw = true;
                    }
                    KeyCode::Right if self.selected_index + 1 < self.items.len() => {
                        self.selected_index += 1;
                        needs_redraw = true;
                    }
                    KeyCode::Center if !self.items.is_empty() => {
                        match &self.items[self.selected_index] {
                            LauncherItem::Category { .. } => {
                                // 카테고리 헤더에서 엔터 누르면 해당 카테고리의 첫 번째 앱으로 이동
                                if self.selected_index + 1 < self.items.len() {
                                    self.selected_index += 1;
                                    needs_redraw = true;
                                }
                            }
                            LauncherItem::Applet { applet_index, .. } => {
                                let applet = &self.applets[*applet_index];
                                let display_name =
                                    applet.name.get_string_by_lang(&context.language);
                                log::info!("'{}' 애플릿을 실행합니다...", display_name);
                                if let Err(e) = self.applet_manager.start_applet(&applet.id) {
                                    log::error!("실행 실패: {:?}", e);
                                    self.launch_error = true;
                                } else {
                                    self.launching_app_name = Some(display_name.to_string());
                                }
                                needs_redraw = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if needs_redraw && !self.items.is_empty() {
            let display_size = context.window.get_size();
            let selected_pos = self.items[self.selected_index].pos();

            let item_h = match &self.items[self.selected_index] {
                LauncherItem::Category { .. } => HEADER_HEIGHT,
                LauncherItem::Applet { .. } => BOX_SIZE,
            };

            if selected_pos.y < self.target_scroll_y as i16 {
                self.target_scroll_y = selected_pos.y as f32;
            } else if selected_pos.y + item_h > self.target_scroll_y as i16 + display_size.height {
                self.target_scroll_y = (selected_pos.y + item_h - display_size.height) as f32;
            }
        }

        let now = context.time.get_monotonic_time();
        let dt = if let Some(last) = self.last_frame_time {
            let elapsed = now.saturating_sub(last).as_secs_f32();
            elapsed.clamp(0.001, 0.1)
        } else {
            1.0 / 60.0
        };
        self.last_frame_time = Some(now);

        let lerp_factor = 1.0 - (1.0 - LERP_FACTOR).powf(120.0 * dt);

        let diff = self.target_scroll_y - self.current_scroll_y;
        let update_result = if diff.abs() > 0.1 {
            self.current_scroll_y += diff * lerp_factor;
            UpdateResult::NeedsRedraw
        } else {
            self.current_scroll_y = self.target_scroll_y;
            if needs_redraw {
                UpdateResult::NeedsRedraw
            } else {
                UpdateResult::Unchanged
            }
        };

        self.viewport.center.y = self.current_scroll_y as i16 + self.viewport.height / 2;

        Ok(update_result)
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();
        let display_size = canvas.get_size();

        let pixels = self.viewport.iter_visible_pixels(&self.canvas);
        canvas.draw_iter(pixels);

        if !self.items.is_empty() && self.selected_index < self.items.len() {
            let item = &self.items[self.selected_index];
            let world_pos = item.pos();
            let screen_pos = self.viewport.world_to_screen(world_pos);

            match item {
                LauncherItem::Category { category_name, .. } => {
                    // 카테고리 헤더 포커스 박스 테두리 표시
                    let braille_w = braille::measure_text(category_name);
                    let box_w = (braille_w + 4).min(display_size.width);
                    let box_h = HEADER_HEIGHT;

                    for j in screen_pos.y..screen_pos.y + box_h {
                        for i in screen_pos.x..screen_pos.x + box_w {
                            if j == screen_pos.y
                                || j == screen_pos.y + box_h - 1
                                || i == screen_pos.x
                                || i == screen_pos.x + box_w - 1
                            {
                                if i >= 0
                                    && i < display_size.width
                                    && j >= 0
                                    && j < display_size.height
                                {
                                    canvas.set_pin(Point::new(i, j), Intensity::MAX);
                                }
                            }
                        }
                    }
                }
                LauncherItem::Applet { .. } => {
                    // 앱 선택 테두리 (2픽셀)
                    for j in screen_pos.y..screen_pos.y + BOX_SIZE {
                        for i in screen_pos.x..screen_pos.x + BOX_SIZE {
                            if j < screen_pos.y + BORDER_THICKNESS
                                || j >= screen_pos.y + BOX_SIZE - BORDER_THICKNESS
                                || i < screen_pos.x + BORDER_THICKNESS
                                || i >= screen_pos.x + BOX_SIZE - BORDER_THICKNESS
                            {
                                if i >= 0
                                    && i < display_size.width
                                    && j >= 0
                                    && j < display_size.height
                                {
                                    canvas.set_pin(Point::new(i, j), Intensity::MAX);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        if self.load_error {
            return SpeechResult::segments_with_options(
                LauncherMessage::LoadError.to_segments(context.language),
                SpeechOption::forced(),
            );
        }

        if self.launch_error {
            return SpeechResult::segments_with_options(
                LauncherMessage::LaunchError.to_segments(context.language),
                SpeechOption::forced(),
            );
        }

        if let Some(app_name) = &self.launching_app_name {
            return SpeechResult::segments_with_options(
                LauncherMessage::LaunchingApp(app_name).to_segments(context.language),
                SpeechOption::forced(),
            );
        }

        if let Some(desc) = &self.speech_description {
            return SpeechResult::segments_with_options(
                vec![AudioSegment::Text(desc.clone())],
                SpeechOption::forced(),
            );
        }


        if self.selected_index < self.items.len() {
            let item = &self.items[self.selected_index];

            match item {
                LauncherItem::Category {
                    category_name,
                    applet_count,
                    ..
                } => SpeechResult::segments(
                    LauncherMessage::CategoryFocused {
                        category: category_name,
                        count: *applet_count,
                    }
                    .to_segments(context.language),
                ),
                LauncherItem::Applet { applet_index, .. } => {
                    let applet = &self.applets[*applet_index];
                    let display_name = applet.name.get_string_by_lang(&context.language);
                    SpeechResult::segments(
                        LauncherMessage::AppFocused(display_name).to_segments(context.language),
                    )
                }
            }
        } else {
            SpeechResult::None
        }
    }

    fn on_help(&self, _context: &Context) -> LocalizedString {
        LocalizedString {
            ko: "런처 도움말입니다. 점자로 표시된 카테고리 헤더와 애플릿을 방향키로 탐색하고 Center 키로 실행합니다.".to_string(),
            en: "Launcher help. Navigate category headers in braille and applets using arrow keys and press Center to launch.".to_string(),
            ja: "ランチャーのヘルプです。点字で表示されたカテゴリヘッダーとアプレットを矢印キーで探索し、Centerキーで実行します。".to_string(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let app = Box::new(LauncherApp::default());
    sdk::applet::run(app);
}
