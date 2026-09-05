use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::ClickerState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopZoneRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

fn normalize_rect(start: (i32, i32), end: (i32, i32)) -> StopZoneRect {
    let left = start.0.min(end.0);
    let top = start.1.min(end.1);
    let right = start.0.max(end.0);
    let bottom = start.1.max(end.1);

    StopZoneRect {
        x: left,
        y: top,
        width: right - left + 1,
        height: bottom - top + 1,
    }
}

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct PickerRuntime {
    active: bool,
    drawing_start: Option<(i32, i32)>,
    app: Option<AppHandle>,
    last_preview_emit: Option<Instant>,
}

unsafe impl Send for PickerRuntime {}
unsafe impl Sync for PickerRuntime {}

static PICKER: OnceLock<Mutex<PickerRuntime>> = OnceLock::new();

fn picker() -> &'static Mutex<PickerRuntime> {
    PICKER.get_or_init(|| Mutex::new(PickerRuntime::default()))
}

mod platform {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use tauri::{AppHandle, Emitter, Manager};

    use super::{normalize_rect, picker, StopZoneRect};
    use crate::ClickerState;

    const NX_RMOUSEDOWN: u32 = 3;
    const NX_RMOUSEUP: u32 = 4;
    const NX_KEYDOWN: u32 = 10;
    const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    const VK_ESCAPE: u16 = 53;

    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: unsafe extern "C" fn(
                *mut c_void,
                u32,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void,
            user_info: *mut c_void,
        ) -> *mut c_void;
        fn CGEventTapEnable(tap: *mut c_void, enable: bool);
        fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
        fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: *mut c_void,
            port: *mut c_void,
            order: i64,
        ) -> *mut c_void;
        fn CFRunLoopGetCurrent() -> *mut c_void;
        fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *mut c_void);
        fn CFRunLoopRun();
        fn CFRelease(cf: *mut c_void);
        static kCFRunLoopCommonModes: *mut c_void;
    }

    static START_DRAWING: AtomicBool = AtomicBool::new(false);
    static FINISH_DRAWING: AtomicBool = AtomicBool::new(false);
    static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
    static PICK_X: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    static PICK_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

    unsafe extern "C" fn tap_callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut c_void,
        _user_info: *mut c_void,
    ) -> *mut c_void {
        match event_type {
            NX_RMOUSEDOWN => {
                let point = CGEventGetLocation(event);
                PICK_X.store(point.x as i32, Ordering::SeqCst);
                PICK_Y.store(point.y as i32, Ordering::SeqCst);
                let drawing = picker().lock().unwrap().drawing_start.is_some();
                if drawing {
                    FINISH_DRAWING.store(true, Ordering::SeqCst);
                } else {
                    START_DRAWING.store(true, Ordering::SeqCst);
                }
                std::ptr::null_mut()
            }
            NX_RMOUSEUP => std::ptr::null_mut(),
            NX_KEYDOWN => {
                let code = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u16;
                if code == VK_ESCAPE {
                    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
                    std::ptr::null_mut()
                } else {
                    event
                }
            }
            _ => event,
        }
    }

    pub fn start_hooks(app: &AppHandle) -> Result<(), String> {
        START_DRAWING.store(false, Ordering::SeqCst);
        FINISH_DRAWING.store(false, Ordering::SeqCst);
        CANCEL_REQUESTED.store(false, Ordering::SeqCst);

        let (ready_tx, ready_rx) = mpsc::channel();

        std::thread::spawn(move || unsafe {
            let events_of_interest =
                (1u64 << NX_RMOUSEDOWN) | (1u64 << NX_RMOUSEUP) | (1u64 << NX_KEYDOWN);

            let tap = CGEventTapCreate(
                KCG_HID_EVENT_TAP,
                KCG_HEAD_INSERT_EVENT_TAP,
                KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
                events_of_interest,
                tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                let _ = ready_tx.send(Err(String::from(
                    "Event tap failed – need Accessibility permissions",
                )));
                return;
            }

            let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
            if source.is_null() {
                CFRelease(tap);
                let _ = ready_tx.send(Err(String::from("Run loop source failed")));
                return;
            }

            let _ = ready_tx.send(Ok(()));
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            CFRunLoopRun();
            CGEventTapEnable(tap, false);
            CFRelease(source);
            CFRelease(tap);
        });

        match ready_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                super::cancel_custom_stop_zone_pick_inner(app);
                Err(e)
            }
            Err(_) => {
                super::cancel_custom_stop_zone_pick_inner(app);
                Err(String::from("Timed out"))
            }
        }
    }

    pub fn stop_hooks(_notify_overlay: bool) -> Option<AppHandle> {
        let app = {
            let mut runtime = picker().lock().unwrap();
            let app = runtime.app.clone();
            runtime.active = false;
            runtime.app = None;
            runtime.drawing_start = None;
            runtime.last_preview_emit = None;
            app
        };
        if let Some(app) = &app {
            app.state::<ClickerState>()
                .custom_stop_zone_pick_active
                .store(false, Ordering::SeqCst);
        }
        app
    }

    pub fn poll_pick(app: &AppHandle) -> bool {
        if CANCEL_REQUESTED.swap(false, Ordering::SeqCst) {
            let app = app.clone();
            super::cancel_custom_stop_zone_pick_inner(&app);
            return false;
        }

        if START_DRAWING.swap(false, Ordering::SeqCst) {
            let x = PICK_X.load(Ordering::SeqCst);
            let y = PICK_Y.load(Ordering::SeqCst);
            picker().lock().unwrap().drawing_start = Some((x, y));
            let _ = app.emit(
                "custom-stop-zone-preview",
                StopZoneRect {
                    x,
                    y,
                    width: 1,
                    height: 1,
                },
            );
            return true;
        }

        if FINISH_DRAWING.swap(false, Ordering::SeqCst) {
            let end_x = PICK_X.load(Ordering::SeqCst);
            let end_y = PICK_Y.load(Ordering::SeqCst);
            let start = picker().lock().unwrap().drawing_start;
            if let Some(start) = start {
                let rect = normalize_rect(start, (end_x, end_y));
                super::finish_custom_stop_zone_pick(rect);
            }
            return true;
        }

        false
    }
}

