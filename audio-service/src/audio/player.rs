use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::audio::mixer::{
    write_data_f32, write_data_i16, write_data_u16,
};
use crate::audio::track::AudioState;
use crate::audio::types::{AudioCommand, PlaybackCategory, StreamCommand};
use crate::error::{Error, Result};

/// 외부 서비스 및 메인 애플리케이션 프레임워크가 실시간 음성 및 효과음을 조작하기 위해 호출하는 전역 재생기 구조체입니다.
/// 메모리 내의 가공된 오디오 데이터를 채널 통신을 통해 수신하여 하드웨어 드라이버로 전달하고 조율합니다.
#[derive(Clone)]
pub struct AudioPlayer {
    /// 백그라운드 오디오 디바이스 제어 스레드로 명령을 송신하기 위한 송신 채널 엔드포인트
    tx: mpsc::Sender<AudioCommand>,
    /// 현재 재생기 인스턴스가 렌더링하고 있는 기기 기본 샘플 레이트 (Hz)
    pub sample_rate: f32,
    /// 실시간 음성 합성 디코딩의 연산 과부하를 억제하기 위한 동시성 제어 세마포어
    decode_semaphore: Arc<Semaphore>,
    // 현재 활성화되어 백그라운드에서 사운드를 로드하고 가공하고 있는 비동기 조인 핸들(JoinHandle) 리스트 관리자
    /// TtsNormal  
    normal_tasks: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// TtsImportant
    important_tasks: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    // SoundEffect <- 나중에 볼륨조절 로직으로추가. 더킹으로 합성할꺼면 그냥 deploy시 그냥 wav나 pcm으로 만들거나 소리 크기별로 만들어놔도 될듯.
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    /// 새로운 오디오 재생기 인스턴스를 정상 구축하고, 하드웨어 음향 출력 백그라운드 스레드를 즉시 시동합니다.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        // 오디오 스레드 외부(초기화하는 메인 시점)에서 사운드 카드의 실제 물리적 오디오 장치 기본 샘플 레이트를 미리 조회해 둡니다.
        let host = cpal::default_host();
        let sample_rate = host
            .default_output_device()
            .and_then(|d| d.default_output_config().ok())
            .map(|c| c.sample_rate() as f32)
            .unwrap_or(24000.0);

        // CPAL 백그라운드 하드웨어 다이렉트 출력을 구동하기 위한 전용 시스템 스레드를 즉각 스폰(생성)합니다.
        thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_output_device() {
                Some(d) => d,
                None => {
                    warn!(
                        "오디오 출력 하드웨어 장치를 찾을 수 없습니다. 웹 대체(Web Fallback) 모드로 안전하게 전환합니다."
                    );
                    return;
                }
            };

