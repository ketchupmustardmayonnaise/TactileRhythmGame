# 다중 라인 점자 디스플레이 프로젝트 (Multiline Braille Display)

이 프로젝트는 WASM(WebAssembly)을 사용하여 다중 라인 점자 디스플레이에서 실행되는 애플리케이션(이하 애플릿)을 구동하기 위한 실험적인 펌웨어 및 런타임 환경입니다.

## 아키텍처 및 구성요소

이 시스템은 다음과 같은 핵심 구성요소로 이루어져 있습니다.

- **런타임 (Runtime):** WASM으로 컴파일된 애플릿을 로드하고 실행하는 실행 환경입니다.
  - `runtime-common`: 네이티브 및 웹 런타임 간 공유되는 공통 로직 및 인터페이스
  - [`runtime-native`](./runtime-native/README.md): Linux 호스트 또는 디바이스(예: Raspberry Pi / CM5) 상에서 동작하는 네이티브 런타임
  - [`runtime-web`](./runtime-web/README.md): 웹 브라우저 기반의 시뮬레이터 런타임 (Trunk 활용)

- **애플릿 (Applets):** `applets` 디렉토리에 위치한 개별 WASM 애플리케이션입니다 (예: `analog-clock`, `tetris`, `drawing-game` 등). 각 애플릿은 독립적인 Rust 라이브러리 크레이트(`crate-type = ["cdylib"]`)로 개발되며, WASM으로 컴파일되어 런타임에 의해 동적으로 로드됩니다.

- **SDK & Extensions:** [`sdk`](./sdk/README.md) 디렉토리에 위치한 애플릿 개발용 SDK와, 각종 기능 확장 모듈([`extensions/`](./extensions/README.md) - `braille`, `graphics`, `i18n`, `physics`, `views`, `widget`)을 포함합니다.

- **음성 서비스 (Audio Service):** [`audio-service`](./audio-service/README.md) 디렉토리에 위치하며, Piper TTS 기반으로 음성 출력을 담당하는 백엔드 서비스입니다.

- **펌웨어 (Firmware):** [`firmware`](./firmware/README.md) 디렉토리에 위치하며, ESP32 등 임베디드 하드웨어 제어를 담당하는 펌웨어 코드입니다.

- **PC 앱 및 도구 (PC Apps & Tools):**
  - [`pc-apps/capture`](./pc-apps/capture/README.md): 점자 디스플레이 화면 캡처 및 도구
  - [`tools/applet-metadata-tool`](./tools/applet-metadata-tool/README.md): 애플릿 메타데이터 임베딩 및 빌드 도구
  - `host-utils`, [`protocols`](./protocols/README.md): 호스트 유틸리티 및 통신 프로토콜 정의

## 시작하기 (Prerequisites)

이 프로젝트를 빌드하고 실행하려면 Rust 도구 모음과 `just` 명령어 실행기가 필요합니다.

### 1. Rust 설치 (rustup)

Rust 공식 설치 관리자인 `rustup`을 통해 Rust 환경을 구성합니다.

