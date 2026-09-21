/// # tts_segment! 매크로 정의
///
/// 이 매크로는 여러 개의 개별 인자값들을 입력받아 순서대로 `String` 타입의 벡터(`Vec<String>`)로 묶어 반환합니다.
/// 런타임에 유입되는 모든 데이터는 본질적으로 문자열(문자 바이트 스트림)에 속하므로,
/// `&str`, `String`, `&&str` 등 어떠한 문자열 변형 표현도 예외 없이 안전하게 `.to_string()`을 거쳐 수집됩니다.
/// 추가적으로, 각 문자열을 구분자 기준(`DELIMITERS`)으로 똑똑하게 자동 파싱(청킹)하여 긴 문장의 합성 지연을 예방합니다.
///
#[macro_export]
macro_rules! tts_segment {
    ($($val:expr),* $(,)?) => {
        {
            // 모든 인자들을 빈칸 없이(또는 필요시 공백 하나를 두고) 하나의 String으로 합칩니다.
            let mut combined = String::new();
            $(
                combined.push_str(&($val).to_string());
            )*
            // 완성된 통 문자열을 분할 함수로 전달하여 최종 Vec<String>을 반환합니다.
            $crate::macros::split_tts_text(&combined)
        }
    };
}

/// # TTS 텍스트 분할에 사용할 구분자 상수 배열
///
/// 문장을 나누는 기준이 되는 구두점 목록입니다. 새로운 구분자를 추가하려면 이 배열에 문자(char)를 추가하기만 하면 됩니다.
pub const DELIMITERS: &[char] = &[
    '.', ',', '?', '!', '-', '/', '／', ';', '・', '；', '？', '、', '。', '！',
];

/// # 구두점 보존 문자열 분할 함수
///
/// 입력된 문자열을 `DELIMITERS`에 지정된 구분자 기준으로 분할합니다.
/// 이때 TTS 엔진의 올바른 억양(Prosody) 처리를 위해 해당 구분자들을 문장 끝에 그대로 보존합니다.
/// 또한 `...` 이나 `!?` 처럼 연속된 구두점들이 강제로 쪼개지지 않고 하나의 그룹으로 묶이도록 처리합니다.
pub fn split_tts_text(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut current = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        current.push(c);

        // 현재 문자가 구분자 목록(DELIMITERS)에 포함되어 있는지 확인합니다.
        if DELIMITERS.contains(&c) {
            // 다음에 연속되는 구분자들(예: "...", "!?", "!!!")이 있다면 함께 포함시킵니다.
            while i + 1 < chars.len() {
                let next_c = chars[i + 1];
                if DELIMITERS.contains(&next_c) {
                    current.push(next_c);
                    i += 1;
                } else {
                    break;
                }
            }

            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                segments.push(trimmed);
            }
            current.clear();
        }
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        segments.push(trimmed);
    }

    if segments.is_empty() {
        vec![text.to_string()]
    } else {
        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_tts_text() {
        let input = "안녕하세요. 반갑습니다! 식사는 하셨나요? 네, 아주 맛있게 먹었습니다.";
        let result = split_tts_text(input);
        assert_eq!(
            result,
            vec![
                "안녕하세요.".to_string(),
                "반갑습니다!".to_string(),
                "식사는 하셨나요?".to_string(),
                "네,".to_string(),
                "아주 맛있게 먹었습니다.".to_string()
            ]
        );
    }

    #[test]
    fn test_consecutive_punctuations() {
        let input = "오 정말인가요...?! 대박이네요!!!";
        let result = split_tts_text(input);
        assert_eq!(
            result,
            vec![
                "오 정말인가요...?!".to_string(),
                "대박이네요!!!".to_string()
            ]
        );
    }
}
