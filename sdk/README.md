# `sdk` Crate (Software Development Kit)

이 `sdk` 크레이트는 `multiline-braille-display` 프로젝트의 WASM(WebAssembly) 애플릿 개발을 위한 소프트웨어 개발 키트(SDK)를 제공합니다. WASM 애플릿이 호스트 런타임 환경(`runtime-native`, `runtime-web`)과 통신 및 상호작용하는 데 필요한 공통 인터페이스, 진입점 매크로, 데이터 타입을 제공합니다.

## 주요 기능 및 목적

- **표준화된 API 제공**: 디스플레이 출력, 키패드/마우스 입력, 오디오/TTS 재생, 시간 관리, 환경 설정, HTTP 요청, 애플릿 관리 등 호스트 기능을 활용할 수 있는 API 모듈 세트 제공
- **애플릿 메인 진입점 및 이벤트 처리**: `Applet` 트레이트 및 `export_applet!` / `run` 매크로/함수를 통해 생명주기 및 이벤트 처리 추상화
- **타입 안전성 및 바인딩 추상화**: WASM 경계를 넘나드는 데이터 직렬화 및 FFI/호스트 바인딩 세부사항을 숨기고 안전한 Rust API 제공

## 모듈 구조

SDK는 다음과 같은 주요 모듈로 구성됩니다:

- **`api/` (호스트 API 모듈):**
  - `display`: 디스플레이 출력 및 그래픽/텍스트 렌더링 API
  - `keypad` / `mouse`: 키패드 버튼 입력 및 마우스 조작 이벤트
  - `audio`: 음성(TTS) 합성 및 오디오 효과음 재생 API
  - `time`: 현재 시간, 타임존 및 타이머 API
  - `http`: 네트워크 HTTP 요청 API
  - `preferences` / `settings`: 애플릿 환경 설정 및 영속화 스토리지 API
  - `applet_manager` / `window`: 애플릿 간 전환 및 창 관리 API
  - `log`: 호스트 런타임 로그 기록 API
- **`applet.rs` / `macros.rs`:** `Applet` 트레이트 정의 및 애플릿 구동용 매크로/생명주기 실행 함수
- **`event/`:** 디스플레이, 키패드, 타임, 센서 등 애플릿에 전달되는 이벤트 정의
- **`types.rs` / `error.rs`:** 공통 결과/오류 타입 및 도메인 데이터 구조체

## 사용법 (WASM 애플릿 개발자용)

1. 애플릿 프로젝트의 `Cargo.toml`에 `sdk`를 의존성으로 추가합니다:

   ```toml
   [dependencies]
   sdk = { path = "../../sdk" }
   ```

2. `Applet` 트레이트를 구현하고 매크로로 진입점을 선언합니다:

   ```rust
   use sdk::applet::{Applet, AppletInfo, Context};
   use sdk::event::Event;
   use sdk::Result;

   pub struct MyApplet;

   impl Applet for MyApplet {
       fn init(ctx: &mut Context) -> Result<Self> {
           Ok(Self)
       }

       fn update(&mut self, ctx: &mut Context, event: Event) -> Result<()> {
           match event {
               Event::Keypad(key_event) => {
                   // 키 입력 처리
               }
               _ => {}
           }
           Ok(())
       }
   }

   // 애플릿 진입점 생성
   sdk::export_applet!(MyApplet);
   ```
