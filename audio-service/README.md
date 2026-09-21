# 음성 서비스 (`audio-service`)

Piper TTS 엔진 및 오디오 재생 처리기 기반 백엔드 음성/효과음 HTTP 백엔드 서비스입니다. 호스트 런타임 및 애플릿에서 전달된 텍스트 음성 합성(TTS) 요청 및 효과음/시퀀스 오디오 재생 요청을 비동기로 처리합니다.

## 주요 기능 및 용도

- **텍스트 음성 합성 (TTS):** ONNX 런타임 기반 `piper-rs` 엔진을 사용하여 고품질 텍스트 음성 합성 지원
- **스마트 청킹 (Text Chunking) & 스트리밍 재생:** 긴 문장을 텍스트 청크 단위로 나누어 지연 시간을 최소화하고 실시간 스트리밍 형태로 오디오 출력
- **시퀀스 및 효과음 오디오 재생:** 텍스트 세그먼트, PCM/사운드 세그먼트, 묵음(Silence) 세그먼트를 조합한 시퀀스 재생 처리
- **카테고리별 오디오 믹싱 & 제어:** 중요 TTS(`TtsImportant`), 일반 TTS(`TtsNormal`), 효과음(`SoundEffect`) 카테고리별 트랙 관리 및 우선순위/볼륨 조정

## HTTP API 엔드포인트

기본 포트: `3005` (환경 변수 또는 설정으로 변경 가능)

- **`POST /audio_segments`**: 텍스트, 사운드, 묵음 조합 시퀀스 음성 합성 및 스트리밍 재생
- **`POST /sound`**: 원시 오디오 바이너리(MP3, WAV 등) 효과음 즉시 재생
- **`POST /volume`**: 전체 시스템 음성/효과음 볼륨 변경
- **`POST /settings`**: 음성(Voice 모델) 및 언어 설정 변경
- **`GET /`**: 서비스 상태 확인 헬스체크

## 실행 및 사용법

### 1. 자산(Assets) 및 espeak-ng-data 준비

1. **Piper 음성 모델:** Piper TTS 음성 모델 파일(`.onnx` 및 `.onnx.json`)이 `audio-service/assets` 디렉토리(또는 지정된 asset 경로)에 존재해야 합니다.
2. **eSpeak NG 설치 (필수):**
   - **Windows:** [espeak-ng GitHub Releases](https://github.com/espeak-ng/espeak-ng/releases)에서 `espeak-ng-X.X.X-x64.msi`를 설치하거나, 설치된 `espeak-ng-data` 폴더를 `audio-service/assets/espeak-ng-data` 경로로 복사합니다.
   - **Linux (Ubuntu/Debian):** `sudo apt-get install espeak-ng-data` 명령어로 설치합니다.

### 2. 서비스 실행 (개발 모드)

루트 디렉토리에서 `just` 레시피로 실행 (`cargo watch` 자동 감지 재시작):

```sh
just serve-audio-service
```

또는 Cargo 명령어로 직접 실행:

```sh
TTS_ASSET_DIR=./assets RUST_LOG=info,audio_service=debug,ort=warn cargo run --package audio-service
```

### 3. 릴리스 빌드 및 배포

- **네이티브 호스트 빌드:**
  ```sh
  just build-audio-service
  ```
- **ARM64 (Raspberry Pi/CM5) 크로스 빌드:**
  ```sh
  just build-audio-service-aarch64
  ```
- **디바이스 오디오 서비스 디버깅:**
  ```sh
  just debug-audio
  ```

## 디렉토리 구조

```
audio-service/
├── assets/          # Piper TTS 모델 (.onnx, .onnx.json) 및 음성 자산
├── src/
│   ├── audio/       # 오디오 디코딩, cpal/rodio 디바이스 믹서 및 재생 관리자
│   ├── chunk.rs     # 실시간 음성 합성을 위한 텍스트 청크 분할기
│   ├── config.rs    # 서비스 포트 및 자산 경로 등 환경설정
│   ├── error.rs     # 서비스 오류 및 예외 처리
│   ├── tts.rs       # Piper-rs TTS 엔진 연동 및 비동기 TtsActor
│   ├── lib.rs       # 모듈 선언
│   └── main.rs      # Axum HTTP 서버 엔트리포인트 및 API 라우터
├── Cargo.toml       # axum, piper-rs, ort, rodio 등의 의존성
└── LICENSE          # GNU General Public License v3.0 (GPLv3)
```

## 라이선스 (License)

이 서비스(`audio-service`)는 **[GNU General Public License v3.0 (GPLv3)](./LICENSE)** 하에 라이선스가 부여됩니다.

- **Piper TTS 및 하위 라이브러리 연동:** `audio-service`는 GPLv3 기반의 `piper-rs` 및 의존 라이브러리를 직접 정적/동적 링크하여 사용하므로, GPLv3 라이선스가 적용됩니다.
- **다른 컴포넌트와의 격리:** `runtime-native`, `runtime-web`, `applets` 등 프로젝트의 다른 컴포넌트들은 `audio-service`와 직접 링크되지 않고 **독립된 프로세스 간 HTTP REST API 통신**을 통해 작동합니다. 따라서 GPL 라이선스 전염성이 다른 런타임 및 애플릿 컴포넌트에는 미치지 않습니다.
