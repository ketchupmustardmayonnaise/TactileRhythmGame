// applets/tactile-game/src/pattern.rs

/// 하나의 문제 패턴에 대한 정보를 담고 있는 구조체입니다.
/// 각 문항별로 고정된 진동 세기와 정답의 위치(3지선다 중 선택 위치)를 나타냅니다.
pub struct Pattern {
    /// 정답에 해당하는 물리적 진동 세기 단계 (1..=5)
    pub vibration: u8,
    /// 3지선다 객관식 보기 중 정답의 실제 위치 (1-based index: 1..=3)
    pub answer_pos: usize,
}

/// 사용자가 지정한 최신 촉지게임 개발 기획서에 근거하여 작성한 1~10번 문항의 고정 문제 패턴 정의입니다.
/// 문제 번호는 배열 인덱스 0번(1번 문제)부터 순차적으로 매칭되어 출제됩니다.
/// 정답의 위치는 모두 1번에서 3번 사이로 정확히 매핑되었습니다.
pub const PATTERNS: [Pattern; 10] = [
    // 1번 문제: 진동세기 1, 정답 위치 3
    Pattern {
        vibration: 1,
        answer_pos: 3,
    },
    // 2번 문제: 진동세기 3, 정답 위치 1
    Pattern {
        vibration: 3,
        answer_pos: 1,
    },
    // 3번 문제: 진동세기 5, 정답 위치 2
    Pattern {
        vibration: 5,
        answer_pos: 2,
    },
    // 4번 문제: 진동세기 4, 정답 위치 3
    Pattern {
        vibration: 4,
        answer_pos: 3,
    },
    // 5번 문제: 진동세기 2, 정답 위치 1
    Pattern {
        vibration: 2,
        answer_pos: 1,
    },
    // 6번 문제: 진동세기 4, 정답 위치 1
    Pattern {
        vibration: 4,
        answer_pos: 1,
    },
    // 7번 문제: 진동세기 3, 정답 위치 2
    Pattern {
        vibration: 3,
        answer_pos: 2,
    },
    // 8번 문제: 진동세기 2, 정답 위치 3
    Pattern {
        vibration: 2,
        answer_pos: 3,
    },
    // 9번 문제: 진동세기 5, 정답 위치 2
    Pattern {
        vibration: 5,
        answer_pos: 2,
    },
    // 10번 문제: 진동세기 1, 정답 위치 2
    Pattern {
        vibration: 1,
        answer_pos: 2,
    },
];
