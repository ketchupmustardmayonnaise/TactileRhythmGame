/// 세팅 메뉴를 렌더링하기 위한 JSON 스키마 옵션을 생성합니다.
pub fn generate_app_settings_json(
    lang_val: &str,
    voice_val: &str,
    vol_val: &str,
    speed_val: &str,
) -> String {
    format!(
        r#"{{
            "items": {{
                "language": {{
                    "default": "0",
                    "value": "{lang_val}",
                    "value_list": {{ "Ko": "0", "En": "1", "Ja": "2" }}
                }},
                "tts_voice": {{
                    "default": "0",
                    "value": "{voice_val}",
                    "value_list": {{ "M1": "0", "M2": "1", "M3": "2", "M4": "3", "M5": "4", "F1": "5", "F2": "6", "F3": "7", "F4": "8", "F5": "9" }}
                }},
                "audio_volume": {{
                    "default": "50",
                    "value": "{vol_val}",
                    "value_list": {{ "10": "10", "20": "20", "30": "30", "40": "40", "50": "50", "60": "60", "70": "70", "80": "80", "90": "90", "100": "100" }}
                }},
                "speech_rate":{{
                    "default": "1",
                    "value": "{speed_val}",
                    "value_list": {{ "slow": "0", "normal": "1", "fast": "2"}}
                }}
            }}
        }}"#
    )
}
