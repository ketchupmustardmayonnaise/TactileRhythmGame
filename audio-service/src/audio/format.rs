use crate::error::{Error, Result};
use rodio::Source;
use std::io::Cursor;

/// 선형 보간(Linear Interpolation) 방식으로 샘플 레이트를 변환합니다.
///
/// 읽기 위치는 반드시 **f64 로, 그리고 누적이 아니라 `i * step` 으로** 계산합니다.
/// f32 로 `pos += step` 을 누적하면 pos 가 커질수록 유효자릿수가 모자라 오차가 쌓이고,
/// 그만큼 결과물의 길이가 늘어나 **재생 속도가 미세하게 느려집니다.**
/// (44.1kHz → 48kHz, 250만 샘플 기준 약 +0.28초. 곡이 뒤로 갈수록 점점 밀림)
///
/// 리듬 게임처럼 음악과 타이밍을 맞추는 애플릿에서는 이 드리프트가 그대로 판정 오차가 됩니다.
fn resample_linear(samples: &[f32], source_sample_rate: f32, target_sample_rate: f32) -> Vec<f32> {
    if (source_sample_rate - target_sample_rate).abs() <= 1.0 {
        return samples.to_vec();
    }

    let step = source_sample_rate as f64 / target_sample_rate as f64;
    let src_len = samples.len();
    let out_len = (src_len as f64 / step).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let pos = i as f64 * step;
        let idx = pos as usize;
        if idx + 1 < src_len {
            let frac = (pos - idx as f64) as f32;
            out.push(samples[idx] + (samples[idx + 1] - samples[idx]) * frac);
        } else if idx < src_len {
            out.push(samples[idx]);
        }
    }

    out
}

/// 임의의 오디오 포맷(WAV, MP3 등) 바이너리 데이터를 실시간으로 디코딩하고 리샘플링하여
/// f32 모노 PCM 샘플 배열로 변환합니다.
pub fn decode_to_pcm(
    audio_data: Vec<u8>,
    target_sample_rate: f32,
    volume: f32,
) -> Result<Vec<f32>> {
    match rodio::Decoder::new(Cursor::new(audio_data)) {
        Ok(source) => {
            let source_channels = source.channels().get() as usize;
            let source_sample_rate = source.sample_rate().get() as f32;
            let raw_samples: Vec<f32> = source.collect();

            // 1. 다채널 -> 모노(Mono) 다운믹스 진행
            let mut mono_samples = Vec::with_capacity(raw_samples.len() / source_channels);
            for frame in raw_samples.chunks(source_channels) {
                let mut sum = 0.0;
                for &sample in frame {
                    sum += sample;
                }
                mono_samples.push((sum / source_channels as f32) * volume);
            }

            // 2. 샘플 레이트 리샘플링 (음의 높낮이 및 속도 왜곡 방지)
            Ok(resample_linear(
                &mono_samples,
                source_sample_rate,
                target_sample_rate,
            ))
        }
        Err(e) => Err(Error::Io(std::io::Error::other(format!(
            "오디오 세그먼트 실시간 디코딩 작업에 실패했습니다: {}",
            e
        )))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 샘플 레이트가 같으면 원본을 그대로 돌려줘야 합니다.
    #[test]
    fn same_rate_is_passthrough() {
        let samples = vec![0.0, 0.5, -0.5, 1.0];
        let out = resample_linear(&samples, 48_000.0, 48_000.0);
        assert_eq!(out, samples);
    }

    /// 긴 음원에서도 길이가 정확해야 합니다.
    ///
    /// 과거 구현은 f32 로 위치를 누적해서, 44.1kHz -> 48kHz 로 약 57초짜리 곡을 변환하면
    /// 결과가 0.28초쯤 길어졌습니다. 재생이 그만큼 느려지므로 음악에 맞춘 판정이 뒤로 밀립니다.
    #[test]
    fn long_resample_keeps_duration() {
        const SRC_RATE: f32 = 44_100.0;
        const TGT_RATE: f32 = 48_000.0;
        // 약 56.7초 분량 (little_snowman.mp3 과 같은 규모)
        let src_len = 2_499_840usize;
        let samples = vec![0.0f32; src_len];

        let out = resample_linear(&samples, SRC_RATE, TGT_RATE);

        let ideal = src_len as f64 * (TGT_RATE as f64 / SRC_RATE as f64);
        let drift_secs = (out.len() as f64 - ideal) / TGT_RATE as f64;
        assert!(
            drift_secs.abs() < 0.001,
            "리샘플링 길이 오차가 너무 큽니다: {drift_secs:+.4}초 (출력 {}, 이상 {ideal:.0})",
            out.len()
        );
    }

    /// 보간 값 자체가 맞는지 — 2배로 업샘플링하면 중간값이 평균이어야 합니다.
    #[test]
    fn interpolates_between_neighbours() {
        let samples = vec![0.0, 1.0, 0.0];
        let out = resample_linear(&samples, 1_000.0, 2_000.0);
        assert!(out.len() >= 5);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!(
            (out[1] - 0.5).abs() < 1e-6,
            "중간값이 평균이어야 합니다: {}",
            out[1]
        );
        assert!((out[2] - 1.0).abs() < 1e-6);
        assert!(
            (out[3] - 0.5).abs() < 1e-6,
            "중간값이 평균이어야 합니다: {}",
            out[3]
        );
    }
}
