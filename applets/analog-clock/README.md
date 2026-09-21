# `analog-clock` Applet

이 `analog-clock`은 `multiline-braille-display` 프로젝트를 위해 개발된 WASM(WebAssembly) 애플릿입니다. 점자 디스플레이에 아날로그 시계를 렌더링하여 현재 시간을 촉각적으로 보여줍니다.

## 주요 기능

- **아날로그 시계 디스플레이**: 시침, 분침, 초침을 사용하여 현재 시간을 아날로그 방식으로 표시합니다.
- **`sdk` 활용**: 디스플레이 렌더링을 위해 `sdk` 크레이트에서 제공하는 인터페이스를 사용합니다.

## 실행 방법

이 애플릿을 빌드하고 실행하려면, 프로젝트의 루트 디렉토리에서 `runtime` 크레이트를 사용합니다.

1.  **빌드**: `analog-clock` 애플릿을 WASM 모듈로 빌드합니다.
    ```bash
    just build-applets
    # 또는 특정 애플릿만 빌드:
    cargo build --package analog_clock --target wasm32-unknown-unknown --release
    ```
2.  **실행**: `runtime` 크레이트를 사용하여 빌드된 `analog-clock` 애플릿을 실행합니다.
    ```bash
    cargo run --package runtime analog_clock
    # 또는 Justfile 레시피를 통해 실행:
    just run analog_clock
    ```
    `--display braille` 옵션을 추가하여 실제 점자 디스플레이(또는 해당 에뮬레이션)를 사용할 수도 있습니다:
    ```bash
    cargo run --package runtime analog_clock -- --display braille
    # 또는 Justfile 레시피를 통해 실행:
    just run-braille analog_clock
    ```

## 기술 상세

`analog-clock` 애플릿은 Rust로 작성되었으며 WebAssembly (WASM) 타겟으로 컴파일됩니다. `multiline-braille-display`의 `sdk`를 사용하여 호스트 런타임 환경의 디스플레이 기능과 상호작용합니다. 이를 통해 실제 하드웨어 디스플레이나 콘솔 에뮬레이션 디스플레이에 시계를 그릴 수 있습니다.
