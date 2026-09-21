mod format;
mod mixer;
mod player;
// mod save;
mod track;
/// 오디오 서비스의 하위 컴포넌트들을 통합 관리하고 외부 인터페이스를 재노출하는 진입점 모듈 파일입니다.
///
/// 이 모듈은 기존에 혼재되어 있던 단일 거대 파일(audio.rs)을 고유한 책임을 갖는 개별 서브 모듈 파일들로
/// 나누어 안전하게 분할 전개하고 가독성 및 정비 편의성을 개선한 구조를 지닙니다.
/// 또한, 기존 외부 바인딩(예: src/main.rs 등)들과의 코드 정합성을 완벽히 유지하기 위해 공개 심볼들을 그대로 재노출(Re-export)합니다.
mod types;
// 다른 컴포넌트나 외부 크레이트들이 기존 경로대로 끊김 없이 동일하게 접근하여 활용할 수 있도록 주요 API를 일제히 재노출합니다.
pub use format::decode_to_pcm;
pub use player::AudioPlayer;
pub use types::PlaybackCategory;
