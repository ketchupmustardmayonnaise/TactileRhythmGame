use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

use crate::config::ServiceConfig;
use crate::error::{Error, Result};

/// piper-rs가 다중 문자 음소 키(예: "aɪ")를 char로 역직렬화할 수 없어 발생하는 오류를 방지하기 위한 안전한 로더 함수
fn load_piper(model_path: &Path, config_path: &Path) -> Result<piper_rs::Piper> {
    let file = std::fs::File::open(config_path).map_err(|e| {
        Error::Piper(format!(
            "설정 파일 열기 실패 `{}`: {}",
            config_path.display(),
            e
        ))
    })?;

    let mut val: serde_json::Value = serde_json::from_reader(file).map_err(|e| {
        Error::Piper(format!(
            "설정 JSON 파싱 실패 `{}`: {}",
            config_path.display(),
            e
        ))
    })?;

    if let Some(map_obj) = val.get_mut("phoneme_id_map").and_then(|v| v.as_object_mut()) {
        let multi_keys: Vec<String> = map_obj
            .keys()
            .filter(|k| k.chars().count() != 1)
            .cloned()
            .collect();

        for k in multi_keys {
            if let Some(v) = map_obj.remove(&k) {
                if let Some(first_char) = k.chars().next() {
                    let first_str = first_char.to_string();
                    map_obj.entry(first_str).or_insert(v);
                }
            }
        }
    }

    let config: piper_rs::ModelConfig = serde_json::from_value(val).map_err(|e| {
        Error::Piper(format!(
            "ModelConfig 변환 실패 `{}`: {}",
            config_path.display(),
            e
        ))
    })?;

    let session = ort::session::Session::builder()
        .map_err(|e| Error::Piper(format!("Session builder 생성 실패: {}", e)))?
        .commit_from_file(model_path)
        .map_err(|e| {
            Error::Piper(format!(
                "모델 세션 로드 실패 `{}`: {}",
                model_path.display(),
                e
            ))
        })?;

    Ok(piper_rs::Piper::from_session(session, config))
}

/// Piper TTS 백엔드 엔진
pub struct PiperEngine {
    pipers: HashMap<String, Mutex<piper_rs::Piper>>,
    default_piper: Option<Mutex<piper_rs::Piper>>,
}

impl PiperEngine {
    pub fn new(asset_dir: &Path) -> Result<Self> {
        info!("PiperEngine 초기화 중 (경로: {:?})", asset_dir);
        let mut pipers = HashMap::new();

        let lang_keys = [
            ("Ko", "piper_ko.onnx", "piper_ko.onnx.json"),
            ("En", "piper_en.onnx", "piper_en.onnx.json"),
            ("Ja", "piper_ja.onnx", "piper_ja.onnx.json"),
        ];

        for (lang, model_file, config_file) in lang_keys {
            let model_path = asset_dir.join(model_file);
            let config_path = asset_dir.join(config_file);

            if model_path.exists() && config_path.exists() {
                match load_piper(&model_path, &config_path) {
                    Ok(piper) => {
                        info!("Piper 모델 로드 성공 (언어: {}, 모델: {:?})", lang, model_path);
                        pipers.insert(lang.to_string(), Mutex::new(piper));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Piper 로드 실패 ({:?}, {:?}): {}",
                            model_path,
                            config_path,
                            e
                        );
                    }
                }
            }
        }

        let mut default_piper = None;
        let default_model_path = asset_dir.join("piper.onnx");
        let default_config_path = asset_dir.join("piper.onnx.json");
        if default_model_path.exists() && default_config_path.exists() {
            if let Ok(piper) = load_piper(&default_model_path, &default_config_path) {
                info!("Piper 기본 모델 로드 성공 ({:?})", default_model_path);
                default_piper = Some(Mutex::new(piper));
            }
        }

