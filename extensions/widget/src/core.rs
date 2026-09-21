use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface};
use sdk::error::Result;

/// 위젯의 상태 업데이트 결과를 나타냅니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetUpdateResult {
    /// 위젯의 내부 상태가 변경되어 화면을 다시 그려야 함을 나타냅니다.
    NeedsRedraw,
    /// 상태 변경이 없어 다시 그릴 필요가 없음을 나타냅니다.
    Unchanged,
}

/// 애플릿 내에서 재사용 가능한 UI 컴포넌트(위젯)가 공통으로 구현해야 하는 트레이트입니다.
pub trait Widget {
    /// 위젯이 화면에 그려지는 영역을 반환합니다.
    fn bounds(&self) -> BoundsRect;

    /// 위젯의 렌더링 영역을 새롭게 설정합니다.
    fn set_bounds(&mut self, bounds: BoundsRect);

    /// 이 위젯이 사용자 입력(포커스)을 받을 수 있는지 여부를 반환합니다.
    /// (예: 단순 텍스트 라벨은 false, 텍스트 영역이나 버튼은 true)
    fn can_focus(&self) -> bool {
        false
    }

    /// 위젯이 포커스를 얻었을 때 호출됩니다. (이때 TTS 안내 등을 수행하기 좋습니다.)
    fn on_focus(&mut self, _context: &mut Context) -> Result<()> {
        Ok(())
    }

    /// 위젯이 포커스를 잃었을 때 호출됩니다. (커서 깜빡임 중지 등에 활용 가능합니다.)
    fn on_blur(&mut self, _context: &mut Context) -> Result<()> {
        Ok(())
    }

    /// 매 프레임 또는 이벤트 발생 시 위젯의 내부 상태를 업데이트합니다.
    ///
    /// * `context`: 시스템 API 및 상태에 접근하기 위한 컨텍스트
    /// * `is_focused`: 현재 앱 전체에서 이 위젯이 포커스를 가지고 있는지 여부.
    ///   위젯은 이 값이 `true`일 때만 키패드 이벤트를 팝(pop)하여 처리해야 합니다.
    fn on_update(&mut self, context: &mut Context, is_focused: bool) -> Result<WidgetUpdateResult>;

    /// 위젯을 할당된 `bounds` 내에 그립니다.
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()>;
}
