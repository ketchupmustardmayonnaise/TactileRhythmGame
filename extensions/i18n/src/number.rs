use super::data;
use sdk::types::Language;

/// 지정된 언어에 맞춰 '시(Hour)'에 해당하는 숫자 발음 텍스트를 반환합니다.
pub fn format_hour(lang: Language, hour: u32) -> String {
    let mut h = if hour == 0 {
        12
    } else if hour > 12 {
        hour % 12
    } else {
        hour
    };
    if h == 0 {
        h = 12;
    } // 12의 배수 처리

    match lang {
        Language::Ko => data::KOR_NATIVE_DETS[h as usize].to_string(),
        Language::En => {
            if h < 10 {
                data::ENG_UNITS[h as usize].to_string()
            } else {
                data::ENG_TEENS[(h - 10) as usize].to_string()
            }
        }
        Language::Ja => {
            // 일본어 시 단위 발음 예외 처리 (4: よ, 7: しち, 9: く)
            match h {
                4 => "よ".to_string(),
                7 => "しち".to_string(),
                9 => "く".to_string(),
                10 => "じゅう".to_string(),
                11 => "じゅういち".to_string(),
                12 => "じゅうに".to_string(),
                _ => data::JA_UNITS[h as usize].to_string(),
            }
        }
    }
}

/// 지정된 언어에 맞춰 '분/초(Minute/Second)'에 해당하는 숫자 발음 텍스트를 반환합니다.
pub fn format_minute(lang: Language, minute: u32) -> String {
    match lang {
        Language::Ko => {
            if minute == 0 {
                "영".to_string()
            } else {
                let t = (minute / 10) as usize;
                let u = (minute % 10) as usize;
                format!("{}{}", data::KOR_SINO_TENS[t], data::KOR_SINO_UNITS[u])
            }
        }
        Language::En => {
            if minute == 0 {
                "Zero".to_string()
            } else if minute < 10 {
                // 한 자릿수 분은 영어 TTS를 위해 'Oh One' 같이 읽는게 자연스럽지만, 우선은 일반 숫자로 처리
                data::ENG_UNITS[minute as usize].to_string()
            } else if minute < 20 {
                data::ENG_TEENS[(minute - 10) as usize].to_string()
            } else {
                let t = (minute / 10) as usize;
                let u = (minute % 10) as usize;
                if u == 0 {
                    data::ENG_TENS[t].to_string()
                } else {
                    format!(
                        "{}-{}",
                        data::ENG_TENS[t],
                        data::ENG_UNITS[u].to_lowercase()
                    )
                }
            }
        }
        Language::Ja => {
            // 일본어 분 단위는 촉음/반탁음 예외가 많으나 (いっぷん 등),
            // 여기서는 TTS 엔진이 유추할 수 있도록 텍스트 또는 보편적 한자어 숫자로 조합
            if minute == 0 {
                "ゼロ".to_string()
            } else {
                // 정확한 일본어 발음 변형 (분/초 단위 조수사 결합 전 숫자)
                let t = (minute / 10) as usize;
                let u = (minute % 10) as usize;
                let tens = data::JA_TENS[t].to_string();
                let units = if u != 0 {
                    data::JA_UNITS[u].to_string()
                } else {
                    "".to_string()
                };
                format!("{}{}", tens, units)
            }
        }
    }
}

/// 지정된 언어에 맞춰 '일반 숫자'에 해당하는 발음 텍스트를 반환합니다.
/// (예: 1 -> Ko: "일"(한자어) 또는 "하나"(고유어), En: "One", Ja: "いち")
/// 현재는 기본적으로 한국어의 경우 한자어(일, 이, 삼)를 반환하도록 구현되어 있습니다.
/// 필요에 따라 고유어(하나, 둘) 모드를 추가할 수 있습니다.
pub fn format_num(lang: Language, num: u32) -> String {
    match lang {
        Language::Ko => {
            if num == 0 {
                "영".to_string()
            } else if num < 100 {
                let t = (num / 10) as usize;
                let u = (num % 10) as usize;
                format!("{}{}", data::KOR_SINO_TENS[t], data::KOR_SINO_UNITS[u])
            } else if num == 100 {
                "백".to_string()
            } else {
                num.to_string()
            }
        }
        Language::En => {
            if num == 0 {
                "Zero".to_string()
            } else if num < 10 {
                data::ENG_UNITS[num as usize].to_string()
            } else if num < 20 {
                data::ENG_TEENS[(num - 10) as usize].to_string()
            } else if num < 100 {
                let t = (num / 10) as usize;
                let u = (num % 10) as usize;
                if u == 0 {
                    data::ENG_TENS[t].to_string()
                } else {
                    format!(
                        "{}-{}",
                        data::ENG_TENS[t],
                        data::ENG_UNITS[u].to_lowercase()
                    )
                }
            } else if num == 100 {
                "One hundred".to_string()
            } else {
                num.to_string()
            }
        }
        Language::Ja => {
            if num == 0 {
                "ゼロ".to_string()
            } else if num < 100 {
                let t = (num / 10) as usize;
                let u = (num % 10) as usize;
                let tens = data::JA_TENS[t].to_string();
                let units = if u != 0 {
                    data::JA_UNITS[u].to_string()
                } else {
                    "".to_string()
                };
                format!("{}{}", tens, units)
            } else if num == 100 {
                "ひゃく".to_string()
            } else {
                num.to_string()
            }
        }
    }
}


