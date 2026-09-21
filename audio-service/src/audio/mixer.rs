use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use crate::audio::track::{AudioState, Track};
use crate::audio::types::{PlaybackCategory, StreamCommand};

/// 오디오 명령 채널로부터 실시간 기기 명령들을 모두 수신하여 현재 오디오 출력 상태(AudioState)를 갱신합니다.
pub(crate) fn process_stream_commands(state: &mut AudioState, rx: &mpsc::Receiver<StreamCommand>) {
    // 수신 버퍼 큐에 쌓여 있는 모든 제어 명령을 비차단(try_recv) 방식으로 신속하게 전부 처리합니다.
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            StreamCommand::Play {
                samples,
                step,
                category,
                volume,
                clear_previous,
            } => {
                if !samples.is_empty() {
                    let is_effect = category == PlaybackCategory::SoundEffect;
                    // 효과음(SoundEffect) 카테고리인 경우에는 볼륨 크기를 10분의 1로 감쇠하여 트랙별 볼륨에 할당합니다.
                    // 기존처럼 마스터 시스템 볼륨(state.volume)을 강제 덮어쓰지 않고 개별 트랙 인스턴스에만 국소적으로 적용합니다.
                    // let track_volume = if is_effect { volume / 2.0 } else { volume };

                    // 덮어쓰기 (초기화) 모드 작동 시: 효과음이 아니고, 이전 트랙 삭제 옵션이 켜져 있다면
                    // 현재 재생되고 있는 동일 카테고리의 트랙을 우선 제거합니다.
                    if !is_effect && clear_previous {
                        state.tracks.retain(|t| t.category != category);
                    }

                    // 이어붙이기 (Append) 모드 작동 시: 동일 카테고리의 트랙이 이미 가동 중이고 효과음이 아닌 경우
                    // 기존 트랙의 내부 샘플 큐 맨 뒤에 새 샘플 리스트를 이어붙여 연속 재생되도록 유도합니다.
                    if !is_effect
                        && !clear_previous
                        && let Some(track) =
                            state.tracks.iter_mut().find(|t| t.category == category)
                    {
                        track.samples_queue.push_back(samples);
                        continue; // 기존 재생 상태를 연계 유지하므로 여기서 다음 명령으로 건너뜁니다.
                    }

                    // 지나치게 많은 트랙이 생성되어 CPU 부하가 걸리는 것을 철저히 예방하기 위해 동시 믹싱 트랙 수를 제한합니다.
                    const MAX_TRACKS: usize = 4;
                    if state.tracks.len() >= MAX_TRACKS {
                        // 허용한도 초과 시, 가장 제거하기 적합한 효과음(SoundEffect) 트랙을 우선적으로 퇴출시킵니다.
                        if let Some(idx) = state
                            .tracks
                            .iter()
                            .position(|t| t.category == PlaybackCategory::SoundEffect)
                        {
                            state.tracks.remove(idx);
                        } else {
                            // 모두 필수 성격의 안내음 트랙이라면 가장 오래 전에 시작된 트랙을 강제로 제거하여 자리를 만듭니다.
                            state.tracks.remove(0);
                        }
                    }

                    // 새로운 재생 트랙을 준비하여 상태 리스트에 등록하고, 즉시 재생 중 원자 플래그를 활성화합니다.
                    let mut queue = VecDeque::new();
                    queue.push_back(samples);
                    state.tracks.push(Track {
                        samples_queue: queue,
                        pos: 0.0,
                        step,
                        category,
                        volume,
                        // volume: track_volume,
                    });
                    state.is_playing.store(true, Ordering::Release);
                }
            }
            StreamCommand::Clear(category) => {
                // 특정 카테고리에 속하는 대기/연주 트랙들을 완전히 소거합니다.
                state.tracks.retain(|t| t.category != category);
            }
            StreamCommand::SetVolume(volume) => {
                // 마스터 시스템 볼륨 값을 조정합니다.
                state.volume = volume;
            }
        }
    }
}

