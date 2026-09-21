# PC Capture App (`capture`)

PC의 화면 영역을 실시간으로 캡처하여 다중 라인 점자 디스플레이 디바이스(또는 런타임)로 전송하고 시각화/촉각화할 수 있도록 도와주는 GUI 데스크톱 애플리케이션입니다.

## 주요 기능 및 용도

- **화면 영역 캡처 (Screen Capture):** PC 화면의 특정 영역을 선택하여 실시간으로 프레임을 캡처합니다.
- **다양한 연결 방식 지원 (Multi-Connection):**
  - **Serial (USB):** USB CDC 시리얼 포트 연결 지원 (자동 포트 감지)
  - **Network (TCP):** 소켓 네트워크 연결 지원 (기본 포트: `3006`)
  - **WebSocket:** 웹소켓 연결 지원 (기본 포트: `3007`)
- **이미지 필터 및 전처리:**
  - 대비(Contrast) 및 이분화 임계값(Threshold) 조절
  - 윤곽선 추출 (Outline) 및 고대비 (High Contrast) 모드
  - 색상 반전 (Invert) 기능
  - 세로 모드 (Portrait Mode) 및 감도 범위 조정
- **설정 자동 저장:** 연결 정보, 필터 모드, 캡처 영역 좌표 등의 설정이 자동으로 저장되고 재시행 시 복원됩니다.

## 실행 및 사용법

### 1. 명령어 실행

프로젝트 루트 디렉토리에서 `just` 명령어로 실행할 수 있습니다:

```sh
just run-capture
```

또는 Cargo 명령어로 직접 실행할 수 있습니다:

```sh
cargo run --package capture --release
```

### 2. 주요 CLI 옵션

필요에 따라 명령줄 인자로 시리얼 포트, 통신 속도, 픽셀 당 비트 수를 지정할 수 있습니다.

```sh
cargo run --package capture -- --port COM3 --baud-rate 460800 --bits-per-pixel 4
```

- `-p, --port <PORT>`: 연결할 시리얼 포트 이름 (예: `COM3`, `/dev/ttyUSB0`)
- `-b, --baud-rate <BAUD>`: 시리얼 통신 속도 (기본값: `460800`)
- `--bits-per-pixel <BITS>`: 픽셀당 비트 수 (기본값: `4`)

### 3. GUI 사용 가이드

1. **연결 설정:**
   - 상단 컨트롤 패널에서 연결 방식(Serial / Network / WebSocket)을 선택합니다.
   - 대상 주소 또는 포트를 지정한 후 `Connect` 버튼을 클릭합니다.
2. **캡처 영역 지정:**
   - 캡처 영역 지정 도구를 통해 PC 화면에서 점자 디스플레이로 전송할 영역을 드래그하여 선택합니다.
3. **이미지 처리 옵션:**
   - 필터 모드(HighContrast, Outline 등)와 Threshold 슬라이더를 조절하여 점자 디스플레이 표현에 최적화된 이미지를 설정합니다.
