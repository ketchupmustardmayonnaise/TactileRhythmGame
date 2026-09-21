use crate::number;
use sdk::types::Language;

/// 지정된 언어와 숫자에 기반하여 '시간(시)'에 대응하는 발음 텍스트를 반환합니다.
/// 한국어는 "한" + "시" = "한시" 와 같이 수사와 단위 명사를 결합한 완성형으로 생성하여,
/// 런타임에 단어 단위로 결합해 발생할 수 있는 음성 운율 끊김 문제를 방지합니다.
///
/// # 예시
/// * Ko: `1` -> `"한시"`, `12` -> `"열두시"`
/// * En: `1` -> `"One"` (영어는 시계 발화 시 단위를 보통 생략)
/// * Ja: `1` -> `"いちじ"` (일시), `4` -> `"よじ"` (사시 - 예외 발음 반영)
pub fn h(lang: Language, hour: u32) -> String {
    let val = number::format_hour(lang, hour);
    if val.is_empty() {
        return "".to_string();
    }
    match lang {
        Language::Ko => format!("{}시", val),
        Language::Ja => format!("{}じ", val), // 일본어 '時(じ)' 결합
        Language::En => val, // 영어는 시각 뒤에 분이 바로 붙으므로 숫자 단독 반환이 가장 자연스러움 (예: One, Two)
    }
}

/// 지정된 언어와 숫자에 기반하여 '분'에 대응하는 발음 텍스트를 반환합니다.
///
/// # 예시
/// * Ko: `1` -> `"일분"`, `30` -> `"삼십분"`
/// * En: `15` -> `"Fifteen"` (영어는 분 단위를 보통 생략)
/// * Ja: `1` -> `"いっぷん"` (일분 - 촉음 변형), `2` -> `"にふん"` (이분 - 기본 발음)
pub fn m(lang: Language, minute: u32) -> String {
    match lang {
        Language::Ko => {
            let val = number::format_minute(lang, minute);
            format!("{}분", val)
        }
        Language::Ja => {
            // 일본어 분(分 - ふん/ぷん)은 숫자에 따라 발음이 극심하게 변하므로 예외 처리 헬퍼 사용
            format_ja_minute(minute)
        }
        Language::En => {
            // 영어는 분 단위를 보통 숫자 단독으로 읽음 (예: Twenty-five)
            number::format_minute(lang, minute)
        }
    }
}

/// 지정된 언어와 숫자에 기반하여 '초'에 대응하는 발음 텍스트를 반환합니다.
///
/// # 예시
/// * Ko: `1` -> `"일초"`, `45` -> `"사십오초"`
/// * En: `1` -> `"One second"`, `15` -> `"Fifteen seconds"` (단복수 일치 적용)
/// * Ja: `1` -> `"いちびょう"` (일초), `10` -> `"じゅうびょう"` (십초)
pub fn s(lang: Language, second: u32) -> String {
    match lang {
        Language::Ko => {
            let val = number::format_minute(lang, second);
            format!("{}초", val)
        }
        Language::Ja => {
            let val = number::format_minute(lang, second);
            format!("{}びょう", val) // 일본어 '秒(びょう)' 결합
        }
        Language::En => {
            let val = number::format_minute(lang, second);
            if second == 1 {
                format!("{} second", val.to_lowercase())
            } else {
                format!("{} seconds", val.to_lowercase())
            }
        }
    }
}

/// 오전/오후 구분 텍스트를 반환합니다.
pub fn period(lang: Language, hour: u32) -> &'static str {
    let is_am = hour < 12;
    match lang {
        Language::Ko => {
            if is_am {
                "오전"
            } else {
                "오후"
            }
        }
        Language::Ja => {
            if is_am {
                "午前"
            } else {
                "午後"
            }
        }
        _ => {
            if is_am {
                "AM"
            } else {
                "PM"
            }
        }
    }
}

/// 지정된 언어와 숫자에 기반하여 '일반 숫자' 텍스트를 반환합니다.
pub fn num(lang: Language, n: u32) -> String {
    number::format_num(lang, n)
}

/// 일본어 분(分 - ふん/ぷん)의 촉음 및 반탁음 발음 규칙을 완벽하게 처리해주는 내부 헬퍼 함수입니다.
/// TTS 엔진이 잘못 읽는 오류를 원천 차단하기 위해 실 가나 표기로 완성합니다.
fn format_ja_minute(minute: u32) -> String {
    if minute == 0 {
        return "ゼロ分".to_string();
    }
    if minute > 59 {
        return format!("{}分", minute);
    }

    let t = (minute / 10) as usize;
    let u = (minute % 10) as usize;

    let tens_prefix = if t > 0 {
        super::data::JA_TENS[t].to_string()
    } else {
        "".to_string()
    };

    // 일의 자리에 따른 일본어 분 단위 결합 및 촉음화 처리
    match u {
        1 => format!("{}いっぷん", tens_prefix),
        2 => format!("{}にふん", tens_prefix),
        3 => format!("{}さんぷん", tens_prefix),
        4 => format!("{}よんぷん", tens_prefix),
        5 => format!("{}ごふん", tens_prefix),
        6 => format!("{}ろっぷん", tens_prefix),
        7 => format!("{}ななふん", tens_prefix),
        8 => format!("{}はっぷん", tens_prefix),
        9 => format!("{}きゅうふん", tens_prefix),
        0 => {
            // 십 단위 분 처리 (10분: じゅっぷん, 20분: にじゅっぷん 등)
            if t == 1 {
                "じゅっぷん".to_string()
            } else {
                let prefix = match t {
                    2 => "ni",
                    3 => "san",
                    4 => "yon",
                    5 => "go",
                    _ => "",
                };
                // 로마자 접두어 한글화 번역 및 매핑
                let prefix_ja = match prefix {
                    "ni" => "に",
                    "san" => "さん",
                    "yon" => "よん",
                    "go" => "ご",
                    _ => "",
                };
                format!("{}じゅっぷん", prefix_ja)
            }
        }
        _ => unreachable!(),
    }
}
