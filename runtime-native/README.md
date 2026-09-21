# 네이티브 런타임 (`runtime-native`)

Linux 호스트 PC 또는 임베디드 디바이스(Raspberry Pi, CM5 등)에서 WASMtime 엔진을 활용하여 WASM 애플릿을 로드하고 실행하는 네이티브 호스트 런타임 환경입니다.

## 주요 기능 및 용도

- **WASMtime 기반 WASM 샌드박스 실행:** WASM으로 빌드된 애플릿 모듈을 로드하고 런타임 Host API와 바인딩하여 안전하게 실행
- **디바이스 및 호스트 제어 (HAL):** 디스플레이 출력, 키패드 입력, 음성/오디오 처리
- **시리얼/네트워크 통신:** USB CDC 시리얼 통신, UART, TCP 네트워크 소켓 통신을 통한 제어 및 이미지/데이터 수신
- **크로스 컴파일 지원:** x86_64 Linux 및 ARM64 (`aarch64-unknown-linux-gnu`) 크로스 빌드 지원

## 실행 및 사용법

### 1. 네이티브 런타임 실행 (디버그 모드)

루트 디렉토리에서 `just` 명령어를 사용하여 실행할 수 있습니다:

```sh
just run-applet [ARGS]
```

예시 (특정 애플릿 바로 실행 또는 도움말 확인):

```sh
# 도움말 확인
just run-applet --help

# launcher 애플릿 실행
just run-applet launcher

# 디바이스 모드로 실행
just run-applet --device
```

또는 Cargo로 직접 실행:

```sh
cargo run --package runtime-native -- [ARGS]
```

### 2. 주요 CLI 옵션 (`cli.rs`)

- `<APP_NAME>`: 실행할 WASM 애플릿 이름 (지정하지 않으면 설정된 기본 런처 애플릿 실행)
- `--width <WIDTH>`: 디스플레이 가로 크기 (기본값: `48`)
- `--height <HEIGHT>`: 디스플레이 세로 크기 (기본값: `32`)
- `--lang <ko|en|ja>`: 언어 설정 (기본값: `ko`)
- `--device`: 실제 디바이스 모드로 실행
- `--bits-per-pixel <BITS>`: 픽셀당 비트 수 (기본값: `4`)
- `--benchmark`: 초당 프레임 수(FPS) 측정 및 출력
- `--print-applet-dir`: 애플릿이 저장되는 기본 디렉토리 경로 출력 후 종료

### 3. 빌드 및 배포 레시피 (Justfile)

- **네이티브 런타임 디버그 빌드:**
  ```sh
  just build-runtime-native-debug
  ```
- **네이티브 런타임 릴리스 빌드:**
  ```sh
  just build-runtime-native
  ```
- **aarch64 (Raspberry Pi/CM5) 크로스 컴파일:**
  ```sh
  just build-runtime-native-aarch64
  ```
- **디바이스에 배포 및 디버깅:**
  ```sh
  just debug-runtime
  ```

## 디렉토리 구조

```
runtime-native/
├── src/
│   ├── audio/        # 오디오 및 TTS 클라이언트 처리
│   ├── hal/          # 하드웨어 추상화 레이어 (Display, Keypad, Framebuffer 등)
│   ├── host/         # SDK Host API 구현체 및 WASMtime 바인딩
│   ├── cli.rs        # Command Line Interface 인자 파싱
│   ├── config.rs     # 런타임 설정 및 경로 관리
│   ├── net_comm.rs   # 네트워크 통신 모듈
│   ├── runner.rs     # WASMtime 인스턴스 실행 및 이벤트 루프
│   ├── usb_serial.rs # USB 시리얼 통신
│   └── main.rs       # 런타임 엔트리포인트
└── Cargo.toml        # Rust 크레이트 의존성 설정
```
