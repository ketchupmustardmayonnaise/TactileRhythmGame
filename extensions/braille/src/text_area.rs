use std::time::Duration;

use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface, Point};
use sdk::error::Result;
use widget::{Widget, WidgetUpdateResult};

/// Represents a single word composed of braille chords.
pub type Word = Vec<u8>;

/// Represents a single line of text composed of words.
pub type Line = Vec<Word>;

/// A widget that provides a multi-line text editing area for braille input.
///
/// It handles text insertion, deletion, cursor movement, and rendering of the text
/// along with a blinking cursor.
pub struct TextArea {
    /// The lines of text, where each line contains words, and each word contains braille chords.
    pub lines: Vec<Line>,
    /// The bounding rectangle defining the visible area of the text area.
    pub bounds: BoundsRect,
    /// The current horizontal scroll offset.
    pub scroll_x: i16,
    /// The current vertical scroll offset.
    pub scroll_y: i16,
    /// Whether word wrapping is enabled.
    pub word_wrap: bool,
    /// Whether the cursor is currently visible (used for blinking effect).
    pub show_cursor: bool,
    /// The time of the last cursor blink toggle.
    pub last_blink_time: Duration,
    /// Flag to indicate that the blink timer should be reset on the next update.
    pub force_reset_blink: bool,
    /// Indicates whether the text area currently has focus.
    pub is_focused: bool,
    /// The current line index of the cursor.
    pub cursor_line: usize,
    /// The current word index of the cursor within the current line.
    pub cursor_word: usize,
    /// The current character (chord) index of the cursor within the current word.
    pub cursor_char: usize,
    /// Indicates whether the text area is in view mode (no cursor, manual scroll).
    pub is_view_mode: bool,
}

const BLINK_INTERVAL: Duration = Duration::from_millis(500);

impl TextArea {
    /// Creates a new `TextArea` with the specified bounds.
    pub fn new(bounds: BoundsRect) -> Self {
        Self {
            lines: vec![vec![Vec::new()]],
            bounds,
            scroll_x: 0,
            scroll_y: 0,
            word_wrap: false,
            show_cursor: true,
            last_blink_time: Duration::ZERO,
            force_reset_blink: false,
            is_focused: false,
            cursor_line: 0,
            cursor_word: 0,
            cursor_char: 0,
            is_view_mode: false,
        }
    }

    /// Sets whether text should wrap at the edge of the bounding box.
    pub fn set_word_wrap(&mut self, word_wrap: bool) {
        self.word_wrap = word_wrap;
        if word_wrap {
            self.scroll_x = 0;
        }
        self.update_scroll();
    }

    /// Sets the view mode. When true, cursor is hidden and manual scrolling is enabled.
    pub fn set_view_mode(&mut self, is_view_mode: bool) {
        self.is_view_mode = is_view_mode;
        if !is_view_mode {
            self.reset_cursor_blink();
            self.update_scroll();
        }
    }

    pub fn max_scroll_y(&self) -> i16 {
        let line_step = crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
        if !self.word_wrap {
            return ((self.lines.len() as i16) * line_step - self.bounds.height()).max(0);
        }

        let mut y = 0;
        let wrap_max_x = self.bounds.width();

        for line in &self.lines {
            let mut x = 0;
            for word in line {
                if word.is_empty() {
                    x += crate::WORD_GAP;
                    if x >= wrap_max_x {
                        x = 0;
                        y += line_step;
                    }
                    continue;
                }
                let word_pixel_width = (word.len() as i16)
                    * (crate::cell::CELL_WIDTH + crate::cell::CELL_GAP)
                    - crate::cell::CELL_GAP;

                if x > 0 && x + word_pixel_width > wrap_max_x {
                    x = 0;
                    y += line_step;
                }
                for _ in 0..word.len() {
                    if x + crate::cell::CELL_WIDTH > wrap_max_x {
                        x = 0;
                        y += line_step;
                    }
                    x += crate::cell::CELL_WIDTH + crate::cell::CELL_GAP;
                }
                x += crate::WORD_GAP;
            }
            y += line_step;
        }
        (y - self.bounds.height()).max(0)
    }

