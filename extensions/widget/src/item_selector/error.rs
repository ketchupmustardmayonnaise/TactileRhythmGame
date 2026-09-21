/// 아이템 셀렉터 컴포넌트 내부에서 발생하는 계산 및 구조적 에러를 나타냅니다.
#[derive(thiserror::Error, Debug)]
pub enum SelectorError {
    /// 디스플레이 해상도 공간이 아이템들의 크기와 여백을 담기에 턱없이 부족하여 자동 계산 로직이 실패했을 때 발생합니다.
    #[error(
        "레이아웃 자동 계산 실패: 화면 공간이 부족합니다. (가로 부족분: {shortage_width}px, 세로 부족분: {shortage_height}px)"
    )]
    InsufficientSpace {
        /// 부족한 픽셀 너비 (가로 방향)
        shortage_width: i16,
        /// 부족한 픽셀 높이 (세로 방향)
        shortage_height: i16,
    },
    /// 제공된 아이템의 크기(w, h)가 0 이하로 설정되어 렌더링이 불가능할 때 발생합니다.
    #[error("레이아웃 자동 계산 실패: 아이템 상자의 크기(w, h)가 0 이하로 잘못 설정되었습니다.")]
    InvalidItemSize,
    /// 표시할 아이템 목록이 완전히 비어있을 때 발생합니다.
    #[error("레이아웃 렌더링 실패: 표시할 아이템 리스트가 비어있습니다.")]
    EmptyItems,
}
