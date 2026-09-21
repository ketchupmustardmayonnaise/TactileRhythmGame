# 확장 모듈 (`extensions`)

`extensions` 디렉토리는 WASM 애플릿 및 런타임 개발 시 공통적으로 유용하게 사용할 수 있는 기능별 고수준 확장 크레이트(Extension Crates) 모음입니다.

기본 SDK(`sdk`) 위에 레이어링되어 있으며, 고수준 UI 위젯, 점자 변환, 그래픽 렌더링, 다국어 처리, 물리 연산 등의 기능을 독립된 모듈 형태로 제공합니다.

---

## 확장 모듈 목록 및 용도

### 1. `braille` ([`extensions/braille`](./braille))

- **용도:** 점자(Braille) 텍스트 및 코드 변환/처리 확장 라이브러리
- **주요 기능:** `braillify` 등을 활용하여 텍스트를 점자 유니코드 및 디스플레이 출력 포맷으로 변환하거나 점자 입출력 관련 편의 위젯 제공

### 2. `graphics` ([`extensions/graphics`](./graphics))

- **용도:** 점자 디스플레이용 그래픽 및 폰트 렌더링 확장 모듈
- **주요 기능:** `embedded-graphics` 및 `rusttype` 기반 폰트/도형 렌더링, 픽셀 다중 높이(그레이스케일/높이) 처리 및 도형 그리기 기능

### 3. `i18n` ([`extensions/i18n`](./i18n))

- **용도:** 다국어 지원 (Internationalization) 확장 라이브러리
- **주요 기능:** 애플릿 및 UI 내 다국어(한국어, 영어, 일본어 등) 텍스트 리소스 관리 및 언어 설정별 문자열 변환 기능

### 4. `physics` ([`extensions/physics`](./physics))

- **용도:** 2D 샌드박스 및 인터랙티브 게임/시뮬레이션을 위한 물리 연산 엔진
- **주요 기능:** 속도, 가속도, 충돌 감지, 경계 반사, 바운딩 박스 연산 등 2D 물리학 계산 보조

### 5. `views` ([`extensions/views`](./views))

- **용도:** 뷰(View) 렌더링 및 뷰 레이아웃 계층 모듈
- **주요 기능:** 화면 단위의 뷰 컨테이너 관리, 렌더링 루프 및 이벤트 전달 레이어 제공

### 6. `widget` ([`extensions/widget`](./widget))

- **용도:** UI 위젯 및 레이아웃 컴포넌트 세트
- **주요 기능:** 버튼, 리스트, 다이얼로그, 스크롤 뷰, 텍스트 박스 등 애플릿 개발용 reusable UI 위젯 컴포넌트 제공

---

## 사용 방법

애플릿의 `Cargo.toml` 파일에 필요한 확장 모듈만 의존성으로 추가하여 사용할 수 있습니다.

예시 (`Cargo.toml`):

```toml
[dependencies]
sdk = { path = "../../sdk" }
widget = { path = "../../extensions/widget" }
graphics = { path = "../../extensions/graphics" }
i18n = { path = "../../extensions/i18n" }
```
