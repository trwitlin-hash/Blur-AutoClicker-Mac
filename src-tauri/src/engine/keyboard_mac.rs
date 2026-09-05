//! macOS keyboard input via CoreGraphics events.
//!
//! Ported from the Windows SendInput implementation. Adapted to the 3.9.6
//! engine API, where `sleep_interruptible` takes a `should_abort` predicate
//! and `send_key_down`/`send_key_up` take an `uppercase` flag rather than a
//! pre-resolved `use_shift`.

use crate::engine::cycle::{execute_click_cycle, ClickCycleKind, ClickCyclePlan};
use crate::engine::worker::{sleep_interruptible, RunControl};
use std::ffi::c_void;

const CG_EVENT_TAP_HID: u32 = 1; // kCGSessionEventTap - session level (faster path)
const CG_EVENT_SOURCE_STATE_HID: i32 = 1;

/// Standard macOS virtual key code for Shift.
const VK_SHIFT: u16 = 56;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *mut c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> *mut c_void;
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CGEventSourceCreate(state_id: i32) -> *mut c_void;
    fn CFRelease(cf: *mut c_void);
}

struct EventSource(*mut c_void);
unsafe impl Send for EventSource {}
unsafe impl Sync for EventSource {}

fn event_source() -> *mut c_void {
    use std::sync::OnceLock;
    static SOURCE: OnceLock<EventSource> = OnceLock::new();
    SOURCE
        .get_or_init(|| EventSource(unsafe { CGEventSourceCreate(CG_EVENT_SOURCE_STATE_HID) }))
        .0
}

pub fn is_alphabetic_vk(vk: u16) -> bool {
    // Native macOS CGKeyCodes for the 26 ANSI letter keys.
    matches!(
        vk,
        0x00 | 0x01
            | 0x02
            | 0x03
            | 0x04
            | 0x05
            | 0x06
            | 0x07
            | 0x08
            | 0x09
            | 0x0B
            | 0x0C
            | 0x0D
            | 0x0E
            | 0x0F
            | 0x10
            | 0x11
            | 0x1F
            | 0x20
            | 0x22
            | 0x23
            | 0x25
            | 0x26
            | 0x28
            | 0x2D
            | 0x2E
    )
}

fn send_key_event(vk: u16, key_down: bool) {
    unsafe {
        let event = CGEventCreateKeyboardEvent(event_source(), vk, key_down);
        if !event.is_null() {
            CGEventPost(CG_EVENT_TAP_HID, event);
            CFRelease(event);
        }
    }
}

#[inline]
fn should_hold_shift_for_case(vk: u16, uppercase: bool) -> bool {
    uppercase && is_alphabetic_vk(vk)
}

/// Press (and hold) a key, matching the Windows `send_key_down` contract.
pub fn send_key_down(vk: u16, uppercase: bool) {
    if should_hold_shift_for_case(vk, uppercase) {
        send_key_event(VK_SHIFT, true);
    }
    send_key_event(vk, true);
}

/// Release a key previously pressed with [`send_key_down`].
pub fn send_key_up(vk: u16, uppercase: bool) {
    send_key_event(vk, false);
    if should_hold_shift_for_case(vk, uppercase) {
        send_key_event(VK_SHIFT, false);
    }
}

fn send_key_batch(vk: u16, n: usize, uppercase: bool) {
    let needs_shift = should_hold_shift_for_case(vk, uppercase);
    unsafe {
        let ev_down = CGEventCreateKeyboardEvent(event_source(), vk, true);
        let ev_up = CGEventCreateKeyboardEvent(event_source(), vk, false);
        if ev_down.is_null() || ev_up.is_null() {
            if !ev_down.is_null() {
                CFRelease(ev_down);
            }
            if !ev_up.is_null() {
                CFRelease(ev_up);
            }
            return;
        }
        let shift_down = if needs_shift {
            CGEventCreateKeyboardEvent(event_source(), VK_SHIFT, true)
        } else {
            std::ptr::null_mut()
        };
        let shift_up = if needs_shift {
            CGEventCreateKeyboardEvent(event_source(), VK_SHIFT, false)
        } else {
            std::ptr::null_mut()
        };
        for _ in 0..n {
            if needs_shift && !shift_down.is_null() {
                CGEventPost(CG_EVENT_TAP_HID, shift_down);
            }
            CGEventPost(CG_EVENT_TAP_HID, ev_down);
            CGEventPost(CG_EVENT_TAP_HID, ev_up);
            if needs_shift && !shift_up.is_null() {
                CGEventPost(CG_EVENT_TAP_HID, shift_up);
            }
        }
        if !shift_down.is_null() {
            CFRelease(shift_down);
        }
        if !shift_up.is_null() {
            CFRelease(shift_up);
        }
        CFRelease(ev_down);
        CFRelease(ev_up);
    }
}

pub fn send_key_presses(
    vk: u16,
    count: usize,
    uppercase: bool,
    plan: ClickCyclePlan,
    control: &RunControl,
    should_abort: &dyn Fn() -> bool,
) {
    if count == 0 || should_abort() {
        return;
    }
    if plan.kind == ClickCycleKind::Single && count > 1 && plan.first_hold_ms == 0 {
        send_key_batch(vk, count, uppercase);
        return;
    }

    let is_active = || control.is_active() && !should_abort();
    let mut sleep_for = |duration| sleep_interruptible(duration, control, should_abort);

    for _ in 0..count {
        if should_abort() {
            return;
        }
        if !execute_click_cycle(
            plan,
            &mut || send_key_down(vk, uppercase),
            &mut || send_key_up(vk, uppercase),
            &mut sleep_for,
            &is_active,
        ) {
            return;
        }
    }
}
