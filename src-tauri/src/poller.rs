//! Background polling loop: reads all battery sources, pushes the merged
//! device list to the webview as "battery-update" events, and raises a toast
//! when a device crosses the low-battery threshold.

use std::collections::HashSet;
use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::battery::{ghub::GHubSource, hidpp::HidppSource, merge, BatterySource, DeviceBattery};
use crate::settings::Settings;
use crate::Snapshot;

pub fn spawn(app: AppHandle, snapshot: Snapshot, settings: Settings) {
    thread::spawn(move || {
        // Order matters: earlier sources win when merging (HID++ is primary).
        let mut sources: Vec<Box<dyn BatterySource>> = Vec::new();
        match HidppSource::new() {
            Ok(s) => sources.push(Box::new(s)),
            Err(e) => eprintln!("hidpp source unavailable: {e}"),
        }
        sources.push(Box::new(GHubSource));

        let interval = Duration::from_secs(settings.poll_interval_secs.max(5));
        let mut alerted: HashSet<String> = HashSet::new();
        loop {
            let devices = sources
                .iter_mut()
                .map(|s| s.poll())
                .reduce(merge)
                .unwrap_or_default();
            notify_low_batteries(&app, &devices, settings.low_battery_threshold, &mut alerted);
            *snapshot.lock().unwrap() = devices.clone();
            if let Err(e) = app.emit("battery-update", &devices) {
                eprintln!("battery-update emit failed: {e}");
            }
            thread::sleep(interval);
        }
    });
}

/// One toast per discharge cycle: re-arms once the device charges or recovers.
fn notify_low_batteries(
    app: &AppHandle,
    devices: &[DeviceBattery],
    threshold: u8,
    alerted: &mut HashSet<String>,
) {
    for d in devices {
        let low = d.online && !d.charging && d.percentage.is_some_and(|p| p <= threshold);
        if !low {
            alerted.remove(&d.id);
            continue;
        }
        if alerted.insert(d.id.clone()) {
            let _ = app
                .notification()
                .builder()
                .title("배터리 부족")
                .body(format!("{} — {}%", d.name, d.percentage.unwrap_or(0)))
                .show();
        }
    }
}