// ── Public API ─────────────────────────────────────────────────────────────────

pub fn start_custom_stop_zone_pick_inner(app: AppHandle) -> Result<(), String> {
    crate::click_point_picker::cancel_click_point_pick_inner(&app);

    {
        let mut runtime = picker().lock().unwrap();
        if runtime.active {
            crate::overlay::show_custom_stop_zone_pick_overlay(&app).map_err(|e| e.to_string())?;
            return Ok(());
        }
        runtime.active = true;
        runtime.drawing_start = None;
        runtime.app = Some(app.clone());
        runtime.last_preview_emit = None;
    }

    app.state::<ClickerState>()
        .custom_stop_zone_pick_active
        .store(true, Ordering::SeqCst);

    crate::overlay::show_custom_stop_zone_pick_overlay(&app).map_err(|e| e.to_string())?;

    {
        platform::start_hooks(&app)?;

        let app_handle = app.clone();
        std::thread::spawn(move || loop {
            let runtime = picker().lock().unwrap();
            let active = runtime.active;
            drop(runtime);
            if !active {
                break;
            }

            platform::poll_pick(&app_handle);
            std::thread::sleep(Duration::from_millis(16));
        });
    }

    Ok(())
}

pub fn cancel_custom_stop_zone_pick_inner(app: &AppHandle) {
    let app_opt = platform::stop_hooks(true);
    if let Some(a) = app_opt.or(Some(app.clone())) {
        let _ = crate::overlay::hide_custom_stop_zone_pick_overlay(&a);
    }
}

fn finish_custom_stop_zone_pick(rect: StopZoneRect) {
    let app = platform::stop_hooks(true);
    if let Some(app) = app {
        let _ = app.emit("custom-stop-zone-picked", rect);
        let _ = crate::overlay::hide_custom_stop_zone_pick_overlay(&app);
    }
}