        if default_piper.is_none() && pipers.is_empty() {
            if let Ok(entries) = std::fs::read_dir(asset_dir) {
                let paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
                for path in &paths {
                    if path.extension().and_then(|s| s.to_str()) == Some("onnx") {
                        let json_path = path.with_extension("onnx.json");
                        let fallback_json = path.with_extension("json");
                        let cfg_path = if json_path.exists() {
                            Some(json_path)
                        } else if fallback_json.exists() {
                            Some(fallback_json)
                        } else {
                            None
                        };

                        if let Some(cfg) = cfg_path {
                            if let Ok(piper) = load_piper(path, &cfg) {
                                info!("Piper 대체 모델 로드 성공 ({:?})", path);
                                default_piper = Some(Mutex::new(piper));
                                break;
                            }
                        }
                    }
                }
            }
        }

        if default_piper.is_none() && pipers.is_empty() {
            tracing::warn!(
                "Piper 모델 파일(.onnx, .onnx.json)을 {:?} 에서 찾을 수 없습니다.",
                asset_dir
            );
        }

        Ok(Self {
            pipers,
            default_piper,
        })
    }

    pub fn synthesize(&self, text: &str, lang: &str) -> Result<(Vec<f32>, u32)> {
        let mutex = self
            .pipers
            .get(lang)
            .or(self.default_piper.as_ref())
            .ok_or_else(|| {
                Error::Piper(format!(
                    "언어 '{}'에 대한 Piper 모델 및 기본 Piper 모델이 존재하지 않습니다.",
                    lang
                ))
            })?;

        let mut piper = mutex
            .lock()
            .map_err(|_| Error::Piper("Piper 뮤텍스 락 획득 실패".to_string()))?;

        let (samples, sample_rate) = piper
            .create(text, false, None, None, None, None)
            .map_err(|e| Error::Piper(format!("{}", e)))?;

        Ok((samples, sample_rate))
    }
}

/// 소스 샘플 레이트(예: 22050Hz)에서 타겟 재생 기기 샘플 레이트(예: 48000Hz)로 선형 보간 리샘플링
fn resample_pcm(samples: &[f32], source_sr: f32, target_sr: f32) -> Vec<f32> {
    if (source_sr - target_sr).abs() <= 1.0 || samples.is_empty() || target_sr <= 0.0 {
        return samples.to_vec();
    }
    let step = source_sr / target_sr;
    let mut resampled = Vec::with_capacity((samples.len() as f32 / step) as usize + 1);
    let mut pos = 0.0;
    let raw_len = samples.len() as f32;
    while pos < raw_len {
        let idx = pos as usize;
        if idx + 1 < samples.len() {
            let frac = pos - idx as f32;
            let val = samples[idx] + (samples[idx + 1] - samples[idx]) * frac;
            resampled.push(val);
        } else if idx < samples.len() {
            resampled.push(samples[idx]);
        }
        pos += step;
    }
    resampled
}

/// TTS 요청 메시지
pub struct TtsMessage {
    pub text: String,
    pub speed: f32,
    #[allow(dead_code)]
    pub voice_type: String,
    /// 요청된 재생 언어 설정
    pub language: sdk::Language,
    /// 타겟 사운드 출력 기기의 샘플 레이트 (예: 48000Hz, 44100Hz)
    pub target_sample_rate: f32,
    pub respond_to: oneshot::Sender<Result<Vec<f32>>>,
}

/// 엔진 설정 업데이트 메시지
pub struct UpdateSettingsMessage {
    pub voice: String,
    pub language: String,
    pub respond_to: oneshot::Sender<Result<()>>,
}

/// TTS 액터 명령어
pub enum TtsCommand {
    Synthesize(TtsMessage),
    UpdateSettings(UpdateSettingsMessage),
}

/// TTS 엔진 액터
pub struct TtsActor {
    engine: PiperEngine,
    rx: mpsc::Receiver<TtsCommand>,
    current_language: sdk::Language,
    shifter: pitch_shift::Shifter<Box<[f32; pitch_shift::TOTAL_F32]>>,
}

