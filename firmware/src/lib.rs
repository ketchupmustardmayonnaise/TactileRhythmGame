#![no_std]
pub mod board_config;
pub mod boot_animation;
pub mod braille_display;
pub mod error;
pub mod keypad;
pub mod power;
pub mod shutdown_animation;
pub mod uart_comm;

pub use error::{Error, Result};