    pub fn max_scroll_x(&self) -> i16 {
        if self.word_wrap {
            return 0;
        }
        let mut max_x = 0;
        for line in &self.lines {
            let mut current_x = 0;
            for word in line {
                if word.is_empty() {
                    current_x += crate::WORD_GAP;
                    continue;
                }
                let word_pixel_width = (word.len() as i16)
                    * (crate::cell::CELL_WIDTH + crate::cell::CELL_GAP)
                    - crate::cell::CELL_GAP;
                current_x += word_pixel_width + crate::cell::CELL_GAP + crate::WORD_GAP;
            }
            if current_x > max_x {
                max_x = current_x;
            }
        }
        (max_x - self.bounds.width()).max(0)
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0, self.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0, self.max_scroll_y());
    }

    pub fn scroll_up(&mut self) {
        self.scroll_y -= crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
        self.clamp_scroll();
    }

    pub fn scroll_down(&mut self) {
        self.scroll_y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
        self.clamp_scroll();
    }

    pub fn scroll_left(&mut self) {
        self.scroll_x -= crate::cell::CELL_WIDTH + crate::cell::CELL_GAP;
        self.clamp_scroll();
    }

    pub fn scroll_right(&mut self) {
        self.scroll_x += crate::cell::CELL_WIDTH + crate::cell::CELL_GAP;
        self.clamp_scroll();
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_y -= self.bounds.height();
        self.clamp_scroll();
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_y += self.bounds.height();
        self.clamp_scroll();
    }

    pub fn scroll_to_start(&mut self) {
        self.scroll_x = 0;
        self.clamp_scroll();
    }

    pub fn scroll_to_end(&mut self) {
        self.scroll_x = self.max_scroll_x();
        self.clamp_scroll();
    }

    /// Inserts a braille chord at the current cursor position.
    pub fn push_chord(&mut self, chord: u8) {
        self.lines[self.cursor_line][self.cursor_word].insert(self.cursor_char, chord);
        self.cursor_char += 1;
    }

    /// Inserts a space at the current cursor position, potentially splitting the current word.
    pub fn push_space(&mut self) {
        let current_word = &mut self.lines[self.cursor_line][self.cursor_word];
        let new_word = current_word.split_off(self.cursor_char);

        self.cursor_word += 1;
        self.cursor_char = 0;
        self.lines[self.cursor_line].insert(self.cursor_word, new_word);
    }

    /// Inserts a newline at the current cursor position, splitting the current line.
    pub fn push_newline(&mut self) {
        let current_word = &mut self.lines[self.cursor_line][self.cursor_word];
        let new_word = current_word.split_off(self.cursor_char);

        let mut new_line = vec![new_word];
        let current_line = &mut self.lines[self.cursor_line];
        if self.cursor_word + 1 < current_line.len() {
            let rest_words = current_line.split_off(self.cursor_word + 1);
            new_line.extend(rest_words);
        }

        self.cursor_line += 1;
        self.cursor_word = 0;
        self.cursor_char = 0;
        self.lines.insert(self.cursor_line, new_line);
    }

    /// Deletes the character immediately before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_char > 0 {
            self.cursor_char -= 1;
            self.lines[self.cursor_line][self.cursor_word].remove(self.cursor_char);
        } else if self.cursor_word > 0 {
            let current_word = self.lines[self.cursor_line].remove(self.cursor_word);
            self.cursor_word -= 1;

            let prev_word = &mut self.lines[self.cursor_line][self.cursor_word];
            self.cursor_char = prev_word.len();
            prev_word.extend(current_word);
        } else if self.cursor_line > 0 {
            let mut current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;

            let prev_line = &mut self.lines[self.cursor_line];
            self.cursor_word = prev_line.len() - 1;
            self.cursor_char = prev_line[self.cursor_word].len();

            let first_word_of_current = current_line.remove(0);
            prev_line[self.cursor_word].extend(first_word_of_current);
            prev_line.extend(current_line);
        }
    }

    /// Deletes the character exactly at the cursor position.
    pub fn delete(&mut self) {
        if self.cursor_char < self.lines[self.cursor_line][self.cursor_word].len() {
            // 커서 위치의 점자 문자 삭제
            self.lines[self.cursor_line][self.cursor_word].remove(self.cursor_char);
        } else if self.cursor_word < self.lines[self.cursor_line].len() - 1 {
            // 현재 단어의 끝(공백)인 경우, 다음 단어를 현재 단어로 당겨와서 병합
            let next_word = self.lines[self.cursor_line].remove(self.cursor_word + 1);
            self.lines[self.cursor_line][self.cursor_word].extend(next_word);
        } else if self.cursor_line < self.lines.len() - 1 {
            // 줄의 끝인 경우, 다음 줄을 현재 줄로 당겨와서 병합
            let mut next_line = self.lines.remove(self.cursor_line + 1);
            let first_word_of_next = next_line.remove(0);
            self.lines[self.cursor_line][self.cursor_word].extend(first_word_of_next);
            self.lines[self.cursor_line].extend(next_line);
        }
        self.reset_cursor_blink();
    }

    /// Resets the cursor blinking animation, making the cursor immediately visible.
    ///
    /// This is typically called after text input or cursor movement.
    pub fn reset_cursor_blink(&mut self) {
        self.show_cursor = true;
        self.force_reset_blink = true;
    }

    /// Moves the cursor one character to the left.
    pub fn move_cursor_left(&mut self) {
        if self.cursor_char > 0 {
            self.cursor_char -= 1;
        } else if self.cursor_word > 0 {
            self.cursor_word -= 1;
            self.cursor_char = self.lines[self.cursor_line][self.cursor_word].len();
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_word = self.lines[self.cursor_line].len() - 1;
            self.cursor_char = self.lines[self.cursor_line][self.cursor_word].len();
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor one character to the right.
    pub fn move_cursor_right(&mut self) {
        let line = &self.lines[self.cursor_line];
        let word = &line[self.cursor_word];

        if self.cursor_char < word.len() {
            self.cursor_char += 1;
        } else if self.cursor_word < line.len() - 1 {
            self.cursor_word += 1;
            self.cursor_char = 0;
        } else if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_word = 0;
            self.cursor_char = 0;
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor one line up.
    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
        } else {
            // 이미 맨 윗줄인 경우, 맨 앞으로 이동
            self.cursor_word = 0;
            self.cursor_char = 0;
            self.reset_cursor_blink();
            return;
        }

        let current_line = &self.lines[self.cursor_line];
        if self.cursor_word >= current_line.len() {
            self.cursor_word = current_line.len() - 1;
            self.cursor_char = current_line[self.cursor_word].len();
        } else {
            let current_word = &current_line[self.cursor_word];
            if self.cursor_char > current_word.len() {
                self.cursor_char = current_word.len();
            }
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor one line down.
    pub fn move_cursor_down(&mut self) {
        if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
        } else {
            // 이미 맨 아랫줄인 경우, 맨 뒤로 이동
            self.cursor_word = self.lines[self.cursor_line].len() - 1;
            self.cursor_char = self.lines[self.cursor_line][self.cursor_word].len();
            self.reset_cursor_blink();
            return;
        }

        let current_line = &self.lines[self.cursor_line];
        if self.cursor_word >= current_line.len() {
            self.cursor_word = current_line.len() - 1;
            self.cursor_char = current_line[self.cursor_word].len();
        } else {
            let current_word = &current_line[self.cursor_word];
            if self.cursor_char > current_word.len() {
                self.cursor_char = current_word.len();
            }
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor to the beginning of the previous word.
    pub fn move_cursor_prev_word(&mut self) {
        if self.cursor_char > 0 {
            self.cursor_char = 0;
        } else if self.cursor_word > 0 {
            self.cursor_word -= 1;
            self.cursor_char = 0;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_word = self.lines[self.cursor_line].len() - 1;
            self.cursor_char = 0;
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor to the beginning of the next word.
    pub fn move_cursor_next_word(&mut self) {
        let line = &self.lines[self.cursor_line];
        if self.cursor_word < line.len() - 1 {
            self.cursor_word += 1;
            self.cursor_char = 0;
        } else if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_word = 0;
            self.cursor_char = 0;
        } else {
            self.cursor_char = self.lines[self.cursor_line][self.cursor_word].len();
        }
        self.reset_cursor_blink();
    }

    /// Moves the cursor to the start of the current line.
    pub fn move_cursor_line_start(&mut self) {
        self.cursor_word = 0;
        self.cursor_char = 0;
        self.reset_cursor_blink();
    }

    /// Moves the cursor to the end of the current line.
    pub fn move_cursor_line_end(&mut self) {
        self.cursor_word = self.lines[self.cursor_line].len() - 1;
        self.cursor_char = self.lines[self.cursor_line][self.cursor_word].len();
        self.reset_cursor_blink();
    }

    /// Updates the vertical scroll offset to ensure the cursor remains visible.
    pub fn update_scroll(&mut self) {
        let mut x = 0;
        let mut y = 0;
        let max_x = self.bounds.width();
        let mut cursor_x = 0;
        let mut cursor_y = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            for (word_idx, word) in line.iter().enumerate() {
                if line_idx == self.cursor_line
                    && word_idx == self.cursor_word
                    && self.cursor_char == 0
                {
                    cursor_x = x;
                    cursor_y = y;
                }

                if word.is_empty() {
                    x += crate::WORD_GAP;
                    if self.word_wrap && x >= max_x {
                        x = 0;
                        y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
                    }
                    continue;
                }

                let word_pixel_width = (word.len() as i16)
                    * (crate::cell::CELL_WIDTH + crate::cell::CELL_GAP)
                    - crate::cell::CELL_GAP;

                if self.word_wrap && x > 0 && x + word_pixel_width > max_x {
                    x = 0;
                    y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;

                    if line_idx == self.cursor_line
                        && word_idx == self.cursor_word
                        && self.cursor_char == 0
                    {
                        cursor_x = x;
                        cursor_y = y;
                    }
                }

                for chord_idx in 0..word.len() {
                    if self.word_wrap && x + crate::cell::CELL_WIDTH > max_x {
                        x = 0;
                        y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;

                        if line_idx == self.cursor_line
                            && word_idx == self.cursor_word
                            && self.cursor_char == chord_idx
                        {
                            cursor_x = x;
                            cursor_y = y;
                        }
                    }

                    x += crate::cell::CELL_WIDTH + crate::cell::CELL_GAP;

                    if line_idx == self.cursor_line
                        && word_idx == self.cursor_word
                        && chord_idx + 1 == self.cursor_char
                    {
                        cursor_x = x;
                        cursor_y = y;
                    }
                }

                x += crate::WORD_GAP;
            }
            x = 0;
            y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
        }

        if self.word_wrap && cursor_x + crate::cell::CELL_WIDTH > max_x {
            cursor_y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
            cursor_x = 0;
        }

        // 가로 스크롤(X) 업데이트
        if !self.word_wrap {
            if cursor_x < self.scroll_x {
                self.scroll_x = cursor_x;
            } else if cursor_x + crate::cell::CELL_WIDTH > self.scroll_x + self.bounds.width() {
                self.scroll_x = cursor_x + crate::cell::CELL_WIDTH - self.bounds.width();
            }
        } else {
            self.scroll_x = 0;
        }

        // 세로 스크롤(Y) 업데이트
        let line_step = crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;

        if cursor_y < self.scroll_y {
            self.scroll_y = cursor_y;
        } else if cursor_y + crate::cell::CELL_HEIGHT > self.scroll_y + self.bounds.height() {
            let required_scroll = cursor_y + crate::cell::CELL_HEIGHT - self.bounds.height();
            self.scroll_y = ((required_scroll + line_step - 1) / line_step) * line_step;
        }
    }
}

impl Widget for TextArea {
    fn bounds(&self) -> BoundsRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: BoundsRect) {
        self.bounds = bounds;
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_update(&mut self, context: &mut Context, is_focused: bool) -> Result<WidgetUpdateResult> {
        let mut needs_redraw = false;
        let now = context.time.get_monotonic_time();

        if self.force_reset_blink {
            self.last_blink_time = now;
            self.force_reset_blink = false;
        }

        if self.is_focused != is_focused {
            self.is_focused = is_focused;
            self.show_cursor = is_focused;
            self.last_blink_time = now;
            needs_redraw = true;
        }

        if is_focused && now.saturating_sub(self.last_blink_time) >= BLINK_INTERVAL {
            self.show_cursor = !self.show_cursor;
            self.last_blink_time = now;
            needs_redraw = true;
        }

        let old_scroll_y = self.scroll_y;
        let old_scroll_x = self.scroll_x;
        if !self.is_view_mode {
            self.update_scroll();
        }
        if old_scroll_y != self.scroll_y || old_scroll_x != self.scroll_x {
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(WidgetUpdateResult::NeedsRedraw)
        } else {
            Ok(WidgetUpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        let mut x = self.bounds.x() - self.scroll_x;
        let mut y = self.bounds.y() - self.scroll_y;
        let start_x = x;
        let wrap_max_x = self.bounds.x() + self.bounds.width();

        let min_x = self.bounds.x();
        let max_x = self.bounds.x() + self.bounds.width();
        let min_y = self.bounds.y();
        let max_y = self.bounds.y() + self.bounds.height();

        let mut cursor_x = x;
        let mut cursor_y = y;

        for (line_idx, line) in self.lines.iter().enumerate() {
            for (word_idx, word) in line.iter().enumerate() {
                if line_idx == self.cursor_line
                    && word_idx == self.cursor_word
                    && self.cursor_char == 0
                {
                    cursor_x = x;
                    cursor_y = y;
                }

                if word.is_empty() {
                    x += crate::WORD_GAP; // 공백 폭 (WORD_GAP)
                    if self.word_wrap && x >= wrap_max_x {
                        x = start_x;
                        y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
                    }
                    continue;
                }

                let word_pixel_width = (word.len() as i16)
                    * (crate::cell::CELL_WIDTH + crate::cell::CELL_GAP)
                    - crate::cell::CELL_GAP;

                // Word Wrap: 단어가 화면을 넘어가고, 현재 줄에 이미 다른 내용이 있다면 줄바꿈
                if self.word_wrap && x > start_x && x + word_pixel_width > wrap_max_x {
                    x = start_x;
                    y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;

                    if line_idx == self.cursor_line
                        && word_idx == self.cursor_word
                        && self.cursor_char == 0
                    {
                        cursor_x = x;
                        cursor_y = y;
                    }
                }

                // 단어 내의 점자 문자를 순회하며 그립니다.
                for (chord_idx, &chord) in word.iter().enumerate() {
                    // Character Wrap: 단일 단어가 화면 전체 너비보다 긴 경우를 위한 안전 장치
                    if self.word_wrap && x + crate::cell::CELL_WIDTH > wrap_max_x {
                        x = start_x;
                        y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;

                        if line_idx == self.cursor_line
                            && word_idx == self.cursor_word
                            && self.cursor_char == chord_idx
                        {
                            cursor_x = x;
                            cursor_y = y;
                        }
                    }

                    // Culling: 화면에 보이는 영역 안의 점자만 렌더링
                    if y + crate::cell::CELL_HEIGHT > min_y
                        && y < max_y
                        && x + crate::cell::CELL_WIDTH > min_x
                        && x < max_x
                    {
                        crate::render_chords(canvas, &[chord], Point::new(x, y));
                    }
                    x += crate::cell::CELL_WIDTH + crate::cell::CELL_GAP;

                    if line_idx == self.cursor_line
                        && word_idx == self.cursor_word
                        && chord_idx + 1 == self.cursor_char
                    {
                        cursor_x = x;
                        cursor_y = y;
                    }
                }

                x += crate::WORD_GAP; // 단어 뒤 공백 폭 (WORD_GAP) 추가
            }

            // 한 줄(Line)의 렌더링이 끝나면 명시적으로 다음 줄로 이동합니다.
            x = start_x;
            y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
        }

        if !self.is_view_mode && self.is_focused && self.show_cursor {
            if self.word_wrap && cursor_x + crate::cell::CELL_WIDTH > wrap_max_x {
                cursor_x = start_x;
                cursor_y += crate::cell::CELL_HEIGHT + crate::cell::CELL_GAP;
            }

            if cursor_y + crate::cell::CELL_HEIGHT > min_y
                && cursor_y < max_y
                && cursor_x + crate::cell::CELL_WIDTH > min_x
                && cursor_x < max_x
            {
                crate::cell::render_cursor(
                    canvas,
                    Point::new(cursor_x, cursor_y),
                    sdk::api::display::Intensity::MAX,
                );
            }
        }

        Ok(())
    }
}
