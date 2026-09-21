// extensions/component/src/item_selector/mod.rs

pub mod bar;
pub mod config;
pub mod error;
pub mod grid;

// 하위 구조체들을 외부 크레이트에서도 사용하기 쉽도록 재노출(Re-export)합니다.
pub use bar::{Horizontal, ItemBar, Vertical};
pub use config::{LayoutStrategy, SelectorBehavior, SelectorOrientation, SelectorStyle};
pub use grid::{Grid, ItemGrid};
use crate::core::{Widget, WidgetUpdateResult};
use sdk::api::context::Context;
use sdk::api::display::{BoundsRect, DisplayInterface};

/// 1차원 Bar와 2차원 Grid 컴포넌트를 편리하게 단일 개념으로 사용하기 위한 통합 래퍼 열거형입니다.
pub enum ItemSelector<'a> {
    /// 1차원 선형 리스트 컴포넌트
    Bar(ItemBar<'a>),
    /// 2차원 격자 바둑판 컴포넌트
    Grid(ItemGrid),
}

impl<'a> ItemSelector<'a> {
    /// 1차원 목록 컴포넌트 생성을 위한 간결한 헬퍼 빌더입니다.
    pub fn new_bar(
        items: Vec<&'a str>,
        orientation: Box<dyn SelectorOrientation>,
        behavior: SelectorBehavior,
    ) -> Self {
        ItemSelector::Bar(ItemBar::new(items, orientation, behavior))
    }

    /// 2차원 격자 목록 컴포넌트 생성을 위한 간결한 헬퍼 빌더입니다.
    pub fn new_grid(
        rows: usize,
        cols: usize,
        total_items: usize,
        behavior: SelectorBehavior,
    ) -> Self {
        ItemSelector::Grid(ItemGrid::new(rows, cols, total_items, behavior))
    }
}

impl<'a> Widget for ItemSelector<'a> {
    fn bounds(&self) -> BoundsRect {
        match self {
            ItemSelector::Bar(bar) => bar.bounds(),
            ItemSelector::Grid(grid) => grid.bounds(),
        }
    }

    fn set_bounds(&mut self, bounds: BoundsRect) {
        match self {
            ItemSelector::Bar(bar) => bar.set_bounds(bounds),
            ItemSelector::Grid(grid) => grid.set_bounds(bounds),
        }
    }

    fn can_focus(&self) -> bool {
        match self {
            ItemSelector::Bar(bar) => bar.can_focus(),
            ItemSelector::Grid(grid) => grid.can_focus(),
        }
    }

    fn on_update(&mut self, context: &mut Context, is_focused: bool) -> sdk::error::Result<WidgetUpdateResult> {
        match self {
            ItemSelector::Bar(bar) => bar.on_update(context, is_focused),
            ItemSelector::Grid(grid) => grid.on_update(context, is_focused),
        }
    }

    fn on_draw(&self, display: &mut dyn DisplayInterface) -> sdk::error::Result<()> {
        match self {
            ItemSelector::Bar(bar) => bar.on_draw(display),
            ItemSelector::Grid(grid) => grid.on_draw(display),
        }
    }
}