- **Linux / macOS (Unix 계열):**
  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Windows:**
  [https://rustup.rs/](https://rustup.rs/)에서 `rustup-init.exe`를 다운로드하여 실행합니다.

### 2. WASM 타겟 추가

WASM 애플릿을 빌드하기 위해 `wasm32-unknown-unknown` 타겟을 추가해야 합니다.

```sh
rustup target add wasm32-unknown-unknown
```

### 3. Just 설치

명령어 실행기 `just`를 설치합니다.

```sh
cargo install just
```

### 4. 크로스 컴파일 환경 설정 (Linux/CM5 target)

Raspberry Pi / CM5 (`aarch64-unknown-linux-gnu`) 등 타겟 디바이스용 크로스 컴파일을 위해 **Zig 컴파일러**와 **`cargo-zigbuild`** 또는 **`cross`** 도구가 필요합니다.

#### Zig 컴파일러 및 `cargo-zigbuild` (네이티브 런타임 크로스 빌드용)

- **Zig 설치:**
  - **macOS:** `brew install zig`
  - **Linux:** `sudo apt install zig` 또는 [ziglang.org](https://ziglang.org/download/)에서 다운로드
  - **Windows:** `winget install zig.zig` 또는 `choco install zig`
- **cargo-zigbuild 설치:**
  ```sh
  cargo install cargo-zigbuild
  ```

#### `cross` (Docker 기반 크로스 컴파일 - 음성 서비스 등)

- **Docker / Podman 설치:** 호스트 시스템에 Docker Desktop 또는 Docker Engine이 설치되어 실행 중이어야 합니다.
- **cross 설치:**
  ```sh
  cargo install cross --git https://github.com/cross-rs/cross
  ```

## 디렉토리 구조

```
/
├── applets/          # WASM으로 컴파일되는 개별 애플릿 프로젝트들
├── audio-service/    # [README](./audio-service/README.md) Piper TTS 기반 음성 서비스
├── extensions/       # [README](./extensions/README.md) SDK 및 런타임용 확장 모듈
├── firmware/         # [README](./firmware/README.md) ESP32 메인 펌웨어
├── host-utils/       # 호스트 유틸리티 라이브러리
├── pc-apps/          # PC용 애플리케이션 (capture [README](./pc-apps/capture/README.md))
├── protocols/        # [README](./protocols/README.md) 통신 및 데이터 프로토콜
├── runtime-common/   # 런타임 공통 모듈
├── runtime-native/   # [README](./runtime-native/README.md) 네이티브 런타임 (Linux/CM5)
├── runtime-web/      # [README](./runtime-web/README.md) 웹 시뮬레이터 런타임
├── sdk/              # [README](./sdk/README.md) 애플릿 개발용 SDK
├── tools/            # 메타데이터 도구 [README](./tools/applet-metadata-tool/README.md) 등 빌드 툴
├── Justfile          # 프로젝트 빌드, 실행, 배포용 스크립트
└── Cargo.toml        # Rust 워크스페이스 설정
```

## 주요 명령어

이 프로젝트는 `just` 명령어 실행기를 사용합니다. `just` 또는 `just --list` 명령어로 사용 가능한 모든 레시피를 확인할 수 있습니다.

### 코드 품질 및 검사

- **코드 포맷팅:**

  ```sh
  just fmt
  ```

  Rust 코드(`cargo fmt`, `leptosfmt`)와 Markdown 파일(`prettier`)을 포맷팅합니다.

- **린트 검사:**

  ```sh
  just lint
  # 또는
  just clippy
  ```

  전체 워크스페이스에 대해 Clippy 린트를 실행합니다.

- **전체 검사:**

  ```sh
  just check
  ```

  포맷팅, 린트, `cargo check`를 수행하여 코드 무결성을 확인합니다.

- **문서 생성:**

  ```sh
  just doc
  ```

  모든 크레이트의 Rustdoc 문서를 생성하고 브라우저에서 엽니다.

### 개발 및 실행 (Development)

- **애플릿 자동 재빌드:**

  ```sh
  just watch-applets
  ```

  `applets` 디렉토리의 변경 사항을 감지하여 WASM 애플릿을 자동으로 재빌드합니다.

- **네이티브 런타임 실행:**

  ```sh
  just run-applet [ARGS]
  ```

  네이티브 런타임을 디버그 모드로 빌드하고 실행합니다. (예: `just run-applet --help`)

- **네이티브 런타임 개발 감지:**

  ```sh
  just watch-runtime-native
  ```

- **웹 런타임 (시뮬레이터) 실행:**

  ```sh
  just run-web
  # 또는
  just serve-runtime-web
  ```

  웹 브라우저 기반 런타임 시뮬레이터를 실행합니다 (`http://127.0.0.1:8080`).

- **음성(TTS) 서비스 실행:**

  ```sh
  just serve-audio-service
  ```

  Piper TTS 음성 서비스를 실행합니다.

- **화면 캡처 앱 실행:**

  ```sh
  just run-capture
  ```

### 배포 및 타겟 빌드 (Release & Deploy)

- **전체 빌드:**

  ```sh
  just build
  ```

  애플릿, 네이티브 런타임, 웹 런타임, 음성 서비스를 모두 릴리스 모드로 빌드합니다.

- **개별 빌드:**

  ```sh
  just build-applets               # 모든 애플릿 (WASM) 빌드 및 metadata 임베딩
  just build-applet <applet-name>  # 특정 애플릿 단일 빌드
  just build-runtime-native        # 네이티브 런타임 빌드
  just build-runtime-native-aarch64 # ARM64 (CM5 등) 네이티브 런타임 크로스 빌드
  just build-runtime-web           # 웹 런타임 빌드
  just build-audio-service         # 음성 서비스 빌드
  ```

- **디바이스 배포 (Raspberry Pi / CM5):**

  ```sh
  just deploy          # 애플릿, 네이티브 런타임, 음성 서비스 전체 배포
  just deploy-applets  # 애플릿만 배포
  just deploy-services # systemd 서비스 파일 배포
  ```

## 개발자 문서 및 API 레퍼런스

- [**애플릿 개발 가이드 (Applet Development Guide)**](./docs/applet-development-guide.md): 애플릿 프로젝트 생성, `Applet.toml` 메타데이터 작성, `sdk` API 활용 및 빌드/테스트 가이드
- **API 레퍼런스 (Rustdoc):** 전체 워크스페이스 크레이트(`sdk`, `protocols`, `extensions`, `runtime` 등)의 API 레퍼런스 문서를 생성하고 웹 브라우저에서 보려면 아래 명령어를 실행하세요:
  ```sh
  just doc
  # 또는
  cargo doc --workspace --open
  ```

## 새로운 애플릿 만들기

자세한 설명과 예제 코드는 [**애플릿 개발 가이드 (docs/applet-development-guide.md)**](./docs/applet-development-guide.md) 문서를 참고하세요.

1. **새 프로젝트 생성:**
   `applets` 디렉토리 아래에 새로운 Rust 라이브러리 프로젝트를 생성합니다.

   ```sh
   cargo new applets/my-new-applet --lib
   ```

2. **`Cargo.toml` 설정:**
   생성된 `applets/my-new-applet/Cargo.toml` 파일에 패키지 설정 및 SDK 의존성을 추가합니다.

   ```toml
   [package]
   name = "my-new-applet"
   version = "0.1.0"
   edition = "2024"

   [lib]
   crate-type = ["cdylib"] # WASM 모듈 출력을 위해 필수

   [dependencies]
   sdk = { path = "../../sdk" }
   ```

3. **애플릿 코드 작성:**
   `applets/my-new-applet/src/lib.rs` 파일에 애플릿 로직을 작성합니다. `sdk`를 통해 디스플레이 출력, 키패드 입력, 사운드 및 센서 이벤트 등을 제어할 수 있습니다.

4. **`Justfile` 애플릿 목록에 추가:**
   루트 디렉토리의 `Justfile` 상단 `applets` 변수에 새로 작성한 애플릿 이름을 추가합니다.

   ```makefile
   applets := "... my-new-applet"
   ```

5. **빌드 및 실행:**
   `just build-applet my-new-applet` 또는 `just build-applets`를 실행하면 `.wasm` 파일이 생성되고 메타데이터가 임베딩되며, `just run-applet` 또는 `just run-web`으로 테스트할 수 있습니다.
