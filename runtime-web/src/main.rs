mod app;
mod applet_manager;
mod audio;
mod components;
mod display;
mod error;
mod http;
mod keypad;
mod log;
mod preferences;
mod runner;
mod screens;
mod storage;
mod time;
pub mod ws_comm;

pub use error::{Error, Result};

use app::App;
use leptos::logging;

fn main() {
    console_error_panic_hook::set_once();
    logging::log!("csr mode -mounting to body");
    leptos::mount::mount_to_body(App)
}
