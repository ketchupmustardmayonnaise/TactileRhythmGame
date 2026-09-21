pub trait Style {
    /// 테두리 두께를 반환합니다. (테두리가 없는 스타일은 None)
    fn border_thickness(&self) -> Option<i16>;

    /// 색상 반전 여부를 반환합니다.
    fn is_inverted(&self) -> bool;

    /// 테두리 두께 보정 유틸리티 (기본 최소 두께 1)
    fn normalized_border(&self) -> i16 {
        self.border_thickness().unwrap_or(0).max(1)
    }
}
/// 아이템 셀렉터에 적용할 시각적/촉각적 강조(하이라이트) 스타일을 정의하는 구조체입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectorStyle {
    /// 테두리선 두께 (None 이면 테두리를 두르지 않습니다.)
    pub border_thickness: Option<i16>,
    /// 내부 영역의 모든 픽셀 색상 강도(Intensity) 반전 여부
    pub invert: bool,
}

impl Style for SelectorStyle {
    fn border_thickness(&self) -> Option<i16> {
        self.border_thickness
    }

    fn is_inverted(&self) -> bool {
        self.invert
    }
}

impl SelectorStyle {
    /// 테두리만 있는 스타일을 생성합니다.
    pub fn border(thickness: i16) -> Self {
        Self {
            border_thickness: Some(thickness),
            invert: false,
        }
    }

    /// 색상 반전만 있는 스타일을 생성합니다.
    pub fn invert() -> Self {
        Self {
            border_thickness: None,
            invert: true,
        }
    }

    /// 테두리와 색상 반전을 동시에 적용하는 스타일을 생성합니다.
    pub fn border_and_invert(thickness: i16) -> Self {
        Self {
            border_thickness: Some(thickness),
            invert: true,
        }
    }
}

/// 레이아웃을 배치하고 계산하는 세부 전략입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStrategy {
    /// 1. 아이템 상자의 가로/세로 크기는 고정하고, 화면 크기에 맞춰 '간격(spacing)'을 균등 분할하여 조절합니다.
    FixedItemSize {
        /// 각 아이템의 고정 가로 크기
        width: i16,
        /// 각 아이템의 고정 세로 크기
        height: i16,
    },
    /// 2. 아이템들 사이의 여백(spacing)은 고정하고, 화면 영역을 등분하여 '아이템 상자 크기'를 자동으로 키우거나 줄입니다.
    FixedSpacing {
        /// 고정 여백(간격) 픽셀 크기
        spacing: i16,
    },
    /// 3. 자동 계산 연산을 전혀 적용하지 않고, 사용자가 입력한 고정 크기 및 간격을 그대로 적용합니다.
    Manual {
        /// 고정 가로 크기
        width: i16,
        /// 고정 세로 크기
        height: i16,
        /// 고정 간격 크기
        spacing: i16,
    },
}

impl Default for LayoutStrategy {
    fn default() -> Self {
        LayoutStrategy::Manual {
            width: 10,
            height: 10,
            spacing: 2,
        }
    }
}

/// 아이템 셀렉터의 개별 동작 사양 및 스타일을 통합 설정하는 구조체입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectorBehavior {
    /// 스크롤 캔버스 정보 (Some((width, height)) 이면 스크롤이 활성화되며 지정된 전체 가상 캔버스 크기를 사용하고, None 이면 스크롤을 끕니다.)
    pub scroll_canvas_size: Option<(i16, i16)>,
    /// 레이아웃 연산 전략
    pub layout_strategy: LayoutStrategy,
    /// 선택 효과 스타일 (None 이면 방향키 선택 및 하이라이트 효과가 완전히 꺼집니다.)
    pub selection_style: Option<SelectorStyle>,
}

/// 아이템 셀렉터의 레이아웃 배치 및 탐색 방향성입니다.
/// 멀티스레드 환경(Send + Sync)에서 안전하게 전송 및 공유될 수 있도록 마커 트레잇을 상속합니다.
pub trait SelectorOrientation: std::fmt::Debug + Send + Sync {
    /// 가로 방향 배치 여부
    fn is_horizontal(&self) -> bool {
        false
    }
    /// 세로 방향 배치 여부
    fn is_vertical(&self) -> bool {
        false
    }
    /// 2차원 격자 배치 여부
    fn is_grid(&self) -> bool {
        false
    }
}
