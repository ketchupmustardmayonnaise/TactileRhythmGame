use crate::clock;
use sdk::types::Language;

/// 다국어별로 캐시 워밍(사전 생성)이 필요한 공통 '의미 뭉치(Semantic Chunk) 단어 목록'을 취합하여 생성합니다.
/// 이 리스트는 캐시 워머가 실행될 때 모든 지원 언어, 목소리, 속도 조합에 걸쳐서 일괄 사전 생성 대상이 됩니다.
///
/// # 반환값
/// 공백 제거 및 중복이 제거된 정렬된 다국어 발음 빌딩 블록 문자열 리스트 (`Vec<String>`)
pub fn generate_all_warmup_texts(lang: Language) -> Vec<String> {
    let mut targets = Vec::new();

    // 1. 시간(Clock) 사전 관련 수량사 빌딩 블록 수집
    targets.extend(generate_clock_warmup_texts(lang));

    // 2. 일반 숫자(Number) 사전 관련 발음 빌딩 블록 수집 (0~99 범위)
    targets.extend(generate_number_warmup_texts(lang));

    // 3. 서수(Ordinal) 사전 관련 발음 빌딩 블록 수집 (1~10 범위)
    targets.extend(generate_ordinal_warmup_texts(lang));

    // 4. [추후 확장] 길이(Length) 사전이 추가되는 경우 주석을 풀고 목록에 연결합니다.
    // targets.extend(super::length::generate_warmup_texts(lang));

    // 4. [추후 확장] 돈(Money) 사전이 추가되는 경우 주석을 풀고 목록에 연결합니다.
    // targets.extend(super::money::generate_warmup_texts(lang));

    // --- 안전 장치 및 후처리 ---
    // 빈 문자열 제거
    targets.retain(|s| !s.trim().is_empty());

    // 중복 연산 방지 및 깔끔한 조회를 위해 정렬 후 중복 제거 실행
    targets.sort();
    targets.dedup();

    targets
}

/// 시간(Clock) 사전 모듈에서 필요한 모든 단위 발음 블록(오전/오후, 한시~열두시, 영분~오십구분 등)을 생성합니다.
fn generate_clock_warmup_texts(lang: Language) -> Vec<String> {
    let mut list = Vec::new();

    // 오전 및 오후 텍스트 추가
    list.push(clock::period(lang, 0).to_string()); // AM / 오전 / 午前
    list.push(clock::period(lang, 12).to_string()); // PM / 오후 / 午後

    // 1시 ~ 12시 뭉치 추가 (예: "한시", "열두시", "よじ" 등)
    for h in 1..=12 {
        list.push(clock::h(lang, h));
    }

    // 0분 ~ 59분 뭉치 추가 (예: "영분", "삼십분", "いっぷん", "Zero", "Fifteen" 등)
    for m in 0..=59 {
        list.push(clock::m(lang, m));
    }

    // 0초 ~ 59초 뭉치 추가 (예: "영초", "사십오초", "いちびょう", "One second", "Fifteen seconds" 등)
    for s in 0..=59 {
        list.push(clock::s(lang, s));
    }

    list
}

/// 일반 숫자(Number) 사전 모듈에서 필요한 0부터 99까지의 일반 다국어 숫자 발음들을 생성합니다.
/// (단독 숫자 및 수치가 필요한 맥락을 사전 캐싱 처리해 둡니다)
fn generate_number_warmup_texts(lang: Language) -> Vec<String> {
    let mut list = Vec::new();

    // 0부터 99까지의 일반 숫자 발음 추가 (예: "영", "일", ..., "구십구", "Zero", "One", ..., "Ninety-nine")
    for n in 0..=50 {
        list.push(clock::num(lang, n));
    }

    list
}

/// 서수(Ordinal) 사전 모듈에서 필요한 1부터 10까지의 다국어 서수 발음들을 생성합니다.
fn generate_ordinal_warmup_texts(lang: Language) -> Vec<String> {
    let mut list = Vec::new();

    // 1부터 10까지의 서수 발음 추가 (예: "첫 번째", "first", "第一 (だいいち)" 등)
    for rank in 1..=10 {
        list.push(crate::format_ordinal(lang, rank));
    }

    list
}
