# 웹 런타임 시뮬레이터 (`runtime-web`)

웹 브라우저 상에서 다중 라인 점자 디스플레이 디바이스의 동작 및 WASM 애플릿을 시뮬레이션할 수 있는 웹 기반 런타임 환경입니다. Leptos(CSR) 및 Trunk 번들러를 기반으로 구축되었습니다.

## 주요 기능 및 용도

- **웹 기반 시뮬레이터:** 별도의 물리 하드웨어 없이 브라우저에서 점자 디스플레이 화면 출력, 키패드 입력, 오디오 출력 시뮬레이션
- **WASM 애플릿 로딩 & 실행:** WASM으로 빌드된 애플릿들을 동적으로 로드하고 샌드박스 환경에서 구동
- **시각적 UI 시뮬레이션:** 점자 디스플레이의 각 핀/셀 상태를 웹 인터페이스로 시각화
- **웹소켓 통신 연결 (`ws_comm`):** 외부 디바이스나 도구와의 통신 지원
- **오디오 시뮬레이션 (Web Audio API & Speech Synthesis):** 웹 브라우저 오디오 및 음성 합성 API 활용

## 실행 및 사용법

### 프리렉퀴직 (사전 요구사항)

Trunk 번들러가 필요합니다. (`Justfile` 명령 실행 시 자동으로 검사 및 설치됩니다)

### 1. 개발 서버 실행 (Serving)

루트 디렉토리에서 `just` 명령어를 사용하여 실행하는 것을 권장합니다 (`applets` 자동 빌드 포함):

```sh
just serve-runtime-web
# 또는
just run-web
```

`runtime-web` 디렉토리에서 직접 Trunk 명령어로 실행할 수도 있습니다:

```sh
trunk serve
```

실행 후 기본 주소인 `http://127.0.0.1:8080`으로 브라우저에 접속할 수 있습니다.

### 2. 릴리스 빌드 (Release Build)

웹 static 배포용 번들을 빌드합니다.

```sh
# 루트 디렉토리에서
just build-runtime-web

# 또는 runtime-web 디렉토리에서
trunk build --release
```

빌드 결과물은 `runtime-web/dist` 디렉토리에 정적 파일 형태로 생성됩니다.

## 디렉토리 구조

```
runtime-web/
├── public/applets/    # 웹 런타임에 로드할 .wasm 애플릿 및 applets.json
├── src/
│   ├── applet_manager/ # 애플릿 로딩 및 관리
│   ├── components/     # 점자 디스플레이 및 키패드 UI 컴포넌트
│   ├── display/        # 점자 디스플레이 렌더링 로직
│   ├── audio/          # Web Audio & TTS 오디오 처리
│   ├── keypad/         # 키보드/키패드 입력 이벤팅
│   ├── screens/        # 화면 페이지 구성
│   ├── ws_comm/        # 웹소켓 통신 모듈
│   └── main.rs         # Leptos 앱 엔트리포인트
├── index.html          # Trunk 엔트리 HTML (Tailwind 및 public 셋업)
├── Trunk.toml          # Trunk 빌드 설정
└── Cargo.toml          # Rust 크레이트 의존성 설정
```
