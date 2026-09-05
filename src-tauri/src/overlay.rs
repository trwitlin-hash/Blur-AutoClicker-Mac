use crate::app_state::ClickerState;
use crate::engine::mouse::{
    current_cursor_position, current_monitor_rects, current_virtual_screen_rect, VirtualScreenRect,
};
use crate::error::poisoned_inner;
use crate::error::AppError;
use crate::error::AppResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

static LAST_ZONE_SHOW: Mutex<Option<Instant>> = Mutex::new(None);
static CLICK_POINT_PICK_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static CUSTOM_STOP_ZONE_PICK_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static OVERLAY_THREAD_RUNNING: AtomicBool = AtomicBool::new(true);

pub fn init_overlay(app: &AppHandle) -> AppResult<()> {
    let window = match app.get_webview_window("overlay") {
        Some(w) => w,
        None => {
            let overlay_data_dir = crate::portable::webview_dir("overlay").unwrap_or_else(|| {
                app.path()
                    .app_local_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("EBWebView-overlay")
            });

            log::info!(
                "[Overlay] Creating overlay window (user data: {})",
                overlay_data_dir.display()
            );

            tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("overlay.html".into()),
            )
            .title("Overlay")
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .focusable(false)
            .shadow(false)
            .data_directory(overlay_data_dir)
            .build()?
        }
    };

    log::info!("[Overlay] Running one-time init...");

    window.set_ignore_cursor_events(true)?;
    let _ = window.set_decorations(false);

    log::info!("[Overlay] Init complete — window configured but hidden");
    Ok(())
}

pub fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<ClickerState>();
    if !state.settings_initialized.load(Ordering::SeqCst) {
        return Ok(());
    }
    {
        let settings = state.settings.lock().unwrap_or_else(poisoned_inner);
        if !settings.show_stop_overlay {
            return Ok(());
        }
    }

    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| AppError::OverlayNotFound)?;
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| AppError::State("Virtual screen bounds not available".into()))?;

    *LAST_ZONE_SHOW.lock().unwrap_or_else(poisoned_inner) = Some(Instant::now());

    let settings = state.settings.lock().unwrap_or_else(poisoned_inner);
    let monitors = current_monitor_rects().unwrap_or_else(|| vec![bounds]);
    let stop_zones_payload: Vec<_> = settings
        .stop_zones
        .iter()
        .map(|zone| {
            let offset =
                VirtualScreenRect::new(zone.x, zone.y, zone.width.max(1), zone.height.max(1))
                    .offset_from(bounds);
            serde_json::json!({
                "id": zone.id,
                "x": offset.left,
                "y": offset.top,
                "width": offset.width,
                "height": offset.height,
                "action": zone.action,
            })
        })
        .collect();
    let monitor_payload: Vec<_> = monitors
        .into_iter()
        .map(|monitor: VirtualScreenRect| {
            let offset = monitor.offset_from(bounds);
            serde_json::json!({
                "x": offset.left,
                "y": offset.top,
                "width": offset.width,
                "height": offset.height,
            })
        })
        .collect();
    let _ = window.emit(
        "zone-data",
        serde_json::json!({
            "edgeStopEnabled": settings.edge_stop_enabled,
            "edgeStopTop": settings.edge_stop_top,
            "edgeStopRight": settings.edge_stop_right,
            "edgeStopBottom": settings.edge_stop_bottom,
            "edgeStopLeft": settings.edge_stop_left,
            "cornerStopEnabled": settings.corner_stop_enabled,
            "cornerStopTL": settings.corner_stop_tl,
            "cornerStopTR": settings.corner_stop_tr,
            "cornerStopBL": settings.corner_stop_bl,
            "cornerStopBR": settings.corner_stop_br,
            "stopZones": stop_zones_payload,
            "stopZonesEnabled": settings.stop_zones_enabled,
            "screenWidth": bounds.width,
            "screenHeight": bounds.height,
            "monitors": monitor_payload,
            "_showDisabledEdges": !settings.edge_stop_enabled,
            "_showDisabledCorners": !settings.corner_stop_enabled,
        }),
    );

    Ok(())
}

pub fn show_click_points_overlay(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<ClickerState>();
    if !state.settings_initialized.load(Ordering::SeqCst) {
        return Ok(());
    }

    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| AppError::OverlayNotFound)?;
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| AppError::State("Virtual screen bounds not available".into()))?;
    let points = {
        let settings = state.settings.lock().unwrap_or_else(poisoned_inner);
        settings.click_points.clone()
    };

    emit_click_points(&window, bounds, &points, false);
    if points.is_empty() && !CLICK_POINT_PICK_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        *LAST_ZONE_SHOW.lock().unwrap_or_else(poisoned_inner) = None;
        hide_overlay_window(&window);
    } else {
        *LAST_ZONE_SHOW.lock().unwrap_or_else(poisoned_inner) = Some(Instant::now());
    }
    Ok(())
}

