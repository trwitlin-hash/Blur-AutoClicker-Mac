//! macOS platform support for global hotkeys.
//!
//! Provides `VK_*`-style names (retaining upstream's naming) mapped onto macOS CGKeyCodes, a
//! direct HID key/button state query, and a CGEventTap that tracks key state
//! even when the app is not frontmost. Extracted from the Djozman macOS fork.

pub mod vk_codes {
    pub const VK_CONTROL: u16 = 0x3B; // left Control
    pub const VK_RCONTROL: u16 = 0x3E; // right Control
    pub const VK_MENU: u16 = 0x3A; // left Option / Alt
    pub const VK_RMENU: u16 = 0x3D; // right Option
    pub const VK_SHIFT: u16 = 0x38; // left Shift
    pub const VK_RSHIFT: u16 = 0x3C; // right Shift
    pub const VK_LCONTROL: u16 = VK_CONTROL;
    pub const VK_LMENU: u16 = VK_MENU;
    pub const VK_LSHIFT: u16 = VK_SHIFT;
    pub const VK_LWIN: u16 = 0x37; // left Command
    pub const VK_RWIN: u16 = 0x36; // right Command

    pub const VK_SPACE: u16 = 0x31;
    pub const VK_TAB: u16 = 0x30;
    pub const VK_RETURN: u16 = 0x24;
    pub const VK_BACK: u16 = 0x33;
    pub const VK_DELETE: u16 = 0x75;
    pub const VK_INSERT: u16 = 0x72; // Help key on Mac
    pub const VK_HOME: u16 = 0x73;
    pub const VK_END: u16 = 0x77;
    pub const VK_PRIOR: u16 = 0x74; // Page Up
    pub const VK_NEXT: u16 = 0x79; // Page Down
    pub const VK_UP: u16 = 0x7E;
    pub const VK_DOWN: u16 = 0x7D;
    pub const VK_LEFT: u16 = 0x7B;
    pub const VK_RIGHT: u16 = 0x7C;
    pub const VK_ESCAPE: u16 = 0x35;
    pub const VK_CAPITAL: u16 = 0x39; // Caps Lock
    pub const VK_NUMLOCK: u16 = 0x47; // Clear on Mac numpad
    pub const VK_SCROLL: u16 = 0xFF; // no Scroll Lock on Mac
    pub const VK_APPS: u16 = 0xFF; // no Apps key on Mac
    pub const VK_SNAPSHOT: u16 = 0xFF; // no Print Screen on Mac
    pub const VK_PAUSE: u16 = 0xFF; // no Pause on Mac

    // OEM / punctuation (US ANSI layout)
    pub const VK_OEM_2: u16 = 0x2C; // /
    pub const VK_OEM_5: u16 = 0x2A; // backslash
    pub const VK_OEM_1: u16 = 0x29; // ;
    pub const VK_OEM_7: u16 = 0x27; // '
    pub const VK_OEM_4: u16 = 0x21; // [
    pub const VK_OEM_6: u16 = 0x1E; // ]
    pub const VK_OEM_MINUS: u16 = 0x1B; // -
    pub const VK_OEM_PLUS: u16 = 0x18; // =
    pub const VK_OEM_3: u16 = 0x32; // `
    pub const VK_OEM_COMMA: u16 = 0x2B; // ,
    pub const VK_OEM_PERIOD: u16 = 0x2F; // .
    pub const VK_OEM_102: u16 = 0x0A; // IntlBackslash (non-US)

    // Numpad
    pub const VK_NUMPAD0: u16 = 0x52;
    pub const VK_NUMPAD1: u16 = 0x53;
    pub const VK_NUMPAD2: u16 = 0x54;
    pub const VK_NUMPAD3: u16 = 0x55;
    pub const VK_NUMPAD4: u16 = 0x56;
    pub const VK_NUMPAD5: u16 = 0x57;
    pub const VK_NUMPAD6: u16 = 0x58;
    pub const VK_NUMPAD7: u16 = 0x59;
    pub const VK_NUMPAD8: u16 = 0x5B;
    pub const VK_NUMPAD9: u16 = 0x5C;
    pub const VK_ADD: u16 = 0x45;
    pub const VK_SUBTRACT: u16 = 0x4E;
    pub const VK_MULTIPLY: u16 = 0x43;
    pub const VK_DIVIDE: u16 = 0x4B;
    pub const VK_DECIMAL: u16 = 0x41;

    // Mouse buttons encoded above the CGKeyCode range
    pub const VK_LBUTTON: u16 = 0xFFF0;
    pub const VK_RBUTTON: u16 = 0xFFF1;
    pub const VK_MBUTTON: u16 = 0xFFF2;
    pub const VK_XBUTTON1: u16 = 0xFFF3;
    pub const VK_XBUTTON2: u16 = 0xFFF4;
}

