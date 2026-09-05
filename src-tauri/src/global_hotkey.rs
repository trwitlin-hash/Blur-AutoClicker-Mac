//! Global hotkey registration through the window server.
//!
//! The Windows original detected hotkeys with a low-level keyboard hook, and the
//! first macOS port translated that literally into a `CGEventTap`. That was the
//! wrong primitive: a tap reads the entire input stream, so macOS gates it behind
//! Input Monitoring / Accessibility, and without that grant the hotkey only works
//! while the app is frontmost.
//!
//! `RegisterEventHotKey` — which `tauri-plugin-global-shortcut` uses on macOS —
//! asks the window server to deliver one specific combination. It needs no
//! permission at all and fires no matter which app is in front.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::hotkeys::HotkeyBinding;

/// Set once a combination is registered with the window server, so the
/// event-tap polling loop stands down and the toggle cannot fire twice.
pub static ACTIVE: AtomicBool = AtomicBool::new(false);

fn token_to_code(token: &str) -> Option<Code> {
    use Code::*;
    Some(match token {
        "a" => KeyA,
        "b" => KeyB,
        "c" => KeyC,
        "d" => KeyD,
        "e" => KeyE,
        "f" => KeyF,
        "g" => KeyG,
        "h" => KeyH,
        "i" => KeyI,
        "j" => KeyJ,
        "k" => KeyK,
        "l" => KeyL,
        "m" => KeyM,
        "n" => KeyN,
        "o" => KeyO,
        "p" => KeyP,
        "q" => KeyQ,
        "r" => KeyR,
        "s" => KeyS,
        "t" => KeyT,
        "u" => KeyU,
        "v" => KeyV,
        "w" => KeyW,
        "x" => KeyX,
        "y" => KeyY,
        "z" => KeyZ,
        "0" => Digit0,
        "1" => Digit1,
        "2" => Digit2,
        "3" => Digit3,
        "4" => Digit4,
        "5" => Digit5,
        "6" => Digit6,
        "7" => Digit7,
        "8" => Digit8,
        "9" => Digit9,
        "f1" => F1,
        "f2" => F2,
        "f3" => F3,
        "f4" => F4,
        "f5" => F5,
        "f6" => F6,
        "f7" => F7,
        "f8" => F8,
        "f9" => F9,
        "f10" => F10,
        "f11" => F11,
        "f12" => F12,
        "f13" => F13,
        "f14" => F14,
        "f15" => F15,
        "f16" => F16,
        "f17" => F17,
        "f18" => F18,
        "f19" => F19,
        "f20" => F20,
        "space" => Space,
        "enter" => Enter,
        "tab" => Tab,
        "escape" | "esc" => Escape,
        "backspace" => Backspace,
        "delete" => Delete,
        "home" => Home,
        "end" => End,
        "pageup" => PageUp,
        "pagedown" => PageDown,
        "up" => ArrowUp,
        "down" => ArrowDown,
        "left" => ArrowLeft,
        "right" => ArrowRight,
        "numpad0" => Numpad0,
        "numpad1" => Numpad1,
        "numpad2" => Numpad2,
        "numpad3" => Numpad3,
        "numpad4" => Numpad4,
        "numpad5" => Numpad5,
        "numpad6" => Numpad6,
        "numpad7" => Numpad7,
        "numpad8" => Numpad8,
        "numpad9" => Numpad9,
        _ => return None,
    })
}

/// Translate one of our bindings into a window-server shortcut.
///
/// Returns `None` for anything the window server cannot express: mouse buttons,
/// modifier-only bindings, and chords of more than one main key. Those keep
/// using the polling path.
pub fn binding_to_shortcut(binding: &HotkeyBinding) -> Option<Shortcut> {
    if binding.key_tokens.len() != 1 {
        return None;
    }
    let code = token_to_code(binding.key_tokens[0].as_str())?;

    let mut mods = Modifiers::empty();
    if binding.ctrl || binding.left_ctrl || binding.right_ctrl {
        mods |= Modifiers::CONTROL;
    }
    if binding.alt || binding.left_alt || binding.right_alt {
        mods |= Modifiers::ALT;
    }
    if binding.shift || binding.left_shift || binding.right_shift {
        mods |= Modifiers::SHIFT;
    }
    if binding.super_key || binding.left_super || binding.right_super {
        mods |= Modifiers::SUPER;
    }
    Some(Shortcut::new(Some(mods), code))
}

/// Register `binding` as the global toggle, replacing whatever was registered
/// before. Passing `None` just clears the current registration.
pub fn apply(app: &AppHandle, binding: Option<&HotkeyBinding>) {
    let shortcut = binding.and_then(binding_to_shortcut);
    let gs = app.global_shortcut();

    if let Err(e) = gs.unregister_all() {
        log::warn!("[GlobalHotkey] unregister_all failed: {e}");
    }
    ACTIVE.store(false, Ordering::SeqCst);

    let Some(shortcut) = shortcut else {
        log::info!("[GlobalHotkey] no window-server shortcut for this binding; using polling");
        return;
    };

    match gs.register(shortcut) {
        Ok(()) => {
            ACTIVE.store(true, Ordering::SeqCst);
            log::info!("[GlobalHotkey] registered {shortcut:?} — works regardless of focus");
        }
        Err(e) => log::warn!("[GlobalHotkey] register failed ({e}); falling back to polling"),
    }
}