pub fn show_click_point_pick_overlay(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| AppError::OverlayNotFound)?;
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| AppError::State("Virtual screen bounds not available".into()))?;

    CLICK_POINT_PICK_OVERLAY_ACTIVE.store(true, Ordering::SeqCst);

    let state = app.state::<ClickerState>();
    let settings = state.settings.lock().unwrap_or_else(poisoned_inner);
    emit_click_points(&window, bounds, &settings.click_points, true);
    set_click_point_pick_mode(app, true)?;

    if let Some((x, y)) = current_cursor_position() {
        let offset = VirtualScreenRect::new(x, y, 1, 1).offset_from(bounds);
        let _ = window.emit(
            "click-pick-cursor",
            serde_json::json!({
                "x": offset.left,
                "y": offset.top,
            }),
        );
    }

    Ok(())
}

pub fn set_click_point_pick_mode(app: &AppHandle, active: bool) -> AppResult<()> {
    CLICK_POINT_PICK_OVERLAY_ACTIVE.store(active, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.emit(
            "click-pick-mode",
            serde_json::json!({
                "active": active,
            }),
        );
    }
    Ok(())
}

pub fn show_custom_stop_zone_pick_overlay(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| AppError::OverlayNotFound)?;
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| AppError::State("Virtual screen bounds not available".into()))?;

    CUSTOM_STOP_ZONE_PICK_OVERLAY_ACTIVE.store(true, Ordering::SeqCst);
    show_overlay(app)?;
    set_custom_stop_zone_pick_mode(app, true)?;

    if let Some((x, y)) = current_cursor_position() {
        let offset = VirtualScreenRect::new(x, y, 1, 1).offset_from(bounds);
        let _ = window.emit(
            "custom-stop-zone-preview",
            serde_json::json!({
                "cursorX": offset.left,
                "cursorY": offset.top,
            }),
        );
    }

    Ok(())
}

pub fn set_custom_stop_zone_pick_mode(app: &AppHandle, active: bool) -> AppResult<()> {
    CUSTOM_STOP_ZONE_PICK_OVERLAY_ACTIVE.store(active, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.emit(
            "custom-stop-zone-pick-mode",
            serde_json::json!({
                "active": active,
            }),
        );
    }
    Ok(())
}

pub fn hide_custom_stop_zone_pick_overlay(app: &AppHandle) -> AppResult<()> {
    set_custom_stop_zone_pick_mode(app, false)?;
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.emit("custom-stop-zone-clear-preview", ());
        hide_overlay_window(&window);
    }
    Ok(())
}

fn emit_click_points(
    window: &tauri::WebviewWindow,
    bounds: VirtualScreenRect,
    points: &[crate::settings::ClickPoint],
    persistent: bool,
) {
    let points_payload: Vec<_> = points
        .iter()
        .map(|point| {
            let offset = VirtualScreenRect::new(point.x, point.y, 1, 1).offset_from(bounds);
            serde_json::json!({
                "id": point.id,
                "x": offset.left,
                "y": offset.top,
                "radius": point.radius,
            })
        })
        .collect();

    let _ = window.emit(
        "click-points-data",
        serde_json::json!({
            "points": points_payload,
            "screenWidth": bounds.width,
            "screenHeight": bounds.height,
            "persistent": persistent,
        }),
    );
}

// ---- Background timer ----

#[allow(unused_variables)]
pub fn check_auto_hide(app: &AppHandle) {
    if CLICK_POINT_PICK_OVERLAY_ACTIVE.load(Ordering::SeqCst)
        || CUSTOM_STOP_ZONE_PICK_OVERLAY_ACTIVE.load(Ordering::SeqCst)
    {
        return;
    }

    let mut last = LAST_ZONE_SHOW.lock().unwrap_or_else(poisoned_inner);
    if let Some(instant) = *last {
        if instant.elapsed() >= Duration::from_secs(3) {
            // ↑ auto-hide after timer

            *last = None;
            log::info!("[Overlay] Auto-hide: hiding window");
        }
    }
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> AppResult<()> {
    *LAST_ZONE_SHOW.lock().unwrap_or_else(poisoned_inner) = None;
    CLICK_POINT_PICK_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
    CUSTOM_STOP_ZONE_PICK_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("overlay") {
        hide_overlay_window(&window);
    }
    Ok(())
}

fn hide_overlay_window(window: &tauri::WebviewWindow) {
    let _ = window.hide();
}
