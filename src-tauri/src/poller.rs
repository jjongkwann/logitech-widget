//! Background polling loop: reads all battery sources, pushes the merged
//! device list to the webview as "battery-update" events, and raises a toast
//! when a device crosses the low-battery threshold.

use std::collections::{HashMap, HashSet};
use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
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
        let mut last_known = load_cache(&app);
        loop {
            let mut devices = sources
                .iter_mut()
                .map(|s| s.poll())
                .reduce(merge)
                .unwrap_or_default();
            fill_last_known(&app, &mut devices, &mut last_known);
            notify_low_batteries(&app, &devices, settings.low_battery_threshold, &mut alerted);
            *snapshot.lock().unwrap() = devices.clone();
            if let Err(e) = app.emit("battery-update", &devices) {
                eprintln!("battery-update emit failed: {e}");
            }
            thread::sleep(interval);
        }
    });
}

/// BLE devices (e.g. Bolt mice) drop their radio link within seconds of
/// idling, so polls frequently catch them "offline". Remember the last
/// reading per device name and show it instead of a blank, persisting across
/// restarts.
fn fill_last_known(
    app: &AppHandle,
    devices: &mut [DeviceBattery],
    last_known: &mut HashMap<String, u8>,
) {
    let mut dirty = false;
    for d in devices.iter_mut() {
        if d.online {
            if let Some(p) = d.percentage {
                if last_known.insert(d.name.clone(), p) != Some(p) {
                    dirty = true;
                }
            }
        } else if d.percentage.is_none() {
            d.percentage = last_known.get(&d.name).copied();
        }
    }
    if dirty {
        save_cache(app, last_known);
    }
}

fn cache_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("battery-cache.json"))
}

fn load_cache(app: &AppHandle) -> HashMap<String, u8> {
    cache_file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_cache(app: &AppHandle, cache: &HashMap<String, u8>) {
    if let Some(p) = cache_file(app) {
        let _ = std::fs::write(p, serde_json::to_string(cache).unwrap());
    }
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
