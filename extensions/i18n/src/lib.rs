// 하위 사전 모듈 등록
pub mod clock;
pub mod data;
pub mod number;
pub mod warmer; // 캐시 워머 연동 제너레이터 모듈 추가

// 사용의 편의를 위해 모든 헬퍼 함수와 상수 데이터를 상위 수준(dict/mod.rs)에서 직접 재수출(Re-export)합니다.

// 시간, 일반 숫자 변환, 서수 변환 및 사전 캐싱 워머 헬퍼 함수 재수출
pub use clock::{h, m, num, period, s};
pub use number::{format_hour, format_minute, format_num, format_ordinal};
pub use warmer::generate_all_warmup_texts; // 워머 연동 함수 노출

// 다국어 상수 데이터셋 전체 재수출 (신규 서수 상수 포함)
pub use data::{
    ENG_ORDINAL_NUMBERS, ENG_TEENS, ENG_TENS, ENG_UNITS, JA_ORDINAL_NUMBERS, JA_TENS, JA_UNITS,
    KOR_NATIVE_DETS, KOR_NATIVE_NOUNS, KOR_ORDINAL_NUMBERS, KOR_SINO_TENS, KOR_SINO_UNITS,
};

/// 다국어 사전 에셋인 `data.json` 파일의 원본 JSON 텍스트를 반환합니다.
/// 컴파일 타임에 바이너리에 임베딩되므로 경로 문제 없이 안전하게 접근할 수 있습니다.
pub fn get_raw_dictionary_json() -> &'static str {
    include_str!("assets/data.json")
}
