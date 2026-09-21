# ESP32-S3 펌웨어 (`firmware`)

다중 라인 점자 디스플레이의 하드웨어를 직접 제어하는 ESP32-S3 기반 베어메탈/Embassy 펌웨어입니다.

`esp-hal`, `esp-rtos`(Embassy 비동기 런타임)를 활용하여 점자 디스플레이 모듈(SPI/GPIO 핀 제어 및 PWM 시분할 제어), 키패드 매트릭스 입력, 전원(CM 모듈 및 12V 전원) 제어, 부팅/종료 애니메이션, UART 통신을 비동기로 처리합니다.

## 주요 기능 및 구성요소

- **점자 디스플레이 제어 (`braille_display.rs`):** SPI 및 핀 제어를 통한 프레임버퍼 기반 시분할 PWM 밝기/높이 제어 (더블 버퍼링 지원)
- **키패드 입력 모니터링 (`keypad.rs`):** 버튼 입력 감지 및 점자 입력(Perkins 모드 / Normal 모드) 지원
- **전원 관리 (`power.rs` / `system_loop`):** 컴퓨트 모듈(CM5) 및 12V 전원 라인(PW12) 활성화/비활성화, 전원 버튼 길게 누름(3초) 감지 시 리눅스 호스트 종료 요청
- **부팅 및 종료 애니메이션 (`boot_animation.rs`, `shutdown_animation.rs`):** 리눅스 부팅 및 시스템 종료 시 시각적/촉각적 애니메이션 출력
- **UART 통신 (`uart_comm.rs`):** COBS (Consistent Overhead Byte Stuffing) 및 `postcard` 직렬화를 통한 호스트 런타임 간 메시지 주고받기

## 개발 환경 구축

펌웨어를 빌드하고 플래싱하려면 ESP32-S3 타겟 Rust 툴체인 및 `espup`, `cargo-espflash` 도구가 필요합니다.

### 1. 툴체인 설치

```sh
# espup 설치 (이미 설치되어 있지 않은 경우)
cargo install espup
cargo install cargo-espflash

# ESP Rust 툴체인 설치
espup install
```

### 2. 환경 변수 로드

플래싱 전 ESP 툴체인 환경 변수를 적용합니다:

```sh
source ~/export-esp.sh
```

## 빌드, 플래싱 및 실행 방법

### 1. `just` 레시피 활용 (권장)

프로젝트 루트 디렉토리의 `Justfile`을 사용하여 펌웨어를 빌드 및 실행할 수 있습니다.

- **펌웨어 실행 및 플래싱 (Release):**
  ```sh
  just run-firmware
  ```
- **펌웨어 빌드 전용:**
  ```sh
  just build-firmware
  ```

### 2. Cargo 및 `cargo-espflash` 직접 활용

`firmware` 디렉토리로 이동한 후 직접 명령어를 실행할 수 있습니다.

- **펌웨어 빌드:**

  ```sh
  cd firmware
  cargo build --release
  ```

- **펌웨어 플래싱 및 모니터링 (Serial Monitor):**

  ```sh
  cd firmware
  cargo espflash flash --release --monitor
  ```

  특정 시리얼 포트를 지정하려는 경우:

  ```sh
  cargo espflash flash --release --monitor --port /dev/ttyUSB0
  ```

## 모듈 구조

```
firmware/
├── src/
│   ├── bin/
│   │   └── main.rs             # 비동기 메인 엔트리포인트 및 Embassy 태스크 루프
│   ├── board_config.rs         # 핀 맵, 디스플레이 규격 및 보드 설정 상수
│   ├── boot_animation.rs       # 부팅 애니메이션 효과
│   ├── braille_display.rs      # 점자 디스플레이 driver 및 PWM 구현
│   ├── keypad.rs               # 키패드 매트릭스 제어 및 입력 파싱
│   ├── power.rs                # 전원 PIN (CM_EN, PW12_EN) 제어
│   ├── shutdown_animation.rs   # 종료 애니메이션 효과
│   ├── uart_comm.rs            # UART 통신 드라이버
│   └── error.rs                # 펌웨어 오류 정의
├── Cargo.toml                  # esp-hal 및 dependencies 설정
└── build.rs                    # 링커 스크립트 및 ESP32-S3 빌드 스크립트
```