/// 지정된 언어와 숫자에 맞춰 서수(몇 번째인지)를 뜻하는 발음 텍스트를 반환합니다.
///
/// * `lang` - 번역 및 변환에 사용할 타겟 언어 (`Language::Ko`, `Language::En`, `Language::Ja`)
/// * `rank` - 순위를 뜻하는 숫자 (1 이상의 양수)
///
/// # 반환값
/// 지정된 언어의 서수 표현 문자열을 반환합니다.
/// - 1 ~ 10 범위의 숫자는 사전 상수 데이터 배열에 정의된 명확한 텍스트로 치환됩니다.
/// - 11 이상의 값은 각 언어의 보편적인 표기 규칙을 적용하여 안전하게 동적 생성해 반환합니다.
pub fn format_ordinal(lang: Language, rank: u32) -> String {
    // 1에서 10 사이인 경우 사전(data.rs)에 정의된 상수 배열에서 값을 가져옵니다.
    if (1..=10).contains(&rank) {
        match lang {
            Language::Ko => data::KOR_ORDINAL_NUMBERS[rank as usize].to_string(),
            Language::En => data::ENG_ORDINAL_NUMBERS[rank as usize].to_string(),
            Language::Ja => data::JA_ORDINAL_NUMBERS[rank as usize].to_string(),
        }
    } else {
        // 11 이상의 범위 밖 숫자에 대해 예외 및 동적 포맷팅 처리 규칙을 적용합니다.
        match lang {
            Language::Ko => {
                // 한국어: "{숫자} 번째" (예: "11 번째")
                format!("{} 번째", rank)
            }
            Language::En => {
                // 영어 서수 규칙 (st, nd, rd, th 접미사 동적 처리)
                // 11, 12, 13은 예외적으로 th 접미사를 가집니다.
                let suffix = if rank % 100 >= 11 && rank % 100 <= 13 {
                    "th"
                } else {
                    match rank % 10 {
                        1 => "st",
                        2 => "nd",
                        3 => "rd",
                        _ => "th",
                    }
                };
                format!("{}{}", rank, suffix)
            }
            Language::Ja => {
                // 일본어: "第{숫자}" (예: "第11")
                format!("第{}", rank)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ordinal() {
        // 한국어 서수 변환 케이스 테스트
        assert_eq!(format_ordinal(Language::Ko, 1), "첫 번째");
        assert_eq!(format_ordinal(Language::Ko, 5), "다섯 번째");
        assert_eq!(format_ordinal(Language::Ko, 10), "열 번째");
        assert_eq!(format_ordinal(Language::Ko, 11), "11 번째");
        assert_eq!(format_ordinal(Language::Ko, 102), "102 번째");

        // 영어 서수 변환 케이스 테스트
        assert_eq!(format_ordinal(Language::En, 1), "first");
        assert_eq!(format_ordinal(Language::En, 2), "second");
        assert_eq!(format_ordinal(Language::En, 3), "third");
        assert_eq!(format_ordinal(Language::En, 4), "fourth");
        assert_eq!(format_ordinal(Language::En, 10), "tenth");
        assert_eq!(format_ordinal(Language::En, 11), "11th");
        assert_eq!(format_ordinal(Language::En, 12), "12th");
        assert_eq!(format_ordinal(Language::En, 13), "13th");
        assert_eq!(format_ordinal(Language::En, 21), "21st");
        assert_eq!(format_ordinal(Language::En, 22), "22nd");
        assert_eq!(format_ordinal(Language::En, 23), "23rd");
        assert_eq!(format_ordinal(Language::En, 101), "101st");

        // 일본어 서수 변환 케이스 테스트
        assert_eq!(format_ordinal(Language::Ja, 1), "第一 (だいいち)");
        assert_eq!(format_ordinal(Language::Ja, 5), "第五 (だいご)");
        assert_eq!(format_ordinal(Language::Ja, 10), "第十 (だいじゅう)");
        assert_eq!(format_ordinal(Language::Ja, 11), "第11");
        assert_eq!(format_ordinal(Language::Ja, 99), "第99");
    }
}
