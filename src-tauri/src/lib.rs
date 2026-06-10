pub mod battery;
mod poller;

use std::sync::{Arc, Mutex};

use battery::DeviceBattery;

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
        .manage(snapshot.clone())
        .invoke_handler(tauri::generate_handler![get_batteries])
        .setup(move |app| {
            poller::spawn(app.handle().clone(), snapshot);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