/// 오디오 장치의 오디오 출력 버퍼를 채우기 위해 단일 프레임 F32 단위의 샘플 값을 실시간으로 계산하고 믹싱하여 산출합니다.
/// 하드웨어 디코딩 부하, 볼륨 페이드 이펙트(더킹) 및 오디오 소리 깨짐 방지용 연산(소프트 클리핑)이 포함됩니다.
pub(crate) fn next_sample(state: &mut AudioState) -> f32 {
    let volume = state.volume;

    // 재생 리스트에 핵심 안내음(TtsImportant)이 포함되어 있다면 일시적인 사운드 페이드 연산(더킹)을 적용하도록 설정합니다.
    let has_important = state
        .tracks
        .iter()
        .any(|t| t.category == PlaybackCategory::TtsImportant);

    // 더킹 목표 볼륨을 안전하게 선언합니다. 중요 안내음이 들릴 때는 다른 카테고리의 소리를 20%(0.2) 수준으로 낮춥니다.
    let ducking_target = if has_important { 0.2 } else { 1.0 };
    // 약 100밀리초(0.1초) 동안 부드러운 아날로그 페이드인/아웃 감각을 연출하기 위해 슬루 리미터 연산을 진행합니다.
    let fade_time_secs = 0.1;
    let slew_rate = 1.0 / (state.sample_rate * fade_time_secs);
    if state.ducking_multiplier < ducking_target {
        state.ducking_multiplier = (state.ducking_multiplier + slew_rate).min(ducking_target);
    } else if state.ducking_multiplier > ducking_target {
        state.ducking_multiplier = (state.ducking_multiplier - slew_rate).max(ducking_target);
    }

    let mut mixed_sample = 0.0;
    let mut i = 0;

    // 현재 기동하고 있는 모든 트랙들을 순회하며 F32 PCM 선형 중첩 믹싱 연산을 처리합니다.
    while i < state.tracks.len() {
        let track = &mut state.tracks[i];

        // 1. 이미 전량 연주가 끝난 무의미한 소리 청크(Chunk)들은 큐에서 안전하게 제거하고 대기열 포인터를 재정렬합니다.
        while !track.samples_queue.is_empty() {
            let current_chunk_len = track.samples_queue.front().unwrap().len() as f32;
            if track.pos >= current_chunk_len {
                track.samples_queue.pop_front();
                track.pos -= current_chunk_len;
            } else {
                break;
            }
        }

        // 2. 고정밀 선형 보간(Linear Interpolation)을 통한 서브 샘플 보정 연산
        let sample = if let Some(current_chunk) = track.samples_queue.front() {
            let idx = track.pos as usize;
            let frac = track.pos - idx as f32;
            let s1 = current_chunk[idx];

            // 배열 경계를 넘나들 때에도 잡음 없이 부드럽게 재생되도록 다음 청크의 첫 프레임과 교차 보간합니다.
            let s2 = if idx + 1 < current_chunk.len() {
                current_chunk[idx + 1]
            } else if let Some(next_chunk) = track.samples_queue.get(1) {
                next_chunk[0]
            } else {
                s1
            };

            track.pos += track.step;
            s1 + (s2 - s1) * frac
        } else {
            0.0
        };

        // 3. 중요 안내음 재생 시, 타 트랙에 한정하여 감쇠 연산(더킹)을 고르게 곱해주고 마스터 볼륨과 합산합니다.
        let track_volume = if track.category != PlaybackCategory::TtsImportant {
            state.ducking_multiplier
        } else {
            1.0
        };
        mixed_sample += sample * track_volume * track.volume;

        // 버퍼를 모두 소비하여 재생 완료된 빈 트랙은 리스트에서 말끔히 소거시킵니다.
        if track.samples_queue.is_empty() {
            state.tracks.remove(i);
        } else {
            i += 1;
        }
    }

    // 더 이상 연주할 소리 조각이 없다면 활성화 플래그를 원자적으로 정상 해제합니다.
    if state.tracks.is_empty() && state.is_playing.load(Ordering::Relaxed) {
        state.is_playing.store(false, Ordering::Relaxed);
    }

    // 4. 고음량 재생 시 하드웨어 한계를 넘어서 소리가 찢어지고 튀는 현상을 보호하는 '소프트 클리핑(Soft clipping)' 적용
    let threshold = 0.8;
    let abs_sample = mixed_sample.abs();
    let soft_clipped = if abs_sample <= threshold {
        mixed_sample
    } else {
        let sign = mixed_sample.signum();
        let excess = abs_sample - threshold;
        let compressed = threshold + (excess / (1.0 + excess / (1.0 - threshold)));
        sign * compressed
    };

    soft_clipped * volume
}

