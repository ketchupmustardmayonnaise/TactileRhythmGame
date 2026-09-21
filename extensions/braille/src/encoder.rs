pub const SPACE: u8 = 0x00;

/// braillify 라이브러리를 사용하여 일반 텍스트(한글, 영어, 숫자, 기호 등)를
/// 2024 개정 점자 규정에 맞춰 점자 셀(u8 비트마스크, 0..=63)의 배열로 변환합니다.
pub fn text_to_braille(text: &str) -> Vec<u8> {
    match braillify::encode(text) {
        Ok(cells) => cells,
        Err(_) => Vec::new(),
    }
}
