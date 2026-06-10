//! Background polling loop: reads all battery sources and pushes the merged
//! device list to the webview as "battery-update" events.

use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter};

use crate::battery::{ghub::GHubSource, hidpp::HidppSource, merge, BatterySource};
use crate::Snapshot;

const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub fn spawn(app: AppHandle, snapshot: Snapshot) {
    thread::spawn(move || {
        // Order matters: earlier sources win when merging (HID++ is primary).
        let mut sources: Vec<Box<dyn BatterySource>> = Vec::new();
        match HidppSource::new() {
            Ok(s) => sources.push(Box::new(s)),
            Err(e) => eprintln!("hidpp source unavailable: {e}"),
        }
        sources.push(Box::new(GHubSource));
        loop {
            let devices = sources
                .iter_mut()
                .map(|s| s.poll())
                .reduce(merge)
                .unwrap_or_default();
            *snapshot.lock().unwrap() = devices.clone();
            if let Err(e) = app.emit("battery-update", &devices) {
                eprintln!("battery-update emit failed: {e}");
            }
            thread::sleep(POLL_INTERVAL);
        }
    });
}