impl TtsActor {
    /// 액터 생성 및 실행
    pub async fn spawn(config: ServiceConfig) -> Result<mpsc::Sender<TtsCommand>> {
        let (tx, rx) = mpsc::channel(32);

        let engine = tokio::task::spawn_blocking(move || -> Result<PiperEngine> {
            info!(
                "TtsActor (Piper) 초기화 중 (에셋 경로: {:?})",
                config.piper_asset_dir
            );
            PiperEngine::new(&config.piper_asset_dir)
        })
        .await
        .map_err(|e| Error::ActorSend(format!("태스크 조인 실패: {}", e)))??;

        let actor = TtsActor {
            engine,
            rx,
            current_language: sdk::Language::Ko,
            shifter: pitch_shift::Shifter::new(Box::new([0.0_f32; pitch_shift::TOTAL_F32])),
        };

        std::thread::spawn(move || {
            actor.run();
        });

        Ok(tx)
    }

    /// 액터 실행 루프
    fn run(mut self) {
        info!("TtsActor 처리 루프 시작됨.");

        while let Some(cmd) = self.rx.blocking_recv() {
            match cmd {
                TtsCommand::Synthesize(msg) => {
                    let result = self.handle_synthesize(&msg);
                    let _ = msg.respond_to.send(result);
                }
                TtsCommand::UpdateSettings(msg) => {
                    let result = self.handle_update_settings(&msg);
                    let _ = msg.respond_to.send(result);
                }
            }
        }

        info!("TtsActor 중지됨.");
    }

    /// 메시지 처리
    fn handle_synthesize(&mut self, msg: &TtsMessage) -> Result<Vec<f32>> {
        let lang_str = match msg.language {
            sdk::Language::Ko => "Ko",
            sdk::Language::En => "En",
            sdk::Language::Ja => "Ja",
        };

        let (raw_audio, sample_rate) = self.engine.synthesize(&msg.text, lang_str)?;

        // 1. 타겟 출력 기기의 샘플 레이트로 선형 보간 리샘플링 (예: Piper 22050Hz -> 출력 기기 48000Hz 보정)
        let resampled_audio = if msg.target_sample_rate > 0.0 {
            resample_pcm(&raw_audio, sample_rate as f32, msg.target_sample_rate)
        } else {
            raw_audio
        };

        // 2. pitch_shift를 이용한 Time-stretching (재생 속도 변환, 음정 유지)
        let final_audio = if (msg.speed - 1.0).abs() > f32::EPSILON {
            let out_samples = ((128.0_f32 / msg.speed).round() as usize).clamp(1, 1023);
            let sr_f32 = if msg.target_sample_rate > 0.0 {
                msg.target_sample_rate
            } else {
                sample_rate as f32
            };

            let mut stretched_audio = Vec::new();

            for chunk in resampled_audio.chunks(128) {
                if chunk.len() == 128 {
                    let out = self.shifter.shift(chunk, 0.0, out_samples, sr_f32);
                    stretched_audio.extend_from_slice(out);
                } else {
                    let mut padded = [0.0; 128];
                    padded[..chunk.len()].copy_from_slice(chunk);
                    let out = self.shifter.shift(&padded, 0.0, out_samples, sr_f32);
                    let valid_out_len =
                        (out.len() as f32 * (chunk.len() as f32 / 128.0)) as usize;
                    stretched_audio.extend_from_slice(&out[..valid_out_len]);
                }
            }
            stretched_audio
        } else {
            resampled_audio
        };

        Ok(final_audio)
    }

    fn handle_update_settings(&mut self, msg: &UpdateSettingsMessage) -> Result<()> {
        self.current_language = match msg.language.as_str() {
            "Ko" => sdk::Language::Ko,
            "En" => sdk::Language::En,
            "Ja" => sdk::Language::Ja,
            _ => {
                info!("알 수 없는 언어 '{}', 기본값 Ko 사용", msg.language);
                sdk::Language::Ko
            }
        };

        info!(
            "TTS 엔진 설정 업데이트 됨: 언어={:?}, 목소리={:?}",
            msg.language, msg.voice
        );

        Ok(())
    }
}
