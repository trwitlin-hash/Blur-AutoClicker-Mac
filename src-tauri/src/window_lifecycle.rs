use tauri::AppHandle;

fn trim_webview_processes() {}

pub fn start_periodic_trimming(interval_secs: u64) {
    std::thread::spawn(move || {
        while crate::overlay::OVERLAY_THREAD_RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
            trim_webview_processes();
        }
    });
}

pub fn on_hide(_app: &AppHandle) {
    trim_webview_processes();
}

pub fn on_show(_app: &AppHandle) {}