            let config = match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        "기본 물리 오디오 출력 설정을 가져오는 데 실패했습니다: {}",
                        e
                    );
                    return;
                }
            };

            let sample_format = config.sample_format();
            let stream_config: cpal::StreamConfig = config.into();
            let target_sample_rate = stream_config.sample_rate as f32;

            let is_playing = Arc::new(AtomicBool::new(false));

            let stream_corrupted = Arc::new(AtomicBool::new(false));
            let stream_corrupted_err = stream_corrupted.clone();

            let err_fn = move |err| {
                error!(
                    "오디오 하드웨어 장치 출력 스트림에서 예기치 않은 치명적인 오류가 발생했습니다: {}",
                    err
                );
                stream_corrupted_err.store(true, Ordering::Release);
            };

            // 스트림 생성 함수 (각 스트림마다 전용 mpsc 채널 쌍 생성)
            let create_stream = |dev: &cpal::Device, cfg: &cpal::StreamConfig, fmt: cpal::SampleFormat, is_playing_ref: &Arc<AtomicBool>| -> Option<(cpal::Stream, mpsc::Sender<StreamCommand>)> {
                let channels = cfg.channels as usize;
                let target_sample_rate = cfg.sample_rate as f32;
                let err_fn_inner = err_fn.clone();
                let (st_tx, st_rx) = mpsc::channel::<StreamCommand>();

                let stream_res = match fmt {
                    cpal::SampleFormat::F32 => {
                        let mut state = AudioState {
                            tracks: Vec::new(),
                            volume: 1.0,
                            is_playing: is_playing_ref.clone(),
                            ducking_multiplier: 1.0,
                            sample_rate: target_sample_rate,
                        };
                        dev.build_output_stream(
                            cfg.clone(),
                            move |data: &mut [f32], _| {
                                write_data_f32(data, channels, &mut state, &st_rx)
                            },
                            err_fn_inner,
                            None,
                        ).ok()
                    }
                    cpal::SampleFormat::I16 => {
                        let mut state = AudioState {
                            tracks: Vec::new(),
                            volume: 1.0,
                            is_playing: is_playing_ref.clone(),
                            ducking_multiplier: 1.0,
                            sample_rate: target_sample_rate,
                        };
                        dev.build_output_stream(
                            cfg.clone(),
                            move |data: &mut [i16], _| {
                                write_data_i16(data, channels, &mut state, &st_rx)
                            },
                            err_fn_inner,
                            None,
                        ).ok()
                    }
                    cpal::SampleFormat::U16 => {
                        let mut state = AudioState {
                            tracks: Vec::new(),
                            volume: 1.0,
                            is_playing: is_playing_ref.clone(),
                            ducking_multiplier: 1.0,
                            sample_rate: target_sample_rate,
                        };
                        dev.build_output_stream(
                            cfg.clone(),
                            move |data: &mut [u16], _| {
                                write_data_u16(data, channels, &mut state, &st_rx)
                            },
                            err_fn_inner,
                            None,
                        ).ok()
                    }
                    _ => None,
                };

                stream_res.map(|s| (s, st_tx))
            };

            let mut stream_handle = create_stream(&device, &stream_config, sample_format, &is_playing);

            if let Some((ref s, _)) = stream_handle {
                if let Err(e) = s.play() {
                    error!("오디오 출력 하드웨어 스트림을 재생하는 데 실패했습니다: {}", e);
                } else {
                    info!("오디오 재생기 백그라운드 구동 스레드가 정상 시작되었습니다 (CPAL 및 사운드 카드 드라이버 연동 완료).");
                }
            }

            // 외부 전송 채널 수신 이벤트를 처리하는 무한 대기 루프입니다.
            for cmd in rx {
                // 스트림 손상 여부 점검 및 자동 재구축 (Rebuild)
                if stream_corrupted.swap(false, Ordering::Acquire) {
                    warn!("오디오 출력 스트림 손상이 감지되었습니다. 사운드 장치 스트림을 재구축(Rebuild)합니다...");
                    let host = cpal::default_host();
                    if let Some(new_dev) = host.default_output_device() {
                        if let Ok(new_cfg) = new_dev.default_output_config() {
                            let new_fmt = new_cfg.sample_format();
                            let new_stream_cfg: cpal::StreamConfig = new_cfg.into();
                            if let Some((new_s, new_st_tx)) = create_stream(&new_dev, &new_stream_cfg, new_fmt, &is_playing) {
                                if new_s.play().is_ok() {
                                    stream_handle = Some((new_s, new_st_tx));
                                    info!("오디오 출력 스트림이 성공적으로 재구축되었습니다.");
                                }
                            }
                        }
                    }
                }

                if let Some((_, ref active_st_tx)) = stream_handle {
                    match cmd {
                        AudioCommand::PlayRaw {
                            samples,
                            sample_rate,
                            category,
                            volume,
                            clear_previous,
                        } => {
                            let step = sample_rate / target_sample_rate;
                            is_playing.store(true, Ordering::Release);
                            let _ = active_st_tx.send(StreamCommand::Play {
                                samples,
                                step,
                                category,
                                volume,
                                clear_previous,
                            });
                            info!(
                                "가공이 완료된 F32 오디오(Raw) 연주를 전개합니다 (주파수 보정: {}Hz -> 타겟 기기: {}Hz)",
                                sample_rate, target_sample_rate
                            );
                        }
                        AudioCommand::Clear(category) => {
                            let _ = active_st_tx.send(StreamCommand::Clear(category));
                        }
                        AudioCommand::SetVolume(volume) => {
                            let _ = active_st_tx.send(StreamCommand::SetVolume(volume));
                            info!(
                                "오디오 재생기 마스터 최종 혼합 음량이 변경되었습니다: {:.2}",
                                volume
                            );
                        }
                    }
                }
            }
        });

        Self {
            tx,
            sample_rate,
            decode_semaphore: Arc::new(Semaphore::new(2)),
            // active_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            normal_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            important_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// [비동기식] 백그라운드에서 재생을 준비하는 JoinHandle 비동기 작업들을 적재 관리 리스트에 신규 가입시킵니다.
    /// 최대 병렬 동작 개수를 충족하여 초과하면, 가장 앞선 선행 작업 핸들을 강제로 abort(중단)시켜 리소스 파괴 현상을 능동 방지합니다.
    pub async fn register_task(
        &self,
        category: PlaybackCategory,
        handle: tokio::task::JoinHandle<()>,
    ) {
        let tasks_mutex = match category {
            PlaybackCategory::TtsImportant => &self.important_tasks,
            _ => &self.normal_tasks,
        };
        let mut tasks = tasks_mutex.lock().await;
        // 이미 종료를 정상 완료한 핸들들은 사전에 가려내어 리스트에서 누락시킵니다.
        tasks.retain(|t| !t.is_finished());
        tasks.push(handle);

        while tasks.len() > 8 {
            let old_task = tasks.remove(0);
            old_task.abort();
            warn!(
                "시스템 리소스 과점유를 방지하기 위해, 만료 시점이 지나치게 지연된 오래된 디코딩/재생 백그라운드 태스크가 한계 개수 초과로 강제 중단(Abort) 처리되었습니다."
            );
        }
    }

    // TtsNormal (일반 음성) 태스크만 선택적으로 강제 중단
    pub async fn clear_normal_tasks(&self) {
        let mut tasks = self.normal_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        info!(
            "가동되고 있던 TtsNormal 오디오 백그라운드 비동기 태스크들의 일괄 강제 중단 및 초기화를 마쳤습니다."
        );
    }

    /// [추가] 현재 백그라운드에서 실행 대기 중이거나 음성 합성이 활발히 진행 중인 일반 음성(TtsNormal) 태스크가 존재하는지 안전하게 확인합니다.
    pub async fn has_active_normal_tasks(&self) -> bool {
        let tasks = self.normal_tasks.lock().await;
        // 목록이 비어있지 않고, 완료(finished)되지 않은 채 동작 중인 태스크가 하나라도 존재하면 참(true)을 반환합니다.
        !tasks.is_empty() && tasks.iter().any(|t| !t.is_finished())
    }

    /// 보관하고 관리 중이던 활성 재생 백그라운드 연산 작업들을 모조리 정지(Abort)시키고 깨끗이 방출합니다.
    pub async fn clear_active_tasks(&self) {
        // let mut tasks = self.active_tasks.lock().await;
        let mut tasks = self.normal_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        let mut tasks = self.important_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        info!(
            "가동되고 있던 모든 오디오 백그라운드 비동기 태스크들의 일괄 강제 중단 및 초기화를 마쳤습니다."
        );
    }

    /// 디코딩 백그라운드 스레드 풀 가동 및 재생 조율 처리를 한 군데로 병합하여 제어하는 공통 도우미(Helper) 함수입니다.
    /// CPU 핫스팟 현상을 해소하기 위해 세마포어 동시성 통제 옵션(`use_semaphore`)을 고르게 전달할 수 있습니다.
    fn decode_and_play(
        &self,
        audio_data: Vec<u8>,
        category: PlaybackCategory,
        volume: f32,
        clear_previous: bool,
        use_semaphore: bool,
    ) -> tokio::task::JoinHandle<()> {
        let self_clone = self.clone();
        let sem = self.decode_semaphore.clone();

        tokio::spawn(async move {
            // 이전 출력을 정리하도록 요청받은 경우, 백그라운드에서 실행 중인 비동기 오디오 태스크들을 즉시 종료(Abort)시킵니다.
            if clear_previous {
                self_clone.clear_active_tasks().await;
            }

            if use_semaphore {
                // 세마포어 잠금을 획득하기 위해 안전하게 대기 순번을 타며 진입을 개시합니다.
                let _permit = match sem.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        error!(
                            "디코딩 동시성 제어용 세마포어 원자 잠금을 획득하는 데 최종 실패했습니다."
                        );
                        return;
                    }
                };

                let _ = tokio::task::spawn_blocking(move || {
                    self_clone.decode_and_play_sync(audio_data, category, volume, clear_previous);
                })
                .await;
            } else {
                // 세마포어를 쓰지 않고 즉시 독립 블로킹 영역으로 제어권을 할당합니다.
                let _ = tokio::task::spawn_blocking(move || {
                    self_clone.decode_and_play_sync(audio_data, category, volume, clear_previous);
                })
                .await;
            }
        })
    }

    /// 동기식 방식을 전제하여 바이너리를 실수 PCM으로 전수 복원해 하드웨어 기기로 방출합니다.
    fn decode_and_play_sync(
        &self,
        audio_data: Vec<u8>,
        category: PlaybackCategory,
        volume: f32,
        clear_previous: bool,
    ) {
        match crate::audio::decode_to_pcm(audio_data, self.sample_rate, 1.0) {
            Ok(new_samples) => {
                if let Err(e) = self.play_raw(
                    new_samples,
                    self.sample_rate,
                    category,
                    volume,
                    clear_previous,
                ) {
                    error!(
                        "메모리 디코딩이 완료된 오디오 샘플 벡터를 실시간 연주 대기열로 전달하지 못했습니다: {:?}",
                        e
                    );
                }
            }
            Err(e) => {
                error!(
                    "사운드 바이너리 디코딩 및 고밀도 오디오 프레임 추출 작업에 최종 실패했습니다: {:?}",
                    e
                );
            }
        }
    }

    /// 임의의 외부 사운드 리소스 버퍼를 백그라운드 연산 스레드풀에서 즉각 분리 전개하고 결합하여 재생 스트림에 투입합니다.
    pub fn play(
        &self,
        audio_data: Vec<u8>,
        category: PlaybackCategory,
        volume: f32,
        clear_previous: bool,
    ) -> Result<()> {
        // 공통 도우미 로직을 가동합니다 (이 연산은 신속한 음향 연출을 위해 세마포어 대기 제한을 걸지 않습니다).
        let _handle = self.decode_and_play(audio_data, category, volume, clear_previous, false);
        Ok(())
    }

    /// 정렬이 완료된 임의의 F32 실수형 PCM 어레이 데이터를 백그라운드 사운드 렌더링 루프로 직접 배출합니다.
    pub fn play_raw(
        &self,
        samples: Vec<f32>,
        sample_rate: f32,
        category: PlaybackCategory,
        volume: f32,
        clear_previous: bool,
    ) -> Result<()> {
        // 이전 트랙을 청소하도록 지시받은 경우,
        // 현재 동작 중인 다른 모든 백그라운드 오디오 태스크(디코딩/합성 등)를 즉시 중단(Abort)시킵니다.
        if clear_previous {
            let self_clone = self.clone();
            tokio::spawn(async move {
                self_clone.clear_active_tasks().await;
            });
        }

        self.tx
            .send(AudioCommand::PlayRaw {
                samples,
                sample_rate,
                category,
                volume,
                clear_previous,
            })
            .map_err(|_| Error::AudioDeviceNotFound(vec![]))
    }

    /// 특정 카테고리에 속한 하드웨어 재생 장비 연주 대기 큐들을 깨끗하게 밀어내어 제거합니다.
    pub fn clear(&self, category: PlaybackCategory) -> Result<()> {
        self.tx
            .send(AudioCommand::Clear(category))
            .map_err(|_| Error::AudioDeviceNotFound(vec![]))
    }

    /// 전반적인 시스템의 마스터 혼합 출력 음량 볼륨 폭을 조절합니다
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.tx
            .send(AudioCommand::SetVolume(volume))
            .map_err(|_| Error::AudioDeviceNotFound(vec![]))
    }
}