/// f32 부동소수점 데이터 형식 전용 하드웨어 버퍼를 채우는 채널 콜백 바인딩입니다.
pub(crate) fn write_data_f32(
    output: &mut [f32],
    channels: usize,
    state: &mut AudioState,
    rx: &mpsc::Receiver<StreamCommand>,
) {
    process_stream_commands(state, rx);
    for frame in output.chunks_mut(channels) {
        let mut sample = next_sample(state);
        if sample.is_nan() || sample.is_infinite() {
            sample = 0.0;
        } else {
            sample = sample.clamp(-1.0, 1.0);
        }
        for sample_out in frame.iter_mut() {
            *sample_out = sample;
        }
    }
}

/// i16 부호 있는 16비트 정수 형식 하드웨어 버퍼를 채우는 채널 콜백 바인딩입니다.
pub(crate) fn write_data_i16(
    output: &mut [i16],
    channels: usize,
    state: &mut AudioState,
    rx: &mpsc::Receiver<StreamCommand>,
) {
    process_stream_commands(state, rx);
    for frame in output.chunks_mut(channels) {
        let sample = next_sample(state);
        let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        for sample_out in frame.iter_mut() {
            *sample_out = sample_i16;
        }
    }
}

/// u16 부호 없는 16비트 정수 형식 하드웨어 버퍼를 채우는 채널 콜백 바인딩입니다.
pub(crate) fn write_data_u16(
    output: &mut [u16],
    channels: usize,
    state: &mut AudioState,
    rx: &mpsc::Receiver<StreamCommand>,
) {
    process_stream_commands(state, rx);
    for frame in output.chunks_mut(channels) {
        let sample = next_sample(state);
        let sample_u16 = ((sample.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
        for sample_out in frame.iter_mut() {
            *sample_out = sample_u16;
        }
    }
}

/// i32 부호 있는 32비트 정수 형식 하드웨어 버퍼를 채우는 채널 콜백 바인딩입니다.
pub(crate) fn write_data_i32(
    output: &mut [i32],
    channels: usize,
    state: &mut AudioState,
    rx: &mpsc::Receiver<StreamCommand>,
) {
    process_stream_commands(state, rx);
    for frame in output.chunks_mut(channels) {
        let sample = next_sample(state);
        let sample_i32 = (sample.clamp(-1.0, 1.0) * 2147483647.0) as i32;
        for sample_out in frame.iter_mut() {
            *sample_out = sample_i32;
        }
    }
}

/// f64 고정밀 부동소수점 데이터 형식 하드웨어 버퍼를 채우는 채널 콜백 바인딩입니다.
pub(crate) fn write_data_f64(
    output: &mut [f64],
    channels: usize,
    state: &mut AudioState,
    rx: &mpsc::Receiver<StreamCommand>,
) {
    process_stream_commands(state, rx);
    for frame in output.chunks_mut(channels) {
        let mut sample = next_sample(state) as f64;
        if sample.is_nan() || sample.is_infinite() {
            sample = 0.0;
        } else {
            sample = sample.clamp(-1.0, 1.0);
        }
        for sample_out in frame.iter_mut() {
            *sample_out = sample;
        }
    }
}
