/// 오디오 트랙의 재생 속성을 구분하는 열거형입니다.
/// 각 소리가 재생되는 방식과 우선순위를 제어하는 핵심적인 분류 기준이 됩니다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaybackCategory {
    /// 일반 음성 합성 안내음 (동일 종류의 음성이 서로 겹쳐 들리지 않도록 기존 일반 안내음만 방지합니다)
    TtsNormal,
    /// 중요한 음성 합성 안내음 (재생 도중 다른 오디오 음량을 줄이는 '더킹'을 수행하며, 동일 종류 겹침을 방지합니다)
    TtsImportant,
    /// 효과음 (어떠한 겹침 방지 규칙도 적용받지 않고, 여러 소리를 동시에 즉시 혼합하여 출력할 수 있습니다)
    SoundEffect,
}

/// 백그라운드에서 오디오를 렌더링하는 전 전용 스레드로 요청을 전달할 때 사용하는 명령어 목록입니다.
pub enum AudioCommand {
    /// 원본 부동소수점 PCM 샘플 어레이를 직접 제공받아 재생합니다. (하드웨어 포맷에 맞춘 리샘플링 및 채널 변환만 동작함)
    PlayRaw {
        /// 실시간으로 연주할 가공이 완료된 F32 실수 샘플 벡터 데이터
        samples: Vec<f32>,
        /// 전달된 원본 오디오 소스의 고유 샘플 레이트 (Hz 단위)
        sample_rate: f32,
        /// 재생 우선순위 카테고리
        category: PlaybackCategory,
        /// 출력할 마스터 소리 크기 배율 (0.0 ~ 1.0)
        volume: f32,
        /// 새로운 소리를 낼 때 기존에 재생 중이던 동일 카테고리 트랙을 비울지 여부
        clear_previous: bool,
    },
    /// 지정된 카테고리의 모든 활성 재생 트랙을 즉시 정지하고 메모리 큐에서 삭제합니다.
    Clear(PlaybackCategory),
    /// 전체 기기 오디오 출력 마스터 볼륨을 조절합니다. (0.0: 무음 ~ 1.0: 원본 최대 크기)
    SetVolume(f32),
}

/// CPAL 오디오 출력 하드웨어 콜백 측의 실시간 스트림 렌더러와
/// 커맨드 대기 루프 사이에서 초고속 비차단 스트림 연동을 위해 사용하는 제어 명령어입니다.
#[derive(Debug)]
pub(crate) enum StreamCommand {
    /// CPAL 실시간 오디오 스트림 출력을 위한 새 재생 트랙 삽입 요청
    Play {
        /// 가공이 끝난 F32 실수 샘플 배열
        samples: Vec<f32>,
        /// 타겟 주파수 비율에 맞추어 보간 탐색 프레임을 제어할 세밀한 증분 단계량 (Step)
        step: f32,
        /// 재생 우선순위 카테고리
        category: PlaybackCategory,
        /// 이 트랙 고유의 개별 볼륨 값
        volume: f32,
        /// 동일 카테고리의 이전 재생 트랙 제거 여부
        clear_previous: bool,
    },
    /// 실시간 스트림 렌더러에서 특정 재생 카테고리 트랙들을 즉각 폐기
    Clear(PlaybackCategory),
    /// 실시간 스트림의 최종 혼합 마스터 음량 수정
    SetVolume(f32),
}
