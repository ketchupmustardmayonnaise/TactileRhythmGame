use crate::error::{Error, Result};
use rodio::Source;
use std::io::Cursor;

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

            // 2. 샘플 레이트 리샘플링 (선형 보간을 사용하여 음의 높낮이 및 속도 왜곡 방지)
            let mut resampled_samples = Vec::new();
            if (source_sample_rate - target_sample_rate).abs() > 1.0 {
                let step = source_sample_rate / target_sample_rate;
                let mut pos = 0.0;
                let raw_len = mono_samples.len() as f32;
                while pos < raw_len {
                    let idx = pos as usize;
                    if idx + 1 < mono_samples.len() {
                        let frac = pos - idx as f32;
                        let val =
                            mono_samples[idx] + (mono_samples[idx + 1] - mono_samples[idx]) * frac;
                        resampled_samples.push(val);
                    } else if idx < mono_samples.len() {
                        resampled_samples.push(mono_samples[idx]);
                    }
                    pos += step;
                }
            } else {
                resampled_samples = mono_samples;
            }

            Ok(resampled_samples)
        }
        Err(e) => Err(Error::Io(std::io::Error::other(format!(
            "오디오 세그먼트 실시간 디코딩 작업에 실패했습니다: {}",
            e
        )))),
    }
}
