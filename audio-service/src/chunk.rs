use std::collections::HashSet;
use std::sync::OnceLock;

// ============================================================================
// 텍스트 청킹 (Text Chunking)
// ============================================================================

const MAX_CHUNK_LENGTH: usize = 300; // 기본 최대 청크 길이
#[allow(dead_code)]
const MIN_CHUNK_LENGTH: usize = 50; // 기본 최소 청크 길이

const ABBREVIATIONS: &[&str] = &[
    "Dr.", "Mr.", "Mrs.", "Ms.", "Prof.", "Sr.", "Jr.", "St.", "Ave.", "Rd.", "Blvd.", "Dept.",
    "Inc.", "Ltd.", "Co.", "Corp.", "etc.", "vs.", "i.e.", "e.g.", "Ph.D.",
];

/// 스마트 청킹 처리기
/// 구조체 기반의 상태 관리(State Machine)와 지연 평가(Lazy Evaluation) 방식의 슬라이싱을 융합합니다.
pub struct TextChunker {
    min_chunk_length: usize,
    max_chunk_length: usize,
}

impl TextChunker {
    /// 약어 사전을 초기화하여 메모리에 한 번만 로드합니다. (OnceLock 사용)
    /// 무거운 정규식/사전을 정적으로 초기화하여 오버헤드를 없앱니다.
    fn abbrev_set() -> &'static HashSet<&'static str> {
        static ABBREVS: OnceLock<HashSet<&'static str>> = OnceLock::new();
        ABBREVS.get_or_init(|| {
            let mut set = HashSet::new();
            for abbr in ABBREVIATIONS {
                set.insert(*abbr);
            }
            set
        })
    }

    pub fn new(min_chunk_length: usize) -> Self {
        Self {
            min_chunk_length,
            max_chunk_length: MAX_CHUNK_LENGTH,
        }
    }

    #[allow(dead_code)]
    /// 최대 길이를 지정할 수 있는 생성자
    pub fn with_max_length(min_chunk_length: usize, max_chunk_length: usize) -> Self {
        Self {
            min_chunk_length,
            max_chunk_length,
        }
    }

    /// 텍스트를 입력받아 TTS 엔진에 전달하기 좋은 크기의 청크 배열로 반환합니다.
    /// 문자열을 반복해서 새로 할당하지 않고, 원본 텍스트의 슬라이스(Slice) 뷰(View) 기반으로 순회합니다.
    pub fn chunk_text(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![String::new()];
        }

        let mut chunks: Vec<String> = Vec::new();
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut start_idx = 0; // chars 배열에서의 인덱스
        let abbrevs = Self::abbrev_set();

        while start_idx < chars.len() {
            let mut end_idx = start_idx;
            let mut current_len = 0;
            let mut found_boundary = false;
            let mut chunk_end_char_idx = start_idx;

            while end_idx < chars.len() {
                let (_, c) = chars[end_idx];
                current_len += 1;

                // 1단계: 최소 길이 보장 (누적)
                if current_len >= self.min_chunk_length {
                    // 2단계: 스마트 구두점 탐색 (약어 필터링)
                    if c == '.' || c == '!' || c == '?' || c == '\n' {
                        // 연속된 구두점 처리 (예: "...", "!?")
                        let mut punct_end = end_idx;
                        while punct_end + 1 < chars.len() {
                            let next_c = chars[punct_end + 1].1;
                            if next_c == '.' || next_c == '!' || next_c == '?' || next_c == '\n' {
                                punct_end += 1;
                                current_len += 1;
                            } else {
                                break;
                            }
                        }

                        let punct_byte_end = chars[punct_end].0 + chars[punct_end].1.len_utf8();
                        let text_up_to_punct = &text[chars[start_idx].0..punct_byte_end];

                        // 약어 판별 (가장 최근의 단어 추출하여 검사)
                        let is_abbrev = if c == '.' {
                            text_up_to_punct
                                .split_whitespace()
                                .next_back()
                                .is_some_and(|word| abbrevs.contains(word))
                        } else {
                            false
                        };

                        if !is_abbrev {
                            // 약어가 아니라면 그곳을 청크의 끝으로 확정하고 분할
                            found_boundary = true;
                            chunk_end_char_idx = punct_end;
                            break;
                        } else {
                            // 약어라면 자르지 않고 계속 누적
                            end_idx = punct_end;
                        }
                    }
                }

                // 3단계: 최대 길이 방어 (단계적 Fallback)
                if current_len >= self.max_chunk_length {
                    let mut fallback_idx = None;

                    // 현재 누적된 버퍼 안에서 역방향 탐색: 가장 가까운 쉼표(,) 찾기
                    for i in (start_idx + 1..=end_idx).rev() {
                        if chars[i].1 == ',' {
                            fallback_idx = Some(i);
                            break;
                        }
                    }

                    // 쉼표도 없다면 역방향으로 공백(띄어쓰기) 찾기
                    if fallback_idx.is_none() {
                        for i in (start_idx + 1..=end_idx).rev() {
                            if chars[i].1.is_whitespace() {
                                fallback_idx = Some(i);
                                break;
                            }
                        }
                    }

                    if let Some(idx) = fallback_idx {
                        found_boundary = true;
                        chunk_end_char_idx = idx;
                    } else {
                        // 최후의 예외 처리 (Hard Break): 쉼표/공백조차 발견하지 못했다면 최대 길이에서 강제 분할
                        found_boundary = true;
                        chunk_end_char_idx = end_idx;
                    }
                    break;
                }

                end_idx += 1;
            }

            if !found_boundary {
                // 문서의 끝에 도달하여 남은 텍스트 (자투리)
                chunk_end_char_idx = chars.len() - 1;
            }

            let byte_start = chars[start_idx].0;
            let byte_end = if chunk_end_char_idx + 1 < chars.len() {
                chars[chunk_end_char_idx + 1].0
            } else {
                text.len()
            };

            let chunk_str = text[byte_start..byte_end].trim();

            if !chunk_str.is_empty() {
                // 4단계: 자투리 병합 로직
                if !found_boundary
                    && chunk_str.chars().count() < self.min_chunk_length
                    && !chunks.is_empty()
                {
                    let last_idx = chunks.len() - 1;
                    let last_chunk = &chunks[last_idx];

                    // 이전 청크의 길이 + 자투리 텍스트의 길이 <= 최대 길이 검사
                    if last_chunk.chars().count() + chunk_str.chars().count()
                        < self.max_chunk_length
                    {
                        let merged = format!("{} {}", last_chunk, chunk_str);
                        chunks[last_idx] = merged;
                    } else {
                        // 최대 길이를 초과한다면 어색하더라도 자투리를 독립적인 청크로 내보내어 시스템 안정성 확보
                        chunks.push(chunk_str.to_string());
                    }
                } else {
                    chunks.push(chunk_str.to_string());
                }
            }

            start_idx = chunk_end_char_idx + 1;
        }

        if chunks.is_empty() {
            vec![String::new()]
        } else {
            chunks
        }
    }
}

#[allow(dead_code)]
// 하위 호환성을 위해 유지하는 함수 (기존 chunk_text)
pub fn chunk_text(text: &str, max_len: Option<usize>) -> Vec<String> {
    let max_len = max_len.unwrap_or(MAX_CHUNK_LENGTH);
    let min_len = MIN_CHUNK_LENGTH.min(max_len / 2);

    let chunker = TextChunker::with_max_length(min_len, max_len);
    chunker.chunk_text(text)
}