// ── macOS key-state polling ───────────────────────────────────────────────────

pub mod macos_input {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
        pub fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
    }

    pub const HID_SYSTEM_STATE: i32 = 1;
}

// ── macOS CGEventTap-based key-state tracking (lock‑free) ────────────────────
// On macOS 14+ (especially with M‑series chips), CGEventSourceKeyState with
// kCGEventSourceStateHIDSystemState does NOT report key presses when the app is
// not the frontmost process.  A CGEventTap fixes this because it hooks into the
// HID event stream *before* the window server filters events by active Space.
pub mod macos_event_tap {
    use super::macos_input;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    // ── CGEventTap FFI ────────────────────────────────────────────────────────
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: unsafe extern "C" fn(
                *mut std::ffi::c_void,
                u32,
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void,
            user_info: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn CGEventTapEnable(tap: *mut std::ffi::c_void, enable: bool);
        fn CGEventGetIntegerValueField(event: *mut std::ffi::c_void, field: u32) -> i64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: *mut std::ffi::c_void,
            port: *mut std::ffi::c_void,
            order: i64,
        ) -> *mut std::ffi::c_void;
        fn CFRunLoopGetCurrent() -> *mut std::ffi::c_void;
        fn CFRunLoopAddSource(
            rl: *mut std::ffi::c_void,
            source: *mut std::ffi::c_void,
            mode: *mut std::ffi::c_void,
        );
        fn CFRunLoopRun();
        static kCFRunLoopCommonModes: *mut std::ffi::c_void;
    }

    // ── Constants ────────────────────────────────────────────────────────────
    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
    const KCG_EVENT_KEY_DOWN: u32 = 10; // NX_KEYDOWN
    const KCG_EVENT_KEY_UP: u32 = 11; // NX_KEYUP
    const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

    // ── Lock‑free key state (4 × AtomicU64 covering CGKeyCode 0…255) ─────────
    static B0: AtomicU64 = AtomicU64::new(0); // keys   0… 63
    static B1: AtomicU64 = AtomicU64::new(0); // keys  64…127
    static B2: AtomicU64 = AtomicU64::new(0); // keys 128…191
    static B3: AtomicU64 = AtomicU64::new(0); // keys 192…255
    pub static ACTIVE: AtomicBool = AtomicBool::new(false);

    fn set(code: u16, down: bool) {
        let code = code as usize;
        if code > 255 {
            return;
        }
        let bucket = code / 64;
        let mask = 1u64 << (code as u64 % 64);
        let atom = match bucket {
            0 => &B0,
            1 => &B1,
            2 => &B2,
            3 => &B3,
            _ => return,
        };
        if down {
            atom.fetch_or(mask, Ordering::SeqCst);
        } else {
            atom.fetch_and(!mask, Ordering::SeqCst);
        }
    }

    pub fn is_down(code: u16) -> bool {
        let code = code as usize;
        if code > 255 {
            return false;
        }
        let bucket = code / 64;
        let mask = 1u64 << (code as u64 % 64);
        let atom = match bucket {
            0 => &B0,
            1 => &B1,
            2 => &B2,
            3 => &B3,
            _ => return false,
        };
        (atom.load(Ordering::SeqCst) & mask) != 0
    }

    unsafe extern "C" fn callback(
        _proxy: *mut std::ffi::c_void,
        event_type: u32,
        event: *mut std::ffi::c_void,
        _user_info: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void {
        let code = unsafe { CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) } as u16;
        set(code, event_type == KCG_EVENT_KEY_DOWN);
        event // pass through
    }

    /// Spawn a background thread that creates a key‑event tap and updates the
    /// atomic bitmap.  Returns immediately; the thread keeps running.
    pub fn start() {
        std::thread::spawn(|| unsafe {
            let tap = CGEventTapCreate(
                KCG_HID_EVENT_TAP,
                KCG_HEAD_INSERT_EVENT_TAP,
                KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
                (1u64 << KCG_EVENT_KEY_DOWN) | (1u64 << KCG_EVENT_KEY_UP),
                callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                log::warn!(
                    "[Hotkey] CGEventTapCreate failed – Accessibility permissions needed. \
                     Polling fallback active."
                );
                return;
            }

            let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
            if source.is_null() {
                log::warn!("[Hotkey] CFMachPortCreateRunLoopSource failed.");
                return;
            }

            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);

            ACTIVE.store(true, Ordering::SeqCst);
            log::info!("[Hotkey] Event tap active.");

            CFRunLoopRun(); // blocks until CFRunLoopStop is called
        });
    }

    // ── Mouse‑button state keeps using CGEventSourceButtonState ──────────────
    pub fn is_mouse_down(button: u32) -> bool {
        unsafe { macos_input::CGEventSourceButtonState(macos_input::HID_SYSTEM_STATE, button) }
    }
}
