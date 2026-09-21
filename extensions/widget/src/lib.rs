// extensions/widget/src/lib.rs

pub mod core;
pub mod container;
pub mod item_selector;

pub use core::{Widget, WidgetUpdateResult};
pub use container::{Container, Spacing};
