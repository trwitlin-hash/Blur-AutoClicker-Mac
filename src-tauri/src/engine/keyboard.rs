#[cfg(target_os = "windows")]
#[path = "keyboard_win.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "keyboard_mac.rs"]
mod platform;

pub use platform::{is_alphabetic_vk, send_key_down, send_key_presses, send_key_up};
