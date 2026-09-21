use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::audio::types::PlaybackCategory;

/// 현재 재생 대기열에서 가동 중인 개별 음원 트랙의 세부 명세입니다.
pub(crate) struct Track {
    /// 실시간으로 인코딩/디코딩되어 순차적으로 재생될 F32 실수 PCM 데이터 버퍼들의 적재 대기열
    pub(crate) samples_queue: VecDeque<Vec<f32>>,
    /// 현재 재생 중인 오디오 프레임 버퍼 내부의 정밀한 탐색 포인터 인덱스 위치 (실수형 값으로 보간 계산에 활용됨)
    pub(crate) pos: f32,
    /// 디바이스 재생 속도(샘플 레이트) 비율에 맞춰 프레임을 건너뛰거나 조밀하게 읽기 위해 적용할 정밀 증분 값
    pub(crate) step: f32,
    /// 이 트랙이 속해 있는 우선순위 카테고리
    pub(crate) category: PlaybackCategory,
    /// 이 트랙의 개별 재생 음량 배율
    pub(crate) volume: f32,
}

/// CPAL 하드웨어 오디오 스트림 콜백 컨텍스트 내부에서 실시간으로 참조하고 갱신하는 렌더러 전용 상태 정보 저장 구조체입니다.
pub(crate) struct AudioState {
    /// 현재 사운드 드라이버에서 재생 혼합(믹싱) 중인 활성 트랙들의 컬렉션 리스트
    pub(crate) tracks: Vec<Track>,
    /// 시스템 최종 마스터 출력 음량 (0.0 ~ 1.0)
    pub(crate) volume: f32,
    /// 현재 장치에서 오디오 소리가 물리적으로 흘러나오고 있는지 여부를 외부 스레드와 안전하게 동기화하기 위한 원자적 불리언 플래그
    pub(crate) is_playing: Arc<AtomicBool>,
    /// 중요 오디오(TtsImportant) 재생 시 다른 카테고리의 소리를 일시적으로 서서히 페이드 아웃시키기 위한 더킹 볼륨 배율
    pub(crate) ducking_multiplier: f32,
    /// 현재 초기화되어 사용 중인 물리 오디오 장비의 출력 주파수 샘플 레이트 (Hz)
    pub(crate) sample_rate: f32,
}
