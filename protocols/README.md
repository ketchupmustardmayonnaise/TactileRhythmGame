# `protocols` Crate

임베디드 하드웨어(ESP32), 런타임, PC 캡처 도구 등 통신 주체 간에 사용되는 공통 데이터 구조 및 통신 메시지 프로토콜을 정의하는 `#![no_std]` 지원 Rust 크레이트입니다.

## 주요 특징

- **`#![no_std]` 지원:** 임베디드 펌웨어(ESP32) 및 WASM, 호스트 런타임 등 모든 타겟에서 메모리 효율적으로 사용할 수 있도록 `no_std` (`alloc` 사용) 기반으로 구현되어 있습니다.
- **`serde` 바이너리/JSON 직렬화 지원:** 메시지 직렬화 및 역직렬화를 위한 `Serialize`, `Deserialize` 트레이트를 도밍 타입에 제공합니다 (`Postcard` 바이너리 직렬화 규격 등과 호환).

## 구성 모듈 및 프로토콜 정의

### 1. `esp32` 모듈 ([`esp32.rs`](./src/esp32.rs))

호스트 런타임과 ESP32 임베디드 메인 보드 간 통신 프로토콜을 정의합니다.

- **`RequestToEsp32` (런타임 ➡️ ESP32):**
  - `Clear`: 화면 초기화
  - `UpdateDisplay { width, height, data }`: 4-bit 모드로 패킹된 전체 프레임 데이터 전송
  - `UpdateDisplayByDiff { data }`: 차분(Diff) 프레임 데이터 전송
  - `SetKeypadMode(KeypadMode)`: 일반(Normal) / 점자 입력(Perkins) 모드 설정
  - `BootCompleted` / `ShutdownStarted`: 부팅 및 종료 상태 전달
- **`ResponseFromEsp32` (ESP32 ➡️ 런타임):**
  - `Ok` / `Error`: 응답 상태
  - `BrailleChord(u8)`: 점자 입력 코드 조합
  - `KeypadButtonEvents(Vec<KeypadButtonEvent>)`: 키패드 버튼 눌림/떼어짐 이벤트 목록
  - `RequestShutdown`: 전원 버튼 등에 의한 종료 요청
- **주요 관련 구조체:** `KeypadButton`, `KeypadButtonType`, `KeypadStatus`, `KeypadMode` 등

### 2. `capture` 모듈 ([`capture.rs`](./src/capture.rs))

PC 화면 캡처 앱(`capture`) 및 외부 제어기와 호스트 런타임 간의 통신 프로토콜을 정의합니다.

- **`CaptureRequestToRuntime` (캡처 앱 ➡️ 런타임):**
  - `LaunchApplet { name }`: 특정 애플릿 실행 요청
  - `UpdateDisplay { width, height, data, bits_per_pixel }`: 화면 이미지 데이터 전송
  - Wi-Fi 스캔 및 설정: `ScanWifi`, `ConnectWifi`, `DisconnectWifi`, `CancelWifiOperation`, `GetNetworkAddresses`
- **`CaptureResponseFromRuntime` (런타임 ➡️ 캡처 앱):**
  - `Ok`, `Error`, `Paused`
  - `WifiScanResult`, `WifiConnectResult`, `WifiDisconnectResult`, `NetworkAddressesResult`
