# Applet Metadata Tool (`applet-metadata-tool`)

애플릿 디렉토리의 `Cargo.toml` 및 `Applet.toml` 파일로부터 메타데이터(이름, 설명, 아이콘, 카테고리, 우선순위 등)를 읽고 검증하거나, `wasm-tools`를 이용하여 컴파일된 WASM 파일에 커스텀 메타데이터 섹션을 임베딩하는 CLI 유틸리티입니다.

## 주요 기능 및 용도

- **템플릿 생성 (`--init`):** 지정한 애플릿 디렉토리에 기본 `Applet.toml` 템플릿 파일 생성
- **메타데이터 검증:**
  - `Applet.toml` 데이터 파싱
  - **11x11 규격 픽셀 아이콘 검증 및 정규화:** 아이콘이 정확히 11개 행과 행당 11개의 허용 문자(`0`, `1`, `.`, `#`, `█` 등)로 구성되었는지 엄격하게 검사
- **정보 출력 (`--name`, `--version`, `--json`):** `Cargo.toml` / `Applet.toml`에서 이름, 버전, 또는 JSON 메타데이터 출력
- **WASM 메타데이터 임베딩 (`--embed ... -o ...`):** `wasm-tools` CLI를 실행하여 컴파일된 WASM 파일 내부에 메타데이터 섹션 추가

## 사용법 (Usage)

### 기본 실행 구문

```sh
cargo run -p applet-metadata-tool -- <applet-dir-path> [OPTIONS]
```

### 1. `Applet.toml` 템플릿 생성 (`--init`)

새로운 애플릿 디렉토리에 기본 `Applet.toml` 설정 파일을 생성합니다.

```sh
cargo run -p applet-metadata-tool -- applets/my-new-applet --init
```

### 2. 패키지 이름/버전/JSON 메타데이터 출력

```sh
# Cargo.toml 패키지 이름 출력
cargo run -p applet-metadata-tool -- applets/analog-clock --name

# Cargo.toml 패키지 버전 출력
cargo run -p applet-metadata-tool -- applets/analog-clock --version

# 파싱 및 정규화된 Applet.toml 메타데이터를 JSON 문자열로 출력
cargo run -p applet-metadata-tool -- applets/analog-clock --json
```

### 3. WASM 파일에 메타데이터 임베딩 (`--embed`)

WASM 바이너리에 메타데이터를 임베딩하여 새로운 WASM 파일로 출력합니다 (`wasm-tools` 필요).

```sh
cargo run -p applet-metadata-tool -- applets/analog-clock \
  --embed target/wasm32-unknown-unknown/release/analog_clock.wasm \
  -o dist/applets/analog-clock.wasm
```

> **참고:** 이 도구는 `Justfile`의 `build-applet` 레시피 내부에서 메타데이터 자동 임베딩 및 빌드 파이프라인의 일부로 활용됩니다.

## `Applet.toml` 작성 규격 예시

```toml
category = "Utility"
priority = "Normal"
hidden = false

# 아이콘은 정확히 11행 x 11열 규격이어야 합니다.
# 켜짐: '1', '#', '█', '●', '*', 'x', 'o'
# 꺼짐: '0', '.', ' ', '░'
icon = """
...#####...
.##.....##.
#.........#
#.........#
#.........#
#.........#
#.........#
#.........#
#.........#
.##.....##.
...#####...
"""

[name]
ko = "아날로그 시계"
en = "Analog Clock"
ja = "아날로그 시계"

[description]
ko = "다중 라인 점자 디스플레이용 아날로그 시계 애플릿"
en = "Analog clock applet for multiline braille display"
ja = "아날로그 시계 설명"
```
