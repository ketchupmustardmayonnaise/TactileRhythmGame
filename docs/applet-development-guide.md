# 애플릿(Applet) 개발 가이드

이 문서는 다중 라인 점자 디스플레이 프로젝트에서 WASM(WebAssembly) 애플릿을 처음부터 작성하고, 빌드 및 테스트하는 전 과정을 안내합니다.

---

## 1. 개요 및 개념

애플릿은 런타임 환경(`runtime-native` 또는 `runtime-web`) 위에서 독립적인 WASM 샌드박스로 동작하는 미니 애플리케이션입니다.
`sdk` 크레이트에서 제공하는 `Applet` 트레이트를 구현함으로써 디스플레이 렌더링, 키패드 입력 처리, 오디오/TTS 재생, 시간 조회 등 하드웨어 기능을 쉽게 이용할 수 있습니다.

---

## 2. 애플릿 개발 단계

### Step 1: 새 크레이트 프로젝트 생성

`applets` 디렉토리 아래에 새로운 Rust 라이브러리 프로젝트를 생성합니다:

```sh
cargo new applets/my-applet --lib
```

---

### Step 2: `Cargo.toml` 설정

생성된 `applets/my-applet/Cargo.toml` 파일을 열어 `crate-type = ["cdylib"]`과 `sdk` 의존성을 추가합니다:

```toml
[package]
name = "my-applet"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"] # WASM 모듈 출력을 위한 필수 설정

[dependencies]
sdk = { path = "../../sdk" }

# 필요 시 확장 라이브러리 추가
graphics = { path = "../../extensions/graphics" }
widget = { path = "../../extensions/widget" }
```

---

### Step 3: `Applet.toml` 메타데이터 작성

애플릿 디렉토리(`applets/my-applet/`)에 `Applet.toml` 메타데이터 파일 생성:

```sh
just -- # 또는 메타데이터 툴 실행:
cargo run -p applet-metadata-tool -- applets/my-applet --init
```

생성된 `Applet.toml` 예시:

```toml
category = "Utility"
priority = "Normal"
hidden = false

# 11행 x 11열 규격의 점자/픽셀 아이콘
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
ko = "내 첫 애플릿"
en = "My First Applet"
ja = "私のアプリ"

[description]
ko = "다중 라인 점자 디스플레이용 테스트 애플릿"
en = "My first test applet for braille display"
ja = "テストアプリの説明"
```

---

### Step 4: 애플릿 소스 코드 작성 (`src/lib.rs`)

`applets/my-applet/src/lib.rs` 파일에 `Applet` 트레이트를 구현합니다:

```rust
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::error::Result;
use sdk::event::UpdateResult;
use sdk::Applet;

#[derive(Default)]
pub struct MyApplet {
    counter: u32,
}

impl Applet for MyApplet {
    /// 키패드 및 타이머 등 이벤트 업데이트
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;

        // 키패드 입력 이벤트 소진
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Up => {
                        self.counter += 1;
                        needs_redraw = true;
                    }
                    KeyCode::Down => {
                        self.counter = self.counter.saturating_sub(1);
                        needs_redraw = true;
                    }
                    KeyCode::Function => {
                        // TTS 음성 안내 실행
                        context.audio.speak_text(&format!("현재 카운트: {}", self.counter));
                    }
                    _ => {}
                }
            }
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    /// 디스플레이 프레임버퍼 렌더링
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        // 텍스트/도형 렌더링 예시
        let size = canvas.get_size();
        // (graphics 확장 등을 활용하거나 기본 픽셀/선 그리기 수행)

        Ok(())
    }
}

/// WASM 런타임 진입점 함수 정의
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::run(Box::new(MyApplet::default()));
}
```

---

### Step 5: `Justfile` 빌드 목록에 추가

루트 디렉토리의 `Justfile` 상단 `applets` 변수에 새로 만든 애플릿 패키지 이름을 추가합니다:

```makefile
applets := "analog-clock drawing-game graph my-applet"
```

---

## 3. 빌드 및 테스트

### 단일 애플릿 빌드 및 메타데이터 임베딩

```sh
just build-applet my-applet
```

### 전체 애플릿 빌드

```sh
just build-applets
```

### 네이티브 런타임에서 애플릿 테스트

```sh
just run-applet my-applet
```

### 웹 런타임 시뮬레이터에서 테스트

```sh
just run-web
```

브라우저에서 `http://127.0.0.1:8080`으로 접속하여 애플릿 실행 및 시뮬레이션을 확인합니다.

---

## 4. 유용한 팁 및 확장 기능

- **그래픽/도형 표현:** `extensions/graphics` 패키지를 추가하여 원, 선, 사각형, 폰트 렌더링을 활용할 수 있습니다.
- **UI 위젯 구축:** `extensions/widget` 및 `extensions/views` 패키지를 활용하면 버튼, 메뉴 리스트, 스크롤 뷰 등을 손쉽게 제작할 수 있습니다.
- **다국어 지원:** `extensions/i18n` 모듈 및 `Applet.toml` [name]/[description] 다국어 설정을 활용하세요.
