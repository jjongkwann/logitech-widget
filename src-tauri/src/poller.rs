//! Background polling loop: reads all battery sources and pushes the merged
//! device list to the webview as "battery-update" events.

use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter};

use crate::battery::{hidpp::HidppSource, BatterySource};
use crate::Snapshot;

const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub fn spawn(app: AppHandle, snapshot: Snapshot) {
    thread::spawn(move || {
        let mut sources: Vec<Box<dyn BatterySource>> = Vec::new();
        match HidppSource::new() {
            Ok(s) => sources.push(Box::new(s)),
            Err(e) => eprintln!("hidpp source unavailable: {e}"),
        }
        loop {
            let devices: Vec<_> = sources.iter_mut().flat_map(|s| s.poll()).collect();
            *snapshot.lock().unwrap() = devices.clone();
            if let Err(e) = app.emit("battery-update", &devices) {
                eprintln!("battery-update emit failed: {e}");
            }
            thread::sleep(POLL_INTERVAL);
        }
    });
}
