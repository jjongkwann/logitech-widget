pub mod battery;
mod poller;
mod settings;
mod tray;
mod window;

use std::sync::{Arc, Mutex};

use battery::DeviceBattery;
use tauri::Manager;

/// Latest poll snapshot, shared between the poller thread and the
/// `get_batteries` command so a freshly loaded webview doesn't have to wait
/// for the next poll cycle.
pub type Snapshot = Arc<Mutex<Vec<DeviceBattery>>>;

#[tauri::command]
fn get_batteries(state: tauri::State<Snapshot>) -> Vec<DeviceBattery> {
    state.lock().unwrap().clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let snapshot: Snapshot = Arc::new(Mutex::new(Vec::new()));
    tauri::Builder::default()
        // Must be registered first: a second launch just reveals the running widget.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(snapshot.clone())
        .invoke_handler(tauri::generate_handler![get_batteries])
        .setup(move |app| {
            let handle = app.handle();
            let main = app.get_webview_window("main").expect("main window");
            window::restore_position(handle, &main);
            window::pin_to_desktop(&main);
            tray::setup(handle)?;
            poller::spawn(handle.clone(), snapshot, settings::load(handle));
            Ok(())
        })
        .on_window_event(|win, event| {
            if let tauri::WindowEvent::Moved(pos) = event {
                window::save_position(win.app_handle(), pos);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
